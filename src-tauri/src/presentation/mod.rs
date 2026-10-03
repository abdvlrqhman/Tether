#[cfg(feature = "desktop")]
mod desktop;
pub mod mcp;
#[cfg(test)]
mod mcp_tests;
#[cfg(feature = "desktop")]
pub use desktop::run;
