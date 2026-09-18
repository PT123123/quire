fn main() {
    // QUIRE_PROBE: compile a subset .slint entry instead of the app — used to
    // bisect compiler issues per component (see docs/DECISIONS.md debugging note).
    println!("cargo:rerun-if-env-changed=QUIRE_PROBE");
    let ui = std::env::var("QUIRE_PROBE").unwrap_or_else(|_| "ui/AppWindow.slint".into());
    slint_build::compile(&ui).expect("Slint build failed");
}
