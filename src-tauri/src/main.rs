#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    if std::env::args().any(|arg| arg == "--mcp") {
        if let Err(error) = tether_lib::run_mcp() {
            eprintln!("Tether MCP: {error}");
            std::process::exit(1);
        }
        return;
    }
    tether_lib::run()
}
