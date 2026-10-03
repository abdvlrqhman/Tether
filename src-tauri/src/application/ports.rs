//! Outward-facing contracts. Infrastructure implements these; use cases depend only on them.
use crate::domain::{AuditEvent, ExecRequest, ExecResult};
use std::{
    future::Future,
    pin::Pin,
    sync::{atomic::AtomicBool, Arc},
};
use tokio::sync::mpsc;

pub trait AuditSink: Send + Sync {
    fn append(&self, event: &AuditEvent) -> Result<(), String>;
}
pub trait Clock: Send + Sync {
    fn now(&self) -> u64;
}
pub trait SecretSource: Send + Sync {
    fn generate(&self) -> String;
}
pub enum TerminalInput {
    Write(Vec<u8>),
    Resize(u16, u16),
}
pub enum TerminalOutput {
    Data(Vec<u8>),
    Exit,
    Error(String),
}
pub struct TerminalConnection {
    pub input: mpsc::Sender<TerminalInput>,
    pub output: mpsc::Receiver<TerminalOutput>,
    pub cancel: Arc<AtomicBool>,
}
impl Drop for TerminalConnection {
    fn drop(&mut self) {
        self.cancel.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
pub trait Shell: Send + Sync {
    fn stop_all(&self);
    fn terminal(&self, cwd: &str) -> Result<TerminalConnection, String>;
    fn execute(
        &self,
        request: ExecRequest,
        cancel: Arc<AtomicBool>,
    ) -> Pin<Box<dyn Future<Output = ExecResult> + Send>>;
}

pub type RemoteFuture<T> = Pin<Box<dyn Future<Output = Result<T, String>> + Send>>;
pub struct OperatorSocket {
    pub input: mpsc::Sender<String>,
    pub output: mpsc::Receiver<serde_json::Value>,
}
pub trait RemoteTransport: Send + Sync {
    fn request(
        &self,
        base: String,
        path: String,
        token: Option<String>,
        body: Option<serde_json::Value>,
    ) -> RemoteFuture<serde_json::Value>;
    fn terminal(&self, base: String, token: String) -> RemoteFuture<OperatorSocket>;
    fn revoke(&self, base: String, token: String) -> RemoteFuture<serde_json::Value>;
}

pub trait SessionStore: Send + Sync {
    fn load(&self) -> Result<super::agent::AgentSession, String>;
    fn save(&self, session: &super::agent::AgentSession) -> Result<(), String>;
    fn remove(&self) -> Result<(), String>;
    fn invitation(&self, path: &str) -> Result<String, String>;
}
