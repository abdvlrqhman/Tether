pub mod application;
pub mod domain;
pub mod infrastructure;
mod presentation;
#[cfg(feature = "desktop")]
pub fn run() {
    presentation::run();
}
pub fn run_mcp() -> Result<(), Box<dyn std::error::Error>> {
    presentation::mcp::run()
}
