// Quire library crate: all modules live here so `tests/` integration tests
// and the thin binary share one compiled unit. The binary (main.rs) only
// parses args, builds the window, and runs the event loop.
//
// The model and the store moved out into `crates/data` (`quire-data`), which
// is the crate the Android port will consume. These re-exports are what makes
// that move invisible from inside the shell: `crate::core::…` here and
// `quire::services::…` from an integration test both resolve through them, so
// the split is a directory change and not a 950-line path rewrite. Delete them
// when the shell starts naming `quire_data::` directly.

pub mod app;
pub mod platform;

pub use quire_data::{core, services, storage, testing};

slint::include_modules!();
