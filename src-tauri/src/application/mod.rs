pub mod agent;
pub mod operator;
pub mod ports;
#[cfg(test)]
mod tests;
use crate::domain::*;
use ports::*;
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

#[derive(Clone)]
pub struct HostService {
    state: Arc<Mutex<Host>>,
    pub clock: Arc<dyn Clock>,
    secrets: Arc<dyn SecretSource>,
    audit: Arc<dyn AuditSink>,
    shell: Arc<dyn Shell>,
    jobs: Arc<Mutex<HashMap<String, Job>>>,
    slots: Arc<Semaphore>,
    rate: Arc<Mutex<(u64, u32)>>,
}
struct Job {
    owner: String,
    view: JobView,
    cancel: Arc<AtomicBool>,
}
pub struct TerminalLease {
    pub connection: TerminalConnection,
    pub session: SessionView,
    pub _permit: OwnedSemaphorePermit,
}
#[derive(Serialize)]
pub struct PairReceipt {
    pub id: String,
    pub proof: String,
}
#[derive(Serialize)]
pub struct PairStatus {
    pub status: String,
    pub token: Option<String>,
    pub expires_at: Option<u64>,
    pub os: Option<String>,
}

impl HostService {
    pub fn new(
        clock: Arc<dyn Clock>,
        secrets: Arc<dyn SecretSource>,
        audit: Arc<dyn AuditSink>,
        shell: Arc<dyn Shell>,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(Host::default())),
            clock,
            secrets,
            audit,
            shell,
            jobs: Arc::new(Mutex::new(HashMap::new())),
            slots: Arc::new(Semaphore::new(4)),
            rate: Arc::new(Mutex::new((0, 0))),
        }
    }
    pub fn snapshot(&self) -> HostView {
        self.state.lock().unwrap().view(self.clock.now())
    }
    pub fn generation(&self) -> u64 {
        self.state.lock().unwrap().generation
    }
    pub fn tunnel_timeout(&self, generation: u64) {
        let mut h = self.state.lock().unwrap();
        if h.running && h.generation == generation && h.tunnel == "starting" {
            h.tunnel = "failed".into();
            h.error=Some("Tunnel startup timed out. Stop sharing, check cloudflared and your network, then retry.".into());
        }
    }
    pub fn tunnel_status(
        &self,
        generation: u64,
        status: &str,
        url: Option<String>,
        error: Option<String>,
    ) {
        let mut h = self.state.lock().unwrap();
        if h.running && h.generation == generation {
            h.tunnel = status.into();
            if let Some(url) = url {
                h.url = url;
            }
            h.error = error;
        }
    }
    fn log(&self, h: &mut Host, kind: &str, actor: &str, detail: &str) -> Result<(), String> {
        let e = AuditEvent {
            at: self.clock.now(),
            kind: kind.into(),
            actor: actor.into(),
            detail: detail.into(),
        };
        self.audit.append(&e)?;
        h.audit.push(e);
        if h.audit.len() > 200 {
            h.audit.remove(0);
        }
        Ok(())
    }
    pub fn start(&self, url: String, cwd: String, online: bool) -> Result<(), String> {
        let mut h = self.state.lock().unwrap();
        if h.running {
            return Err("Host is already running".into());
        }
        self.log(
            &mut h,
            "host_started",
            "local host",
            if online {
                "TryCloudflare requested"
            } else {
                "Local connection"
            },
        )?;
        h.running = true;
        h.generation = h.generation.wrapping_add(1);
        h.local_url = url.clone();
        h.url = url;
        h.working_directory = cwd;
        h.error = None;
        h.pairings.clear();
        h.tunnel = if online { "starting" } else { "local" }.into();
        let token = self.secrets.generate();
        h.invite = Some((
            token.clone(),
            SecretHash::new(&token),
            self.clock.now() + INVITE_TTL,
        ));
        Ok(())
    }
    pub fn stop(&self) {
        let mut h = self.state.lock().unwrap();
        if !h.running {
            return;
        }
        h.running = false;
        h.revoke();
        self.shell.stop_all();
        h.invite = None;
        h.tunnel = "stopped".into();
        if let Err(e) = self.log(&mut h, "host_stopped", "local host", "All access revoked") {
            h.error = Some(e);
        }
        for j in self.jobs.lock().unwrap().values() {
            j.cancel.store(true, Ordering::SeqCst);
        }
    }
    pub fn revoke(&self) -> Result<(), String> {
        let mut h = self.state.lock().unwrap();
        h.revoke();
        self.shell.stop_all();
        for j in self.jobs.lock().unwrap().values() {
            j.cancel.store(true, Ordering::SeqCst);
        }
        self.log(
            &mut h,
            "session_revoked",
            "local host",
            "Remote processes cancelled",
        )
    }
    pub fn release(&self, token: &str) -> Result<(), String> {
        let mut h = self.state.lock().unwrap();
        let session = h.authenticate(token, self.clock.now())?;
        h.revoke();
        self.shell.stop_all();
        for j in self.jobs.lock().unwrap().values() {
            j.cancel.store(true, Ordering::SeqCst);
        }
        self.log(
            &mut h,
            "session_released",
            &session.operator,
            "Operator ended access",
        )
    }
    pub fn renew_invite(&self) -> Result<(), String> {
        let mut h = self.state.lock().unwrap();
        if !h.running || h.session.is_some() {
            return Err("Stop the active session before creating an invitation".into());
        }
        self.log(
            &mut h,
            "invitation_renewed",
            "local host",
            "Expires in ten minutes",
        )?;
        let token = self.secrets.generate();
        h.invite = Some((
            token.clone(),
            SecretHash::new(&token),
            self.clock.now() + INVITE_TTL,
        ));
        h.pairings.clear();
        Ok(())
    }
    pub fn pair(&self, invite: &str, operator: &str) -> Result<PairReceipt, String> {
        let now = self.clock.now();
        let mut rate = self.rate.lock().unwrap();
        if now.saturating_sub(rate.0) >= 60 {
            *rate = (now, 0);
        }
        rate.1 += 1;
        if rate.1 > 30 {
            return Err("Pairing rate limit reached; retry in one minute".into());
        }
        drop(rate);
        let id = self.secrets.generate();
        let proof = self.secrets.generate();
        let mut h = self.state.lock().unwrap();
        h.request_pairing(invite, operator, id.clone(), proof.clone(), now)?;
        if let Err(e) = self.log(
            &mut h,
            "pairing_requested",
            operator,
            "Waiting for local approval",
        ) {
            h.pairings.retain(|p| p.view.id != id);
            return Err(e);
        }
        Ok(PairReceipt { id, proof })
    }
    pub fn pair_status(&self, id: &str, proof: &str) -> Result<PairStatus, String> {
        let h = self.state.lock().unwrap();
        let now = self.clock.now();
        let p = h
            .pairings
            .iter()
            .find(|p| p.view.id == id && p.proof.matches(proof) && now < p.expires_at)
            .ok_or("Pairing is invalid or expired")?;
        let valid = h.active(id, now);
        Ok(PairStatus {
            status: if p.status == "approved" && !valid {
                "revoked".into()
            } else {
                p.status.clone()
            },
            token: if valid { p.token.clone() } else { None },
            expires_at: h
                .session
                .as_ref()
                .filter(|s| s.view.id == id && valid)
                .map(|s| s.view.expires_at),
            os: if valid {
                Some(std::env::consts::OS.into())
            } else {
                None
            },
        })
    }
    pub fn approve(&self, id: &str, minutes: u64) -> Result<(), String> {
        let mut h = self.state.lock().unwrap();
        let now = self.clock.now();
        // Validate a pending request before writing the approval record.
        let actor = h
            .pairings
            .iter()
            .find(|p| p.view.id == id && p.status == "pending" && now < p.expires_at)
            .map(|p| p.view.operator.clone())
            .ok_or("Pairing request expired")?;
        if ![15, 30, 60, 120].contains(&minutes)
            || !h.running
            || h.session.as_ref().is_some_and(|s| now < s.view.expires_at)
        {
            return Err("Cannot approve this request".into());
        }
        self.log(
            &mut h,
            "session_approved",
            &actor,
            &format!("Full user shell access for {minutes} minutes"),
        )?;
        h.approve(id, self.secrets.generate(), minutes, now)?;
        Ok(())
    }
    pub fn deny(&self, id: &str) -> Result<(), String> {
        let mut h = self.state.lock().unwrap();
        let p = h
            .pairings
            .iter_mut()
            .find(|p| p.view.id == id && p.status == "pending")
            .ok_or("Pending request not found")?;
        p.status = "denied".into();
        let actor = p.view.operator.clone();
        self.log(&mut h, "pairing_denied", &actor, "Denied by local host")
    }
    pub fn authenticate(&self, token: &str) -> Result<SessionView, String> {
        self.state
            .lock()
            .unwrap()
            .authenticate(token, self.clock.now())
    }
    pub fn active(&self, id: &str) -> bool {
        self.state.lock().unwrap().active(id, self.clock.now())
    }
    pub fn terminal(&self, token: &str) -> Result<TerminalLease, String> {
        let h = &mut *self.state.lock().unwrap();
        let session = h.authenticate(token, self.clock.now())?;
        let permit = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| "Four remote processes are already running")?;
        self.log(
            h,
            "terminal_opened",
            &session.operator,
            "Interactive shell; input and output are not recorded",
        )?;
        let connection = self.shell.terminal(&h.working_directory)?;
        Ok(TerminalLease {
            connection,
            session,
            _permit: permit,
        })
    }
    pub fn submit(&self, token: &str, mut request: ExecRequest) -> Result<JobView, String> {
        request.validate()?;
        let mut h = self.state.lock().unwrap();
        let session = h.authenticate(token, self.clock.now())?;
        let permit = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| "Four remote processes are already running")?;
        if request.cwd.is_none() {
            request.cwd = Some(h.working_directory.clone());
        }
        let id = self.secrets.generate();
        let cancel = Arc::new(AtomicBool::new(false));
        let view = JobView {
            id: id.clone(),
            status: "running".into(),
            result: None,
        };
        let mut jobs = self.jobs.lock().unwrap();
        if jobs.len() >= 64 {
            jobs.retain(|_, j| j.view.status == "running");
        }
        self.log(
            &mut h,
            "command_started",
            &session.operator,
            &format!("Job {id}; command content is not recorded"),
        )?;
        jobs.insert(
            id.clone(),
            Job {
                owner: session.id.clone(),
                view: view.clone(),
                cancel: cancel.clone(),
            },
        );
        drop(jobs);
        drop(h);
        let service = self.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let execute = service.shell.execute(request, cancel.clone());
            tokio::pin!(execute);
            let mut tick = tokio::time::interval(std::time::Duration::from_millis(100));
            let result = loop {
                tokio::select! {result=&mut execute=>break result,_=tick.tick()=>{if !service.active(&session.id){cancel.store(true,Ordering::SeqCst);}}}
            };
            {
                let mut jobs = service.jobs.lock().unwrap();
                if let Some(job) = jobs.get_mut(&id) {
                    job.view.status = "finished".into();
                    job.view.result = Some(result.clone());
                }
            }
            let mut h = service.state.lock().unwrap();
            if let Err(e) = service.log(
                &mut h,
                "command_finished",
                &session.operator,
                &format!(
                    "Job {id}; exit {:?}; cancelled {}",
                    result.exit_code, result.cancelled
                ),
            ) {
                h.error = Some(e);
            }
        });
        Ok(view)
    }
    pub fn job(&self, token: &str, id: &str, cancel: bool) -> Result<JobView, String> {
        let session = self.authenticate(token)?;
        let jobs = self.jobs.lock().unwrap();
        let job = jobs
            .get(id)
            .filter(|j| j.owner == session.id)
            .ok_or("Job not found")?;
        if cancel {
            job.cancel.store(true, Ordering::SeqCst);
        }
        Ok(job.view.clone())
    }
    pub fn expire(&self) {
        let mut h = self.state.lock().unwrap();
        let now = self.clock.now();
        h.pairings.retain(|p| now < p.expires_at);
        if h.invite.as_ref().is_some_and(|v| now >= v.2) {
            h.invite = None;
        }
        if h.session
            .as_ref()
            .is_some_and(|s| self.clock.now() >= s.view.expires_at)
        {
            h.revoke();
            self.shell.stop_all();
            for j in self.jobs.lock().unwrap().values() {
                j.cancel.store(true, Ordering::SeqCst);
            }
            if let Err(e) = self.log(&mut h, "session_expired", "system", "Remote access expired") {
                h.error = Some(e);
            }
        }
    }
}
