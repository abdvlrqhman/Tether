//! Policy and state transitions. No desktop, network, filesystem, or process APIs.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub const INVITE_TTL: u64 = 600;
pub const PAIR_TTL: u64 = 300;
pub const MAX_PENDING: usize = 8;

#[derive(Clone)]
pub struct SecretHash([u8; 32]);
impl SecretHash {
    pub fn new(secret: &str) -> Self {
        Self(Sha256::digest(secret.as_bytes()).into())
    }
    pub fn matches(&self, secret: &str) -> bool {
        self.0.ct_eq(&Self::new(secret).0).into()
    }
}

#[derive(Clone, Serialize)]
pub struct AuditEvent {
    pub at: u64,
    pub kind: String,
    pub actor: String,
    pub detail: String,
}

#[derive(Clone, Serialize)]
pub struct PendingView {
    pub id: String,
    pub operator: String,
    pub created_at: u64,
}

pub struct Pairing {
    pub view: PendingView,
    pub proof: SecretHash,
    pub status: String,
    pub token: Option<String>,
    pub expires_at: u64,
}

#[derive(Clone, Serialize)]
pub struct SessionView {
    pub id: String,
    pub operator: String,
    pub expires_at: u64,
}
pub struct Session {
    pub view: SessionView,
    pub token: SecretHash,
}

#[derive(Default)]
pub struct Host {
    pub generation: u64,
    pub running: bool,
    pub url: String,
    pub local_url: String,
    pub working_directory: String,
    pub invite: Option<(String, SecretHash, u64)>,
    pub pairings: Vec<Pairing>,
    pub session: Option<Session>,
    pub audit: Vec<AuditEvent>,
    pub tunnel: String,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct HostView {
    pub running: bool,
    pub url: String,
    pub local_url: String,
    pub working_directory: String,
    pub invite: Option<String>,
    pub invite_expires_at: Option<u64>,
    pub pending: Vec<PendingView>,
    pub session: Option<SessionView>,
    pub audit: Vec<AuditEvent>,
    pub tunnel: String,
    pub error: Option<String>,
}

impl Host {
    pub fn view(&self, now: u64) -> HostView {
        HostView {
            running: self.running,
            url: self.url.clone(),
            local_url: self.local_url.clone(),
            working_directory: self.working_directory.clone(),
            invite: self
                .invite
                .as_ref()
                .filter(|v| now < v.2)
                .map(|v| v.0.clone()),
            invite_expires_at: self.invite.as_ref().map(|v| v.2),
            pending: self
                .pairings
                .iter()
                .filter(|p| p.status == "pending" && now < p.expires_at)
                .map(|p| p.view.clone())
                .collect(),
            session: self
                .session
                .as_ref()
                .filter(|s| now < s.view.expires_at)
                .map(|s| s.view.clone()),
            audit: self.audit.clone(),
            tunnel: self.tunnel.clone(),
            error: self.error.clone(),
        }
    }
    pub fn authenticate(&self, token: &str, now: u64) -> Result<SessionView, String> {
        if !self.running {
            return Err("Host is stopped".into());
        }
        self.session
            .as_ref()
            .filter(|s| now < s.view.expires_at && s.token.matches(token))
            .map(|s| s.view.clone())
            .ok_or_else(|| "Session is invalid, revoked, or expired".into())
    }
    pub fn active(&self, id: &str, now: u64) -> bool {
        self.running
            && self
                .session
                .as_ref()
                .is_some_and(|s| s.view.id == id && now < s.view.expires_at)
    }
    pub fn request_pairing(
        &mut self,
        invite: &str,
        operator: &str,
        id: String,
        proof: String,
        now: u64,
    ) -> Result<(), String> {
        if !self.running
            || !self
                .invite
                .as_ref()
                .is_some_and(|v| now < v.2 && v.1.matches(invite))
        {
            return Err("Invitation is invalid or expired".into());
        }
        if operator.is_empty() || operator.len() > 80 || operator.chars().any(char::is_control) {
            return Err("Operator name must be 1–80 characters without control characters".into());
        }
        if self
            .session
            .as_ref()
            .is_some_and(|s| now < s.view.expires_at)
        {
            return Err("A session is already active".into());
        }
        self.pairings.retain(|p| now < p.expires_at);
        if self.pairings.len() >= MAX_PENDING {
            return Err("Too many pairing requests".into());
        }
        self.pairings.push(Pairing {
            view: PendingView {
                id,
                operator: operator.into(),
                created_at: now,
            },
            proof: SecretHash::new(&proof),
            status: "pending".into(),
            token: None,
            expires_at: now + PAIR_TTL,
        });
        Ok(())
    }
    pub fn approve(
        &mut self,
        id: &str,
        token: String,
        minutes: u64,
        now: u64,
    ) -> Result<SessionView, String> {
        if ![15, 30, 60, 120].contains(&minutes) {
            return Err("Choose 15, 30, 60, or 120 minutes".into());
        }
        if !self.running
            || self
                .session
                .as_ref()
                .is_some_and(|s| now < s.view.expires_at)
        {
            return Err("Host must be running without an active session".into());
        }
        let pair = self
            .pairings
            .iter_mut()
            .find(|p| p.view.id == id && p.status == "pending" && now < p.expires_at)
            .ok_or("Pairing request expired")?;
        let view = SessionView {
            id: id.into(),
            operator: pair.view.operator.clone(),
            expires_at: now + minutes * 60,
        };
        pair.status = "approved".into();
        pair.token = Some(token.clone());
        self.session = Some(Session {
            view: view.clone(),
            token: SecretHash::new(&token),
        });
        self.invite = None;
        for p in &mut self.pairings {
            if p.view.id != id {
                p.status = "denied".into();
            }
        }
        Ok(view)
    }
    pub fn revoke(&mut self) {
        self.session = None;
        for p in &mut self.pairings {
            p.status = "revoked".into();
            p.token = None;
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ExecRequest {
    pub command: String,
    pub cwd: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}
fn default_timeout() -> u64 {
    60
}
impl ExecRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.command.trim().is_empty()
            || self.command.len() > 16384
            || self.command.contains('\0')
        {
            return Err("Command must contain 1–16384 bytes and no null bytes".into());
        }
        if !(1..=300).contains(&self.timeout_seconds) {
            return Err("Timeout must be between 1 and 300 seconds".into());
        }
        Ok(())
    }
}
#[derive(Clone, Default, Serialize)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub cancelled: bool,
    pub truncated: bool,
    pub error: Option<String>,
}
#[derive(Clone, Serialize)]
pub struct JobView {
    pub id: String,
    pub status: String,
    pub result: Option<ExecResult>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn host() -> Host {
        Host {
            running: true,
            invite: Some(("invite".into(), SecretHash::new("invite"), 700)),
            ..Host::default()
        }
    }
    #[test]
    fn invitation_expiry_and_bad_secret_rejected() {
        let mut h = host();
        assert!(h
            .request_pairing("bad", "dev", "id".into(), "proof".into(), 100)
            .is_err());
        assert!(h
            .request_pairing("invite", "dev", "id".into(), "proof".into(), 700)
            .is_err());
    }
    #[test]
    fn approval_is_required_and_invitation_is_consumed() {
        let mut h = host();
        h.request_pairing("invite", "dev", "id".into(), "proof".into(), 100)
            .unwrap();
        assert!(h.authenticate("token", 100).is_err());
        h.approve("id", "token".into(), 15, 100).unwrap();
        assert!(h.invite.is_none());
        assert!(h.authenticate("token", 999).is_ok());
        assert!(h.authenticate("token", 1000).is_err());
    }
    #[test]
    fn revocation_erases_delivery_token() {
        let mut h = host();
        h.request_pairing("invite", "dev", "id".into(), "proof".into(), 100)
            .unwrap();
        h.approve("id", "token".into(), 15, 100).unwrap();
        h.revoke();
        assert!(h.authenticate("token", 101).is_err());
        assert!(h.pairings[0].token.is_none());
    }
    #[test]
    fn only_one_approved_session() {
        let mut h = host();
        for id in ["a", "b"] {
            h.request_pairing("invite", "dev", id.into(), "proof".into(), 100)
                .unwrap();
        }
        h.approve("a", "token".into(), 15, 100).unwrap();
        assert!(h.approve("b", "second".into(), 15, 100).is_err());
        assert_eq!(h.pairings[1].status, "denied");
    }
    #[test]
    fn validate_limits() {
        assert!(ExecRequest {
            command: "whoami".into(),
            cwd: None,
            timeout_seconds: 301
        }
        .validate()
        .is_err());
        assert!(SecretHash::new("secret").matches("secret"));
        assert!(!SecretHash::new("secret").matches("different"));
    }
}
