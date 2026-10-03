use super::*;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
struct TestClock(AtomicU64);
impl Clock for TestClock {
    fn now(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
struct TestSecrets(AtomicU64);
impl SecretSource for TestSecrets {
    fn generate(&self) -> String {
        format!("secret-{}", self.0.fetch_add(1, Ordering::SeqCst))
    }
}
struct TestAudit(AtomicBool);
impl AuditSink for TestAudit {
    fn append(&self, _: &AuditEvent) -> Result<(), String> {
        if self.0.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err("Audit unavailable".into())
        }
    }
}
struct TestShell(AtomicBool);
impl Shell for TestShell {
    fn stop_all(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    fn terminal(&self, _: &str) -> Result<TerminalConnection, String> {
        let (input, _) = tokio::sync::mpsc::channel(1);
        let (_, output) = tokio::sync::mpsc::channel(1);
        Ok(TerminalConnection {
            input,
            output,
            cancel: Arc::new(AtomicBool::new(false)),
        })
    }
    fn execute(
        &self,
        _: ExecRequest,
        _: Arc<AtomicBool>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ExecResult> + Send>> {
        Box::pin(async { ExecResult::default() })
    }
}
fn fixture() -> (HostService, Arc<TestClock>, Arc<TestAudit>, Arc<TestShell>) {
    let clock = Arc::new(TestClock(AtomicU64::new(100)));
    let audit = Arc::new(TestAudit(AtomicBool::new(true)));
    let shell = Arc::new(TestShell(AtomicBool::new(false)));
    (
        HostService::new(
            clock.clone(),
            Arc::new(TestSecrets(AtomicU64::new(0))),
            audit.clone(),
            shell.clone(),
        ),
        clock,
        audit,
        shell,
    )
}
#[test]
fn failed_audit_never_grants_access() {
    let (service, _, audit, _) = fixture();
    service
        .start("http://127.0.0.1:1".into(), ".".into(), false)
        .unwrap();
    let invite = service.snapshot().invite.unwrap();
    let receipt = service.pair(&invite, "dev").unwrap();
    audit.0.store(false, Ordering::SeqCst);
    assert!(service.approve(&receipt.id, 15).is_err());
    assert!(service.snapshot().session.is_none());
    assert!(service
        .pair_status(&receipt.id, &receipt.proof)
        .unwrap()
        .token
        .is_none());
}
#[test]
fn expiry_invalidates_and_terminates_even_if_audit_fails() {
    let (service, clock, audit, shell) = fixture();
    service
        .start("http://127.0.0.1:1".into(), ".".into(), false)
        .unwrap();
    let receipt = service
        .pair(&service.snapshot().invite.unwrap(), "dev")
        .unwrap();
    service.approve(&receipt.id, 15).unwrap();
    let token = service
        .pair_status(&receipt.id, &receipt.proof)
        .unwrap()
        .token
        .unwrap();
    clock.0.store(1000, Ordering::SeqCst);
    audit.0.store(false, Ordering::SeqCst);
    service.expire();
    assert!(service.authenticate(&token).is_err());
    assert!(shell.0.load(Ordering::SeqCst));
    assert!(service.snapshot().error.is_some());
}
#[test]
fn terminal_budget_is_shared_and_released() {
    let (service, _, _, _) = fixture();
    service
        .start("http://127.0.0.1:1".into(), ".".into(), false)
        .unwrap();
    let receipt = service
        .pair(&service.snapshot().invite.unwrap(), "dev")
        .unwrap();
    service.approve(&receipt.id, 15).unwrap();
    let token = service
        .pair_status(&receipt.id, &receipt.proof)
        .unwrap()
        .token
        .unwrap();
    let mut terminals = Vec::new();
    for _ in 0..4 {
        terminals.push(service.terminal(&token).unwrap());
    }
    assert!(service.terminal(&token).is_err());
    terminals.pop();
    assert!(service.terminal(&token).is_ok());
}
#[test]
fn old_tunnel_events_cannot_modify_restarted_host() {
    let (service, _, _, _) = fixture();
    service
        .start("http://127.0.0.1:1".into(), ".".into(), true)
        .unwrap();
    let old = service.generation();
    service.stop();
    service
        .start("http://127.0.0.1:2".into(), ".".into(), true)
        .unwrap();
    service.tunnel_status(
        old,
        "online",
        Some("https://old.trycloudflare.com".into()),
        None,
    );
    service.tunnel_timeout(old);
    assert_eq!(service.snapshot().url, "http://127.0.0.1:2");
    assert_eq!(service.snapshot().tunnel, "starting");
}
