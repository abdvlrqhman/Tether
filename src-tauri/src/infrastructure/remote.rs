//! Native HTTP/WebSocket transport; no session policy or desktop dependencies.
use crate::application::ports::{OperatorSocket, RemoteFuture, RemoteTransport};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{client::IntoClientRequest, protocol::WebSocketConfig, Message},
};
pub struct NativeRemoteTransport {
    http: reqwest::Client,
}
impl Default for NativeRemoteTransport {
    fn default() -> Self {
        Self::new()
    }
}
impl NativeRemoteTransport {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(12))
                .build()
                .expect("HTTP client"),
        }
    }
}
impl RemoteTransport for NativeRemoteTransport {
    fn revoke(&self, url: String, token: String) -> RemoteFuture<Value> {
        let http = self.http.clone();
        Box::pin(async move {
            let response = http
                .delete(url)
                .bearer_auth(token)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let status = response.status();
            let data: Value = response
                .json()
                .await
                .map_err(|_| "Invalid revocation response")?;
            if !status.is_success() {
                if status == reqwest::StatusCode::UNAUTHORIZED {
                    return Err(format!(
                        "Unauthorized: {}",
                        data["error"].as_str().unwrap_or("Session rejected")
                    ));
                }
                return Err(data["error"]
                    .as_str()
                    .unwrap_or("Host rejected revocation")
                    .into());
            }
            Ok(data)
        })
    }
    fn request(
        &self,
        base: String,
        path: String,
        token: Option<String>,
        body: Option<Value>,
    ) -> RemoteFuture<Value> {
        let http = self.http.clone();
        Box::pin(async move {
            let mut request = if body.is_some() {
                http.post(format!("{base}{path}"))
            } else {
                http.get(format!("{base}{path}"))
            };
            if let Some(token) = token {
                request = request.bearer_auth(token);
            }
            if let Some(body) = body {
                request = request.json(&body);
            }
            let mut response = request
                .send()
                .await
                .map_err(|e| format!("Cannot reach host: {e}"))?;
            let status = response.status();
            let mut bytes = Vec::new();
            while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
                if bytes.len() + chunk.len() > 3 * 1024 * 1024 {
                    return Err("Host response is too large".into());
                }
                bytes.extend_from_slice(&chunk);
            }
            let data: Value =
                serde_json::from_slice(&bytes).map_err(|_| "Host returned an invalid response")?;
            if !status.is_success() {
                if status == reqwest::StatusCode::UNAUTHORIZED {
                    return Err(format!(
                        "Unauthorized: {}",
                        data["error"].as_str().unwrap_or("Session rejected")
                    ));
                }
                return Err(data["error"]
                    .as_str()
                    .unwrap_or("Host rejected the request")
                    .into());
            }
            Ok(data)
        })
    }
    fn terminal(&self, base: String, token: String) -> RemoteFuture<OperatorSocket> {
        Box::pin(async move {
            let url = format!(
                "{}/v1/terminal",
                base.replacen("https://", "wss://", 1)
                    .replacen("http://", "ws://", 1)
            );
            let mut request = url.into_client_request().map_err(|e| e.to_string())?;
            request.headers_mut().insert(
                "authorization",
                format!("Bearer {token}")
                    .parse()
                    .map_err(|_| "Invalid token")?,
            );
            let config = WebSocketConfig::default()
                .max_message_size(Some(32768))
                .max_frame_size(Some(32768));
            let (socket, _) = tokio::time::timeout(
                Duration::from_secs(15),
                connect_async_with_config(request, Some(config), false),
            )
            .await
            .map_err(|_| "Terminal connection timed out")?
            .map_err(|e| e.to_string())?;
            let (input, mut input_rx) = mpsc::channel::<String>(64);
            let (output_tx, output) = mpsc::channel(64);
            tokio::spawn(async move {
                let (mut sink, mut stream) = socket.split();
                loop {
                    tokio::select! {
                     message=input_rx.recv()=>{let Some(message)=message else{break;};if !matches!(tokio::time::timeout(Duration::from_secs(3),sink.send(Message::Text(message.into()))).await,Ok(Ok(()))){break;}},
                     message=stream.next()=>match message{Some(Ok(Message::Text(text)))=>{let Ok(data)=serde_json::from_str::<Value>(&text)else{break;};if !matches!(tokio::time::timeout(Duration::from_secs(3),output_tx.send(data)).await,Ok(Ok(()))){break;}},Some(Ok(Message::Ping(_)))=>{if !matches!(tokio::time::timeout(Duration::from_secs(3),sink.flush()).await,Ok(Ok(()))){break;}},Some(Ok(Message::Pong(_)))=>{},_=>break}
                    }
                }
                let _ = tokio::time::timeout(Duration::from_secs(1), sink.close()).await;
                let _ = output_tx.try_send(json!({"type":"closed"}));
            });
            Ok(OperatorSocket { input, output })
        })
    }
}
