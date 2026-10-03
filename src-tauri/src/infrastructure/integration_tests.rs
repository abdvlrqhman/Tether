//! Real HTTP, shell, and WebSocket checks against an ephemeral loopback listener.
use super::*;
use crate::{
    application::{ports::AuditSink, HostService},
    domain::AuditEvent,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{sync::Arc, time::Duration};
struct MemoryAudit;
impl AuditSink for MemoryAudit {
    fn append(&self, _: &AuditEvent) -> Result<(), String> {
        Ok(())
    }
}
#[tokio::test]
#[ignore = "Requires Internet and a verified cloudflared binary in TETHER_CLOUDFLARED_PATH"]
async fn real_quick_tunnel_reaches_authenticated_host() {
    let binary = std::env::var_os("TETHER_CLOUDFLARED_PATH")
        .expect("Set TETHER_CLOUDFLARED_PATH to a verified binary");
    let (service, base, server) = fixture().await;
    let mut child =
        super::tunnel::spawn(std::path::Path::new(&binary), &base, service.clone()).unwrap();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .unwrap();
    let mut last_error = String::from("Cloudflare connection not yet registered");
    let result = tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let snapshot = service.snapshot();
            if snapshot.tunnel == "online" {
                match client.get(format!("{}/health", snapshot.url)).send().await {
                    Ok(response) if response.status().is_success() => {
                        let data: Value = response.json().await.unwrap();
                        assert_eq!(data["service"], "tether");
                        let denied = client
                            .get(format!("{}/v1/session", snapshot.url))
                            .send()
                            .await
                            .unwrap();
                        assert_eq!(denied.status(), 401);
                        let token = approve(&service, &snapshot.url).await;
                        let command = if cfg!(windows) {
                            "Write-Output 'cloudflare-command-ok'; exit 7"
                        } else {
                            "printf cloudflare-command-ok; exit 7"
                        };
                        let job: Value = client
                            .post(format!("{}/v1/exec", snapshot.url))
                            .bearer_auth(&token)
                            .json(&json!({"command":command,"timeout_seconds":15}))
                            .send()
                            .await
                            .unwrap()
                            .json()
                            .await
                            .unwrap();
                        let id = job["id"].as_str().unwrap();
                        loop {
                            let job: Value = client
                                .get(format!("{}/v1/jobs/{id}", snapshot.url))
                                .bearer_auth(&token)
                                .send()
                                .await
                                .unwrap()
                                .json()
                                .await
                                .unwrap();
                            if job["status"] == "finished" {
                                assert_eq!(job["result"]["exit_code"], 7);
                                assert!(job["result"]["stdout"]
                                    .as_str()
                                    .unwrap()
                                    .contains("cloudflare-command-ok"));
                                break;
                            }
                            tokio::time::sleep(Duration::from_millis(200)).await;
                        }
                        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
                        let mut request =
                            format!("{}/v1/terminal", snapshot.url.replace("https://", "wss://"))
                                .into_client_request()
                                .unwrap();
                        request
                            .headers_mut()
                            .insert("authorization", format!("Bearer {token}").parse().unwrap());
                        let (mut socket, _) =
                            tokio_tungstenite::connect_async(request).await.unwrap();
                        loop {
                            use tokio_tungstenite::tungstenite::Message;
                            match socket.next().await.unwrap().unwrap() {
                                Message::Text(text) => {
                                    let frame: Value = serde_json::from_str(&text).unwrap();
                                    assert_eq!(frame["type"], "output");
                                    break;
                                }
                                Message::Ping(_) => socket.flush().await.unwrap(),
                                other => panic!("Unexpected Cloudflare terminal frame: {other:?}"),
                            }
                        }
                        socket.close(None).await.unwrap();
                        service.revoke().unwrap();
                        assert_eq!(
                            client
                                .get(format!("{}/v1/session", snapshot.url))
                                .bearer_auth(&token)
                                .send()
                                .await
                                .unwrap()
                                .status(),
                            401
                        );
                        break;
                    }
                    Ok(response) => {
                        last_error = format!("Public health returned {}", response.status())
                    }
                    Err(error) => last_error = format!("Public health request failed: {error}"),
                }
            } else if let Some(error) = snapshot.error {
                last_error = error;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    })
    .await;
    service.stop();
    let _ = child.kill().await;
    server.abort();
    assert!(
        result.is_ok(),
        "Quick Tunnel did not become reachable: {last_error}"
    );
}
pub(crate) async fn fixture() -> (HostService, String, tokio::task::JoinHandle<()>) {
    let service = HostService::new(
        Arc::new(SystemClock),
        Arc::new(RandomSecrets),
        Arc::new(MemoryAudit),
        Arc::new(shell::NativeShell::default()),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    service
        .start(
            base.clone(),
            std::env::current_dir()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
            false,
        )
        .unwrap();
    let router = http::router(service.clone());
    let task = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (service, base, task)
}
async fn approve(service: &HostService, base: &str) -> String {
    let client = reqwest::Client::new();
    let invite = service.snapshot().invite.unwrap();
    let receipt: Value = client
        .post(format!("{base}/v1/pair"))
        .json(&json!({"invite":invite,"operator":"integration test"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = receipt["id"].as_str().unwrap();
    let proof = receipt["proof"].as_str().unwrap();
    let pending: Value = client
        .get(format!("{base}/v1/pair/{id}"))
        .bearer_auth(proof)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(pending["status"], "pending");
    assert!(pending["token"].is_null());
    service.approve(id, 15).unwrap();
    let approved: Value = client
        .get(format!("{base}/v1/pair/{id}"))
        .bearer_auth(proof)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    approved["token"].as_str().unwrap().into()
}
#[tokio::test]
async fn native_operator_pairing_and_revocation() {
    let (host, url, server) = fixture().await;
    let operator = crate::application::operator::OperatorService::new(Arc::new(
        super::remote::NativeRemoteTransport::new(),
    ));
    operator
        .begin(
            json!({"url":url,"invite":host.snapshot().invite.unwrap()}).to_string(),
            "native operator".into(),
        )
        .await
        .unwrap();
    let id = host.snapshot().pending[0].id.clone();
    host.approve(&id, 15).unwrap();
    assert_eq!(operator.poll().await.unwrap().status, "connected");
    host.revoke().unwrap();
    assert_eq!(operator.poll().await.unwrap().status, "error");
    assert!(operator.open_terminal().await.is_err());
    host.stop();
    server.abort();
}
#[tokio::test]
async fn real_http_requires_approval_executes_and_revokes() {
    let (service, base, task) = fixture().await;
    let client = reqwest::Client::new();
    assert_eq!(
        client
            .post(format!("{base}/v1/exec"))
            .json(&json!({"command":"echo unsafe"}))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        client
            .post(format!("{base}/v1/pair"))
            .header("origin", "https://evil.example")
            .json(&json!({"invite":"fake","operator":"browser"}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        client
            .post(format!("{base}/v1/approve"))
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let token = approve(&service, &base).await;
    let job: Value = client
        .post(format!("{base}/v1/exec"))
        .bearer_auth(&token)
        .json(&json!({"command":"echo approved-shell; exit 7","timeout_seconds":10}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = job["id"].as_str().unwrap();
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let job: Value = client
                .get(format!("{base}/v1/jobs/{id}"))
                .bearer_auth(&token)
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            if job["status"] == "finished" {
                break job;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    assert!(result["result"]["stdout"]
        .as_str()
        .unwrap()
        .contains("approved-shell"));
    assert_eq!(result["result"]["exit_code"], 7);
    service.revoke().unwrap();
    assert_eq!(
        client
            .get(format!("{base}/v1/session"))
            .bearer_auth(&token)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    service.stop();
    task.abort();
}
#[tokio::test]
async fn real_pty_roundtrip_and_revocation_closes_socket() {
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message};
    let (service, base, task) = fixture().await;
    let token = approve(&service, &base).await;
    let mut request = format!("{}/v1/terminal", base.replace("http://", "ws://"))
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    socket
        .send(Message::Text(
            json!({"type":"resize","cols":120,"rows":30})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    socket
        .send(Message::Text(
            json!({"type":"input","data":STANDARD.encode(b"echo tether-pty-ok\r")})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let output = tokio::time::timeout(Duration::from_secs(15), async {
        let mut output = String::new();
        loop {
            match socket.next().await {
                Some(Ok(Message::Text(text))) => {
                    let frame: Value = serde_json::from_str(&text).unwrap();
                    if frame["type"] == "output" {
                        let bytes = STANDARD.decode(frame["data"].as_str().unwrap()).unwrap();
                        output.push_str(&String::from_utf8_lossy(&bytes));
                        if output.ends_with("\u{1b}[6n")
                            || String::from_utf8_lossy(&bytes).contains("\u{1b}[6n")
                        {
                            socket
                                .send(Message::Text(
                                    json!({"type":"input","data":STANDARD.encode(b"\x1b[1;1R")})
                                        .to_string()
                                        .into(),
                                ))
                                .await
                                .unwrap();
                        }
                        if output.contains("tether-pty-ok") {
                            break output;
                        }
                    }
                }
                Some(Ok(Message::Ping(_))) => {
                    socket.flush().await.unwrap();
                }
                other => panic!("unexpected terminal frame: {other:?}"),
            }
        }
    })
    .await
    .unwrap();
    assert!(output.contains("tether-pty-ok"));
    service.revoke().unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match socket.next().await {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    service.stop();
    task.abort();
}
