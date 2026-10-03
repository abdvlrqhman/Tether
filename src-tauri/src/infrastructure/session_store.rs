use crate::application::{agent::AgentSession, ports::SessionStore};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
pub struct PrivateSessionStore {
    path: PathBuf,
}
impl PrivateSessionStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    pub fn default_path() -> Result<PathBuf, String> {
        if let Some(path) = std::env::var_os("TETHER_SESSION_FILE") {
            return Ok(path.into());
        }
        #[cfg(windows)]
        let base = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .ok_or("LOCALAPPDATA is unavailable")?;
        #[cfg(unix)]
        let base = std::env::var_os("HOME")
            .map(|p| PathBuf::from(p).join(".config"))
            .ok_or("Home directory is unavailable")?;
        Ok(base.join("spacie-tether/agent.session.json"))
    }
}
fn read_bounded(path: &Path) -> Result<String, String> {
    let mut result = String::new();
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > 32768 {
        return Err("Credential or invitation file is too large".into());
    }
    file.take(32769)
        .read_to_string(&mut result)
        .map_err(|e| e.to_string())?;
    if result.len() > 32768 {
        return Err("File exceeded size limit".into());
    }
    Ok(result)
}
impl SessionStore for PrivateSessionStore {
    fn load(&self) -> Result<AgentSession, String> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if fs::metadata(&self.path)
                .map_err(|_| "No session. Call tether_pair and get host approval.")?
                .permissions()
                .mode()
                & 0o077
                != 0
            {
                return Err("Session file must be private (chmod 600)".into());
            }
        }
        serde_json::from_str(
            &read_bounded(&self.path)
                .map_err(|_| "No valid session. Call tether_pair and get host approval.")?,
        )
        .map_err(|_| "Session file is invalid".into())
    }
    fn save(&self, session: &AgentSession) -> Result<(), String> {
        let parent = self.path.parent().ok_or("Invalid session file path")?;
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let temp = parent.join(format!(".tether-session-{}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let result = (|| {
            let mut file = options.open(&temp).map_err(|e| e.to_string())?;
            file.write_all(&serde_json::to_vec(session).map_err(|e| e.to_string())?)
                .and_then(|_| file.sync_all())
                .map_err(|e| e.to_string())?;
            drop(file);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                let who = std::process::Command::new("whoami.exe")
                    .args(["/user", "/fo", "csv", "/nh"])
                    .creation_flags(0x08000000)
                    .output()
                    .map_err(|e| e.to_string())?;
                let text = String::from_utf8_lossy(&who.stdout);
                let sid = text
                    .split('"')
                    .find(|s| s.starts_with("S-1-"))
                    .ok_or("Cannot identify current user SID")?;
                let output = std::process::Command::new("icacls.exe")
                    .arg(&temp)
                    .args(["/inheritance:r", "/grant:r", &format!("*{sid}:(F)")])
                    .creation_flags(0x08000000)
                    .output()
                    .map_err(|e| e.to_string())?;
                if !output.status.success() {
                    return Err("Cannot restrict session file permissions".into());
                }
            }
            fs::rename(&temp, &self.path).map_err(|e| e.to_string())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result
    }
    fn remove(&self) -> Result<(), String> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
    fn invitation(&self, path: &str) -> Result<String, String> {
        read_bounded(Path::new(path))
    }
}
