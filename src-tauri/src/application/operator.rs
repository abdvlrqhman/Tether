//! Operator use cases. Credentials remain here; adapters only transport requests.
use super::ports::{OperatorSocket, RemoteTransport};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use tokio::sync::mpsc;

#[derive(Deserialize)]
struct Invitation {
    url: String,
    invite: String,
}
#[derive(Clone, Default, Serialize)]
pub struct RemoteView {
    pub status: String,
    pub url: String,
    pub operator: String,
    pub expires_at: Option<u64>,
    pub os: Option<String>,
    pub error: Option<String>,
}
#[derive(Default)]
struct Remote {
    view: RemoteView,
    id: String,
    proof: String,
    token: Option<String>,
    terminal: Option<(String, mpsc::Sender<String>)>,
    opening: Option<String>,
}
#[derive(Clone)]
pub struct OperatorService {
    state: Arc<Mutex<Remote>>,
    transport: Arc<dyn RemoteTransport>,
    sequence: Arc<AtomicU64>,
}
pub fn validate_url(input: &str) -> Result<String, String> {
    let url = url::Url::parse(input).map_err(|_| "Enter a valid HTTPS endpoint")?;
    let local = matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    if !(url.scheme() == "https" || url.scheme() == "http" && local)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
        || url.host_str().is_none()
    {
        return Err(
            "Use HTTPS, or HTTP on loopback only, without a path, credentials, query, or fragment"
                .into(),
        );
    }
    Ok(url.as_str().trim_end_matches('/').into())
}
impl OperatorService {
    pub fn new(transport: Arc<dyn RemoteTransport>) -> Self {
        Self {
            state: Arc::new(Mutex::new(Remote::default())),
            transport,
            sequence: Arc::new(AtomicU64::new(0)),
        }
    }
    pub fn view(&self) -> RemoteView {
        self.state.lock().unwrap().view.clone()
    }
    pub async fn begin(&self, invitation: String, operator: String) -> Result<RemoteView, String> {
        if invitation.len() > 32768 {
            return Err("Invitation is too large".into());
        }
        let invitation: Invitation = serde_json::from_str(&invitation)
            .map_err(|_| "Paste the complete invitation JSON from the host")?;
        let url = validate_url(&invitation.url)?;
        if invitation.invite.len() != 43 {
            return Err("Invitation secret has an invalid length".into());
        }
        if operator.trim().is_empty() || operator.len() > 80 {
            return Err("Enter an operator name of 1–80 characters".into());
        }
        let receipt = self
            .transport
            .request(
                url.clone(),
                "/v1/pair".into(),
                None,
                Some(json!({"invite":invitation.invite,"operator":operator})),
            )
            .await?;
        let id = receipt["id"]
            .as_str()
            .filter(|s| s.len() == 43)
            .ok_or("Invalid pairing response")?
            .to_string();
        let proof = receipt["proof"]
            .as_str()
            .filter(|s| s.len() == 43)
            .ok_or("Invalid pairing proof")?
            .to_string();
        let view = RemoteView {
            status: "pending".into(),
            url,
            operator,
            ..Default::default()
        };
        *self.state.lock().unwrap() = Remote {
            view: view.clone(),
            id,
            proof,
            token: None,
            terminal: None,
            opening: None,
        };
        Ok(view)
    }
    pub async fn poll(&self) -> Result<RemoteView, String> {
        let (view, id, proof, token) = {
            let s = self.state.lock().unwrap();
            (
                s.view.clone(),
                s.id.clone(),
                s.proof.clone(),
                s.token.clone(),
            )
        };
        if !["pending", "connected"].contains(&view.status.as_str()) {
            return Ok(view);
        }
        let (path, credential) = if let Some(token) = token {
            ("/v1/session".into(), token)
        } else {
            (format!("/v1/pair/{id}"), proof)
        };
        let data = self
            .transport
            .request(view.url, path, Some(credential), None)
            .await;
        let mut s = self.state.lock().unwrap();
        if s.id != id {
            return Ok(s.view.clone());
        }
        match data {
            Ok(data) if data.get("session").is_some() => {
                s.view.error = None;
                s.view.expires_at = data["session"]["expires_at"].as_u64()
            }
            Ok(data) => {
                s.view.error = None;
                let status = data["status"].as_str().unwrap_or("error");
                if status == "approved" {
                    s.token = Some(
                        data["token"]
                            .as_str()
                            .filter(|s| s.len() == 43)
                            .ok_or("Invalid session token")?
                            .into(),
                    );
                    s.view.status = "connected".into();
                    s.view.expires_at = data["expires_at"].as_u64();
                    s.view.os = data["os"].as_str().map(String::from);
                } else if status != "pending" {
                    s.view.status = status.into();
                    s.token = None;
                    s.terminal = None;
                }
            }
            Err(error) => {
                if error.starts_with("Unauthorized:") {
                    s.view.status = "error".into();
                    s.token = None;
                    s.terminal = None;
                    s.opening = None;
                }
                s.view.error = Some(error);
            }
        }
        Ok(s.view.clone())
    }
    pub fn disconnect(&self) {
        *self.state.lock().unwrap() = Remote::default();
    }
    pub fn close_terminal(&self) {
        let mut s = self.state.lock().unwrap();
        s.terminal = None;
        s.opening = None;
    }
    pub fn send(&self, message: String) -> Result<(), String> {
        if message.len() > 32768 {
            return Err("Terminal frame too large".into());
        }
        self.state
            .lock()
            .unwrap()
            .terminal
            .as_ref()
            .ok_or("Terminal is closed")?
            .1
            .try_send(message)
            .map_err(|_| "Terminal is busy or disconnected".into())
    }
    pub async fn open_terminal(
        &self,
    ) -> Result<(String, mpsc::Receiver<serde_json::Value>), String> {
        let (url, token, id, terminal_id) = {
            let mut s = self.state.lock().unwrap();
            if s.terminal.is_some() || s.opening.is_some() {
                return Err("A terminal is already open".into());
            }
            let token = s.token.clone().ok_or("No approved connection")?;
            let terminal_id = format!("{}:{}", s.id, self.sequence.fetch_add(1, Ordering::SeqCst));
            s.opening = Some(terminal_id.clone());
            (s.view.url.clone(), token, s.id.clone(), terminal_id)
        };
        let connection = self.transport.terminal(url, token).await;
        let mut s = self.state.lock().unwrap();
        if s.id != id || s.token.is_none() || s.opening.as_deref() != Some(&terminal_id) {
            return Err("Connection changed".into());
        }
        s.opening = None;
        let OperatorSocket { input, output } = connection?;
        s.terminal = Some((terminal_id.clone(), input));
        Ok((terminal_id, output))
    }
    pub fn terminal_closed(&self, id: &str) {
        let mut s = self.state.lock().unwrap();
        if s.terminal
            .as_ref()
            .is_some_and(|(terminal_id, _)| terminal_id == id)
        {
            s.terminal = None;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_validation() {
        assert!(validate_url("http://127.0.0.1:1234").is_ok());
        assert!(validate_url("https://foo.trycloudflare.com").is_ok());
        for s in [
            "http://example.com",
            "https://user:pass@example.com",
            "https://example.com/a",
            "https://example.com?token=x",
            "file:///tmp/x",
        ] {
            assert!(validate_url(s).is_err(), "{s}");
        }
    }
}
