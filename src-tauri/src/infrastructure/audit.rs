use crate::{application::ports::AuditSink, domain::AuditEvent};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
    sync::Mutex,
};
pub struct JsonlAudit(Mutex<File>);
impl JsonlAudit {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options
            .open(path)
            .map(|f| Self(Mutex::new(f)))
            .map_err(|e| format!("Cannot open audit log: {e}"))
    }
}
impl AuditSink for JsonlAudit {
    fn append(&self, event: &AuditEvent) -> Result<(), String> {
        let mut file = self.0.lock().unwrap();
        serde_json::to_writer(&mut *file, event).map_err(|e| e.to_string())?;
        file.write_all(b"\n")
            .and_then(|_| file.flush())
            .map_err(|e| format!("Cannot write audit log: {e}"))
    }
}
