// Quire library crate: all modules live here so `tests/` integration tests
// and the thin binary share one compiled unit. The binary (main.rs) only
// parses args, builds the window, and runs the event loop.

pub mod app;
// Filled by later milestones; kept declared so the skeleton is real code,
// not wishes in a directory listing (docs/ARCHITECTURE.md).
pub mod core;
pub mod platform;
pub mod services;
pub mod storage;
// Scratch-directory guard for tests; see the module header for why it is not
// `#[cfg(test)]`.
pub mod testing;

slint::include_modules!();
