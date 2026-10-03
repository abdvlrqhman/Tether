//! Composition root and thin local desktop IPC adapter.

use crate::{
    application::{
        operator::{OperatorService, RemoteView},
        HostService,
    },
    domain::HostView,
    infrastructure::{self, audit::JsonlAudit, remote::NativeRemoteTransport, shell::NativeShell},
};
use std::{path::PathBuf, sync::Arc, time::Duration};
use tauri::{Emitter, Manager, State};
use tokio::sync::{oneshot, Mutex};
struct Runtime {
    shutdown: oneshot::Sender<()>,
    server: tokio::task::JoinHandle<()>,
    tunnel: Option<tokio::process::Child>,
}
struct Desktop {
    host: HostService,
    remote: OperatorService,
    runtime: Mutex<Option<Runtime>>,
    home: String,
    audit_path: String,
    updating: std::sync::atomic::AtomicBool,
}
#[derive(serde::Serialize)]
struct Overview {
    host: HostView,
    remote: RemoteView,
    home: String,
    audit_path: String,
    executable: String,
}
#[tauri::command]
fn overview(state: State<Desktop>) -> Overview {
    Overview {
        host: state.host.snapshot(),
        remote: state.remote.view(),
        home: state.home.clone(),
        audit_path: state.audit_path.clone(),
        executable: std::env::current_exe()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }
}
#[tauri::command]
async fn start_host(
    state: State<'_, Desktop>,
    working_directory: String,
    online: bool,
) -> Result<(), String> {
    let binary = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Cannot locate application directory")?
        .join(if cfg!(windows) {
            "cloudflared.exe"
        } else {
            "cloudflared"
        });
    let mut runtime = state.runtime.lock().await;
    if state.updating.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("An update is being installed. Restart Tether before sharing.".into());
    }
    if runtime.is_some() {
        return Err("Already sharing this device".into());
    }
    let cwd = std::fs::canonicalize(PathBuf::from(working_directory.trim()))
        .map_err(|e| format!("Choose an existing working folder: {e}"))?;
    if !cwd.is_dir() {
        return Err("Working folder must be a directory".into());
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| e.to_string())?;
    let url = format!(
        "http://127.0.0.1:{}",
        listener.local_addr().map_err(|e| e.to_string())?.port()
    );
    state
        .host
        .start(url.clone(), cwd.to_string_lossy().into_owned(), online)?;
    let (shutdown, rx) = oneshot::channel();
    let host = state.host.clone();
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, infrastructure::http::router(host))
            .with_graceful_shutdown(async {
                let _ = rx.await;
            })
            .await;
    });
    let tunnel = if online {
        match infrastructure::tunnel::spawn(&binary, &url, state.host.clone()) {
            Ok(child) => Some(child),
            Err(e) => {
                state.host.stop();
                let _ = shutdown.send(());
                server.abort();
                return Err(e);
            }
        }
    } else {
        None
    };
    *runtime = Some(Runtime {
        shutdown,
        server,
        tunnel,
    });
    if online {
        let host = state.host.clone();
        let generation = host.generation();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(45)).await;
            host.tunnel_timeout(generation);
        });
    }
    Ok(())
}
#[tauri::command]
async fn stop_host(state: State<'_, Desktop>) -> Result<(), String> {
    let mut runtime = state.runtime.lock().await;
    state.host.stop();
    if let Some(mut rt) = runtime.take() {
        if let Some(mut child) = rt.tunnel.take() {
            let _ = child.kill().await;
        }
        let _ = rt.shutdown.send(());
        if tokio::time::timeout(Duration::from_secs(3), &mut rt.server)
            .await
            .is_err()
        {
            rt.server.abort();
        }
    }
    Ok(())
}
#[tauri::command]
fn approve_pair(state: State<Desktop>, id: String, minutes: u64) -> Result<(), String> {
    state.host.approve(&id, minutes)
}
#[tauri::command]
fn deny_pair(state: State<Desktop>, id: String) -> Result<(), String> {
    state.host.deny(&id)
}
#[tauri::command]
fn revoke_session(state: State<Desktop>) -> Result<(), String> {
    state.host.revoke()
}
#[tauri::command]
fn renew_invite(state: State<Desktop>) -> Result<(), String> {
    state.host.renew_invite()
}
#[tauri::command]
async fn connect_remote(
    state: State<'_, Desktop>,
    invitation: String,
    operator: String,
) -> Result<RemoteView, String> {
    let _runtime = state.runtime.lock().await;
    if state.updating.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("An update is being installed. Restart Tether before connecting.".into());
    }
    state.remote.begin(invitation, operator).await
}
#[tauri::command]
async fn poll_remote(state: State<'_, Desktop>) -> Result<RemoteView, String> {
    state.remote.poll().await
}
#[tauri::command]
fn disconnect_remote(state: State<Desktop>) {
    state.remote.disconnect();
}
#[tauri::command]
async fn open_terminal(
    state: State<'_, Desktop>,
    app: tauri::AppHandle,
    terminal_id: String,
) -> Result<(), String> {
    if terminal_id.len() > 80 {
        return Err("Invalid local terminal id".into());
    }
    let (id, mut output) = state.remote.open_terminal().await?;
    let operator = state.remote.clone();
    tokio::spawn(async move {
        while let Some(frame) = output.recv().await {
            let _ = app.emit(
                "terminal-message",
                serde_json::json!({"terminal_id":terminal_id,"frame":frame}),
            );
        }
        operator.terminal_closed(&id);
    });
    Ok(())
}
#[tauri::command]
fn terminal_send(state: State<Desktop>, message: String) -> Result<(), String> {
    state.remote.send(message)
}
#[tauri::command]
fn close_terminal(state: State<Desktop>) {
    state.remote.close_terminal();
}

#[tauri::command]
async fn begin_update(state: State<'_, Desktop>) -> Result<(), String> {
    let runtime = state.runtime.lock().await;
    if runtime.is_some() || matches!(state.remote.view().status.as_str(), "pending" | "connected") {
        return Err("Stop sharing and disconnect from devices before installing an update.".into());
    }
    state
        .updating
        .store(true, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}
#[tauri::command]
fn cancel_update(state: State<Desktop>) {
    state
        .updating
        .store(false, std::sync::atomic::Ordering::SeqCst);
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let audit_path = app.path().app_local_data_dir()?.join("audit.jsonl");
            let audit = JsonlAudit::open(&audit_path).map_err(std::io::Error::other)?;
            let host = HostService::new(
                Arc::new(infrastructure::SystemClock),
                Arc::new(infrastructure::RandomSecrets),
                Arc::new(audit),
                Arc::new(NativeShell::default()),
            );
            let monitor = host.clone();
            tauri::async_runtime::spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_secs(1));
                loop {
                    tick.tick().await;
                    monitor.expire();
                }
            });
            let home = app.path().home_dir()?.to_string_lossy().into_owned();
            app.manage(Desktop {
                host,
                remote: OperatorService::new(Arc::new(NativeRemoteTransport::new())),
                runtime: Mutex::new(None),
                home,
                audit_path: audit_path.to_string_lossy().into_owned(),
                updating: std::sync::atomic::AtomicBool::new(false),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            overview,
            start_host,
            stop_host,
            approve_pair,
            deny_pair,
            revoke_session,
            renew_invite,
            connect_remote,
            poll_remote,
            disconnect_remote,
            open_terminal,
            terminal_send,
            close_terminal,
            begin_update,
            cancel_update
        ])
        .build(tauri::generate_context!())
        .expect("Tether could not start")
        .run(|app, event| {
            if matches!(
                event,
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
            ) {
                if let Some(state) = app.try_state::<Desktop>() {
                    state.host.stop();
                    state.remote.disconnect();
                    if let Ok(mut rt) = state.runtime.try_lock() {
                        rt.take();
                    }
                }
            }
        });
}
