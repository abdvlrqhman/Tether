use crate::{
    application::ports::*,
    domain::{ExecRequest, ExecResult},
};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::{
    future::Future,
    io::{Read, Write},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
    sync::mpsc,
};

const OUTPUT_LIMIT: usize = 1024 * 1024;
#[derive(Default, Clone)]
pub struct NativeShell {
    processes: Arc<Mutex<Vec<Weak<super::process_scope::ProcessScope>>>>,
}
impl NativeShell {
    fn contain(&self, pid: Option<u32>) -> Result<Arc<super::process_scope::ProcessScope>, String> {
        let scope = Arc::new(super::process_scope::ProcessScope::new(
            pid.ok_or("Remote process has no PID")?,
        )?);
        let mut processes = self.processes.lock().unwrap();
        processes.retain(|p| p.strong_count() > 0);
        processes.push(Arc::downgrade(&scope));
        Ok(scope)
    }
}

impl Shell for NativeShell {
    fn stop_all(&self) {
        for scope in self
            .processes
            .lock()
            .unwrap()
            .iter()
            .filter_map(Weak::upgrade)
        {
            scope.terminate();
        }
    }
    fn terminal(&self, cwd: &str) -> Result<TerminalConnection, String> {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 28,
                cols: 100,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())?;
        #[cfg(windows)]
        let mut command = CommandBuilder::new("powershell.exe");
        #[cfg(unix)]
        let mut command = CommandBuilder::new(
            std::env::var("SHELL")
                .ok()
                .filter(|s| std::path::Path::new(s).is_absolute())
                .unwrap_or("/bin/sh".into()),
        );
        #[cfg(windows)]
        command.args(["-NoLogo", "-NoProfile"]);
        #[cfg(unix)]
        command.arg("-i");
        command.cwd(cwd);
        command.env("TERM", "xterm-256color");
        let mut child = pair
            .slave
            .spawn_command(command)
            .map_err(|e| e.to_string())?;
        drop(pair.slave);
        let pid = child.process_id();
        let scope = match self.contain(pid) {
            Ok(scope) => scope,
            Err(error) => {
                let _ = child.kill();
                return Err(error);
            }
        };
        let child = Arc::new(Mutex::new(child));
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let mut writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        let master = pair.master;
        let (input, mut input_rx) = mpsc::channel(64);
        let (output_tx, output) = mpsc::channel(64);
        let cancel = Arc::new(AtomicBool::new(false));
        let reader_cancel = cancel.clone();
        let monitor_cancel = cancel.clone();
        let out = output_tx.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        // Bounded channel; a stalled peer never leaves a thread blocked on send.
                        let mut frame = TerminalOutput::Data(buf[..n].to_vec());
                        loop {
                            match out.try_send(frame) {
                                Ok(()) => break,
                                Err(mpsc::error::TrySendError::Full(v)) => {
                                    if reader_cancel.load(Ordering::SeqCst) {
                                        return;
                                    }
                                    frame = v;
                                    std::thread::sleep(Duration::from_millis(10));
                                }
                                Err(_) => return,
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = out.try_send(TerminalOutput::Exit);
        });
        let worker_child = child.clone();
        let worker_cancel = cancel.clone();
        let monitor_scope = scope.clone();
        std::thread::spawn(move || {
            loop {
                if worker_cancel.load(Ordering::SeqCst) {
                    break;
                }
                match input_rx.try_recv() {
                    Ok(TerminalInput::Write(bytes)) => {
                        if let Err(e) = writer.write_all(&bytes).and_then(|_| writer.flush()) {
                            let _ = output_tx.try_send(TerminalOutput::Error(e.to_string()));
                            break;
                        }
                    }
                    Ok(TerminalInput::Resize(cols, rows)) => {
                        let _ = master.resize(PtySize {
                            rows: rows.clamp(2, 500),
                            cols: cols.clamp(2, 500),
                            pixel_width: 0,
                            pixel_height: 0,
                        });
                    }
                    Err(mpsc::error::TryRecvError::Empty) => {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    Err(_) => break,
                }
            }
            worker_cancel.store(true, Ordering::SeqCst);
            scope.terminate();
            let _ = worker_child.lock().unwrap().kill();
            let _ = worker_child.lock().unwrap().wait();
        });
        // Monitor kills independently even if a pipe write is stalled.
        std::thread::spawn(move || loop {
            if monitor_cancel.load(Ordering::SeqCst) {
                monitor_scope.terminate();
                let _ = child.lock().unwrap().kill();
                break;
            }
            if child.lock().unwrap().try_wait().ok().flatten().is_some() {
                monitor_cancel.store(true, Ordering::SeqCst);
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        });
        Ok(TerminalConnection {
            input,
            output,
            cancel,
        })
    }
    fn execute(
        &self,
        request: ExecRequest,
        cancel: Arc<AtomicBool>,
    ) -> Pin<Box<dyn Future<Output = ExecResult> + Send>> {
        let shell = self.clone();
        Box::pin(async move {
            if cancel.load(Ordering::SeqCst) {
                return ExecResult {
                    cancelled: true,
                    ..Default::default()
                };
            }
            #[cfg(windows)]
            let mut cmd = Command::new("powershell.exe");
            #[cfg(windows)]
            {
                cmd.args([
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    &request.command,
                ]);
                cmd.creation_flags(0x08000200);
            }
            #[cfg(unix)]
            let mut cmd = Command::new("/bin/sh");
            #[cfg(unix)]
            {
                cmd.args(["-c", &request.command]);
                cmd.process_group(0);
            }
            cmd.kill_on_drop(true)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());
            if let Some(cwd) = request.cwd {
                cmd.current_dir(cwd);
            }
            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    return ExecResult {
                        error: Some(e.to_string()),
                        ..Default::default()
                    }
                }
            };
            let pid = child.id();
            let scope = match shell.contain(pid) {
                Ok(scope) => scope,
                Err(error) => {
                    let _ = child.start_kill();
                    let _ = child.wait().await;
                    return ExecResult {
                        error: Some(error),
                        ..Default::default()
                    };
                }
            };
            let total = Arc::new(AtomicUsize::new(0));
            let truncated = Arc::new(AtomicBool::new(false));
            let mut stdout_task = tokio::spawn(capture(
                child.stdout.take().unwrap(),
                total.clone(),
                truncated.clone(),
            ));
            let mut stderr_task = tokio::spawn(capture(
                child.stderr.take().unwrap(),
                total,
                truncated.clone(),
            ));
            let deadline = tokio::time::sleep(Duration::from_secs(request.timeout_seconds));
            tokio::pin!(deadline);
            let mut tick = tokio::time::interval(Duration::from_millis(100));
            let mut result = ExecResult::default();
            loop {
                tokio::select! {status=child.wait()=>{match status{Ok(s)=>result.exit_code=s.code(),Err(e)=>result.error=Some(e.to_string())}break;},_= &mut deadline=>{result.timed_out=true;break;},_=tick.tick()=>{if cancel.load(Ordering::SeqCst){result.cancelled=true;break;}}}
            }
            // Terminate descendants even when the parent exited (e.g. a background child inherited stdout).
            scope.terminate();
            let _ = child.start_kill();
            let _ = child.wait().await;
            let stdout = tokio::time::timeout(Duration::from_secs(2), &mut stdout_task).await;
            let stderr = tokio::time::timeout(Duration::from_secs(2), &mut stderr_task).await;
            if stdout.is_err() {
                stdout_task.abort();
                result.truncated = true;
            }
            if stderr.is_err() {
                stderr_task.abort();
                result.truncated = true;
            }
            result.stdout =
                String::from_utf8_lossy(&stdout.ok().and_then(Result::ok).unwrap_or_default())
                    .into_owned();
            result.stderr =
                String::from_utf8_lossy(&stderr.ok().and_then(Result::ok).unwrap_or_default())
                    .into_owned();
            result.truncated |= truncated.load(Ordering::SeqCst);
            result.cancelled |= cancel.load(Ordering::SeqCst);
            result
        })
    }
}
async fn capture<R: AsyncRead + Unpin>(
    mut reader: R,
    total: Arc<AtomicUsize>,
    truncated: Arc<AtomicBool>,
) -> Vec<u8> {
    let mut output = Vec::new();
    let mut buf = [0u8; 8192];
    while let Ok(n) = reader.read(&mut buf).await {
        if n == 0 {
            break;
        }
        let previous = total.fetch_add(n, Ordering::SeqCst);
        let keep = OUTPUT_LIMIT.saturating_sub(previous).min(n);
        output.extend_from_slice(&buf[..keep]);
        if keep < n {
            truncated.store(true, Ordering::SeqCst);
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn executes_real_shell_and_preserves_exit_code() {
        let r = NativeShell::default()
            .execute(
                ExecRequest {
                    command: "echo tether-ok; exit 7".into(),
                    cwd: None,
                    timeout_seconds: 10,
                },
                Arc::new(AtomicBool::new(false)),
            )
            .await;
        assert!(r.stdout.contains("tether-ok"));
        assert_eq!(r.exit_code, Some(7));
    }
    #[tokio::test]
    async fn timeout_stops_command() {
        #[cfg(windows)]
        let command = "Start-Sleep -Seconds 10";
        #[cfg(unix)]
        let command = "sleep 10";
        let r = NativeShell::default()
            .execute(
                ExecRequest {
                    command: command.into(),
                    cwd: None,
                    timeout_seconds: 1,
                },
                Arc::new(AtomicBool::new(false)),
            )
            .await;
        assert!(r.timed_out);
    }
    #[tokio::test]
    async fn command_output_is_bounded_without_blocking_the_child() {
        #[cfg(windows)]
        let command = "Write-Output ('x' * 1100000)";
        #[cfg(unix)]
        let command = "yes x | head -c 1100000";
        let result = NativeShell::default()
            .execute(
                ExecRequest {
                    command: command.into(),
                    cwd: None,
                    timeout_seconds: 10,
                },
                Arc::new(AtomicBool::new(false)),
            )
            .await;
        assert!(result.error.is_none(), "{:?}", result.error);
        assert_eq!(result.exit_code, Some(0));
        assert!(result.truncated);
        assert!(result.stdout.len() + result.stderr.len() <= OUTPUT_LIMIT);
    }
    #[tokio::test]
    async fn caller_cancellation_terminates_command() {
        #[cfg(windows)]
        let command = "Start-Sleep -Seconds 20";
        #[cfg(unix)]
        let command = "sleep 20";
        let cancel = Arc::new(AtomicBool::new(false));
        let trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            trigger.store(true, Ordering::SeqCst);
        });
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            NativeShell::default().execute(
                ExecRequest {
                    command: command.into(),
                    cwd: None,
                    timeout_seconds: 30,
                },
                cancel,
            ),
        )
        .await
        .unwrap();
        assert!(result.cancelled);
        assert!(!result.timed_out);
    }
}
