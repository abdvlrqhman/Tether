//! Agent workflows shared by the MCP adapter. All remote authority comes from the host API.
use super::{
    operator::validate_url,
    ports::{Clock, RemoteTransport, SessionStore},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
#[derive(Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub url: String,
    pub token: String,
    pub expires_at: u64,
    pub os: Option<String>,
    pub operator: String,
}
#[derive(Clone)]
struct Pending {
    url: String,
    id: String,
    proof: String,
    operator: String,
}
#[derive(Clone)]
pub struct AgentService {
    transport: Arc<dyn RemoteTransport>,
    store: Arc<dyn SessionStore>,
    clock: Arc<dyn Clock>,
    pending: Arc<Mutex<Option<Pending>>>,
}
impl AgentService {
    pub fn new(
        transport: Arc<dyn RemoteTransport>,
        store: Arc<dyn SessionStore>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            transport,
            store,
            clock,
            pending: Arc::new(Mutex::new(None)),
        }
    }
    fn session(&self) -> Result<AgentSession, String> {
        let s = self.store.load()?;
        validate_url(&s.url)?;
        if s.token.len() != 43 || self.clock.now() >= s.expires_at {
            return Err("Session expired. Request and approve a new pairing.".into());
        }
        Ok(s)
    }
    pub async fn pair(&self, path: String, operator: String) -> Result<Value, String> {
        if operator.trim().is_empty() || operator.len() > 80 {
            return Err("Use an operator name of 1–80 characters".into());
        }
        let invitation: Value = serde_json::from_str(&self.store.invitation(&path)?)
            .map_err(|_| "Invalid invitation JSON")?;
        let url = validate_url(invitation["url"].as_str().ok_or("Invitation has no URL")?)?;
        let invite = invitation["invite"]
            .as_str()
            .filter(|s| s.len() == 43)
            .ok_or("Invalid invitation secret")?;
        let result = self
            .transport
            .request(
                url.clone(),
                "/v1/pair".into(),
                None,
                Some(json!({"invite":invite,"operator":operator})),
            )
            .await?;
        let id = result["id"]
            .as_str()
            .filter(|s| s.len() == 43)
            .ok_or("Invalid pairing response")?
            .to_string();
        let proof = result["proof"]
            .as_str()
            .filter(|s| s.len() == 43)
            .ok_or("Invalid pairing proof")?
            .to_string();
        *self.pending.lock().unwrap() = Some(Pending {
            url,
            id: id.clone(),
            proof,
            operator,
        });
        Ok(
            json!({"status":"pending","request_id":id,"next":"Ask the host to approve this request in Tether, then call tether_pair_status."}),
        )
    }
    pub async fn poll_pair(&self) -> Result<Value, String> {
        let p = self
            .pending
            .lock()
            .unwrap()
            .clone()
            .ok_or("No pairing request. Call tether_pair first.")?;
        let mut result = self
            .transport
            .request(
                p.url.clone(),
                format!("/v1/pair/{}", p.id),
                Some(p.proof),
                None,
            )
            .await?;
        if result["status"] == "approved" {
            let session = AgentSession {
                url: p.url,
                token: result["token"]
                    .as_str()
                    .filter(|s| s.len() == 43)
                    .ok_or("Invalid token")?
                    .into(),
                expires_at: result["expires_at"]
                    .as_u64()
                    .ok_or("Missing session expiry")?,
                os: result["os"].as_str().map(String::from),
                operator: p.operator,
            };
            self.store.save(&session)?;
            *self.pending.lock().unwrap() = None;
        }
        result
            .as_object_mut()
            .ok_or("Invalid pairing response")?
            .remove("token");
        Ok(result)
    }
    pub async fn status(&self) -> Result<Value, String> {
        let s = self.session()?;
        self.transport
            .request(s.url, "/v1/session".into(), Some(s.token), None)
            .await
    }
    pub async fn execute(
        &self,
        command: String,
        cwd: Option<String>,
        timeout: u64,
    ) -> Result<Value, String> {
        let request = crate::domain::ExecRequest {
            command,
            cwd,
            timeout_seconds: timeout,
        };
        request.validate()?;
        let s = self.session()?;
        self.transport
            .request(
                s.url,
                "/v1/exec".into(),
                Some(s.token),
                Some(serde_json::to_value(request).map_err(|e| e.to_string())?),
            )
            .await
    }
    pub async fn job(&self, id: String, cancel: bool) -> Result<Value, String> {
        if id.len() != 43
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err("Invalid job id".into());
        }
        let s = self.session()?;
        if cancel {
            self.transport
                .revoke(format!("{}/v1/jobs/{id}", s.url), s.token)
                .await
        } else {
            self.transport
                .request(s.url, format!("/v1/jobs/{id}"), Some(s.token), None)
                .await
        }
    }
    pub async fn logout(&self) -> Result<Value, String> {
        let session = self.store.load().ok();
        let result = if let Some(s) = session {
            validate_url(&s.url)?;
            if self.clock.now() < s.expires_at {
                self.transport
                    .revoke(format!("{}/v1/session", s.url), s.token)
                    .await
                    .map(|_| ())
            } else {
                Ok(())
            }
        } else {
            Ok(())
        };
        self.store.remove()?;
        *self.pending.lock().unwrap() = None;
        result.map(|_|json!({"status":"logged_out"})).map_err(|e|format!("Local credentials removed; host revocation failed. Ask the host to end access: {e}"))
    }
}
