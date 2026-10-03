pub mod audit;
pub mod http;
#[cfg(test)]
pub(crate) mod integration_tests;
mod process_scope;
pub mod remote;
pub mod session_store;
pub mod shell;
pub mod tunnel;
use crate::application::ports::{Clock, SecretSource};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::TryRng;
pub struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }
}
pub struct RandomSecrets;
impl SecretSource for RandomSecrets {
    fn generate(&self) -> String {
        let mut bytes = [0u8; 32];
        rand::rngs::SysRng
            .try_fill_bytes(&mut bytes)
            .expect("Operating system random source unavailable");
        URL_SAFE_NO_PAD.encode(bytes)
    }
}
