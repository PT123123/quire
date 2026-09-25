set shell := ["powershell.exe", "-NoProfile", "-Command"]

# list available recipes
default:
    @just --list

# release build (default FemtoVG renderer)
build:
    cargo build --release

# run in debug; pass cargo flags as one quoted string
# e.g. just run "--no-default-features --features skia"
run *args:
    cargo run {{ args }}

# local CI replacement (the GitHub workflow was removed on purpose):
# everything a push would run, before you commit.
# `--workspace` survived the two-crate week and is now decoration: this workspace
# has one member again, and `quire-core` is a *dependency*, which no cargo flag
# run from here will test. Its 431 tests belong to that repository
# (`cargo test --all-targets` there). So a green `just check` here means the shell
# compiles against the pinned rev and its own suite passes — never quote it as
# "the whole app is green".
check:
    cargo check --workspace --all-targets
    cargo test --workspace
    cargo build --workspace --release

# headless visual shot: software-rendered PNG of the real UI, no window.
# Scene names: default dark palette search-notes menu rename settings dialog empty
shot scene="default":
    cargo build --features software --bin quire-shot
    .\target\debug\quire-shot.exe --out .scratch\shots\latest.bmp --scene {{ scene }}
    powershell -NoProfile -ExecutionPolicy Bypass -File benchmarks\scripts\shot2png.ps1

# package the release exe into a zip (M8-lite; installer comes later)
dist:
    cargo build --release
    powershell -NoProfile -ExecutionPolicy Bypass -File benchmarks\scripts\dist.ps1

# deploy a release into the workshop, in two places: the archive folder
# C:\workshop\quire-desktop-<version>\quire.exe and the install
# C:\workshop\quire-desktop\quire.exe — the path the app is *run* from, which
# every deploy overwrites in place. The name is `quire-desktop`, the shell: the
# workshop lists one folder per release of several applications, and a bare
# `quire-<version>` would not say which of the two shells put it there. The
# script bumps [package] version's patch, builds, commits and pushes the bump,
# then asks any instance running from the install path to end its own session
# through the app's quit channel and waits for it — asked, never killed, because
# a killed session loses the flush and the clean-exit record (ADR-0105) — and
# only then copies. The bump has to precede the build because build.rs stamps
# the exe's version block from that same key.
#
# The build is ~2m30s, and that is one crate's codegen rather than a cold cache:
# the bump invalidates the whole `quire` crate, and `[profile.release]`'s
# `codegen-units = 1` + thin LTO is *both* the audited size choice and the
# fastest setting for this repeat build (measured 2026-09-25 — PERFORMANCE.md
# "Build cost", and ADR-0024's update).
deploy-workshop:
    powershell -NoProfile -ExecutionPolicy Bypass -File install\deploy-workshop.ps1

# installer end-to-end regression (D8/A6): build iss, silent install,
# verify, silent uninstall, residue check (needs Inno Setup)
# --portable end-to-end regression (A1 follow-up): 24 checks over the
# portable layout, log following, migration suppression, --db precedence
verify-portable:
    powershell -NoProfile -ExecutionPolicy Bypass -File install\verify-portable.ps1

verify-install:
    powershell -NoProfile -ExecutionPolicy Bypass -File install\verify-installer.ps1

# remove build artifacts: ./target + skia/wgpu benchmark target dirs
clean:
    cargo clean
    Remove-Item -Recurse -Force target-skia, target-wgpu -ErrorAction SilentlyContinue
