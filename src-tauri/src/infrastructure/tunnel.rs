use crate::application::HostService;
use std::process::Stdio;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
};
pub fn spawn(binary: &std::path::Path, url: &str, service: HostService) -> Result<Child, String> {
    if !binary.is_file() {
        return Err(
            "Bundled cloudflared is missing. Run npm run prepare:sidecar and rebuild Tether."
                .into(),
        );
    }
    let mut cmd = Command::new(binary);
    cmd.args(["tunnel", "--url", url, "--no-autoupdate"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x08000000);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not start bundled cloudflared: {e}"))?;
    let stderr = child.stderr.take().unwrap();
    let generation = service.generation();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut public_url = None;
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(url) = extract_url(&line) {
                public_url = Some(url);
            }
            // The banner is printed before a connection reaches Cloudflare's edge.
            // Publish the invitation only once cloudflared confirms registration.
            if line.contains("Registered tunnel connection") {
                if let Some(url) = public_url.clone() {
                    service.tunnel_status(generation, "online", Some(url), None);
                }
            }
        }
        service.tunnel_status(
            generation,
            "failed",
            None,
            Some(
                "Cloudflare tunnel exited. Stop sharing and retry, or use a local connection."
                    .into(),
            ),
        );
    });
    Ok(child)
}
fn extract_url(line: &str) -> Option<String> {
    for word in line.split_whitespace() {
        let word = word.trim_matches(|c: char| c == '|' || c == '"');
        if let Ok(url) = url::Url::parse(word) {
            if url.scheme() == "https"
                && url
                    .host_str()
                    .is_some_and(|h| h.ends_with(".trycloudflare.com"))
                && url.path() == "/"
            {
                return Some(url.as_str().trim_end_matches('/').into());
            }
        }
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_cloudflared_banner() {
        assert_eq!(
            extract_url("INF | https://happy-dev.trycloudflare.com |"),
            Some("https://happy-dev.trycloudflare.com".into())
        );
        assert!(extract_url("https://trycloudflare.com.evil.test").is_none());
    }
}
