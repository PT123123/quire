# M9 Android — pre-research notes (planning only, no code)

Written 2026-09-19 against Slint 1.18. Windows RC comes first (SPEC §二十八);
this page is the homework for the day we start.

Updated 2026-09-20 after Track B measured the toolchain instead of trusting
this page. The evidence — three builds, their commands and their exact output —
is in `.scratch/m9/report.md`; the changed claims carry a `measured` marker
below. **The headline: the stack we doubted builds and links for Android, and
the one hard blocker in our dependency graph is a file-dialog crate this page
never mentions.**

## What ports for free

- **`src/core/**`** — the document model, commands, and history are pure
  Rust with no Slint and no I/O. They compile for any target as-is.
- **`src/storage/**`** — rusqlite bundled builds for Android (measured:
  `libsqlite3-sys` compiled and linked for `x86_64-linux-android` with NDK
  30.0.15729638); the DB path moves to the app's data dir — supplied by
  `android-activity`'s `AndroidApp::data_dir()`, which is already in the graph
  via Slint, so no separate `ndk-context` is needed. *New caveat:* the only
  environment read in the crate is `%APPDATA%`
  (`src/storage/data_location.rs:112`), and with no per-user directory the
  placement falls back to `appdata/quire.db` relative to the working directory
  — not writable on Android, so the app silently runs in memory. The seam is
  good news, though: `decide()`/`migration()` already take the per-user path as
  a parameter (`data_location.rs:166`), so this is "supply the path", not a
  policy rewrite.
- **The persistence contract** (ADR-0012) is UI-agnostic by design; the
  editor commands (SPEC §十四) carry over unchanged.
- *Measured 2026-09-20:* `src/core|storage|services` import Slint nowhere
  (0 files), and `src/platform/mod.rs` is 169 lines / 3 functions, each already
  cfg-gated with a "report failure instead of pretending" fallback. The one
  blocker the graph hits first is **`rfd`** — `Cargo.toml:10` pulls it
  unconditionally for two `FileDialog` call sites
  (`src/app/controller.rs:1776`, `:1791`), and it fails to *compile* for
  Android (12 errors, no backend). It is not an Android-runtime problem to
  discover later; it is one cfg-gated dependency plus two call sites routed
  through `platform/`.

## What must be rebuilt

- **All of `ui/`** — the shell assumes a pointer, a 1280×800 window, a
  frameless title bar, and hover affordances. Android needs a
  bottom-bar/touch navigation, larger touch targets (≥44 px), and a
  single-column editor layout. *Measured size of that job:* 31 `has-hover`
  affordances, 25 `TouchArea` / 62 `clicked` handlers, 68 `in-out` properties on
  the `UIState` contract (`ui/Types.slint`, ~244 property declarations across
  `ui/`), and row heights of 28–32 px in the block menus / 36–40 px in the
  palette, sidebar and find bar — **the 52 px search result row is the only
  interactive element that clears the 44 dp minimum.** `PopupWindow` itself is
  fine there (8 components; upstream maintains Android popup cursor/handle/
  safe-area fixes), but note the compiler rule that a popup cannot be the
  exported entry file passed to `slint-build` — Quire's shape already respects
  it.
- **The block renderer's hover model** — BlockHandle menus become
  long-press menus; the find bar moves to a system-style search field.
- **Window size persistence** — replaced by safe-area + fullscreen.

## Known risks (SPEC §二十八)

1. **Slint's Android TextInput** has open issues (history of composition/
   IME gaps). The editor's whole UX rides on it — budget a dedicated
   spike: a minimal IME test app on a real device before committing.
   *Confirmed, with names (11 open `a:platform-android` issues as of
   2026-09-20):* #9240 *Android TextInput issues with Microsoft SwiftKey
   Keyboard* — still open — reproduces items straight off
   `docs/IME_CHECKLIST.md` ("can't backspace/clear existing text", "Enter then
   backspace produces junk", "after clearing, nothing types"); #11810
   *Keyboard/input problems on Android* is open and newer; #6162/#6531 cover
   OSK events and misplaced selection.
2. **Our own caret model** (`cursor-position-byte-offset` +
   `set-selection-offsets`) is exercised the same way, but composition
   state handling may differ per keyboard (Gboard vs Samsung). *Measured
   supplement:* that property and its two-way `text <=>` binding do codegen and
   type-check for the Android target, so the risk is behaviour on hardware, not
   the build. Treat the Windows checklist as the test plan, run per keyboard.
3. **Performance** — FemtoVG on GLES vs the desktop GL path; re-run the
   scene matrix on-device before promising anything. *Corrected:* it is not
   FemtoVG. Slint 1.18 gates `i-slint-renderer-femtovg` behind
   `cfg(not(target_os = "android"))`; the Android renderer is **Skia on GLES
   (Ganesh)**, and `skia-bindings` fetches a prebuilt binary from GitHub rather
   than compiling Skia (so the first build of a given revision needs network).
   Two consequences: our `[features]` renderer matrix means something different
   on Android, and the desktop skia comparison it made mandatory is now
   measured in `docs/PERFORMANCE.md` (A2 follow-up #3) — Android numbers will
   be read against the **skia-on-OpenGL** desktop arm, since the default
   desktop skia build delivers no rendering-notifier events at all and has no
   first-paint number. Same instrument, same caveat on device: the notifier
   only fires on surfaces whose `Surface::with_graphics_api` is real, so a
   silent `--measure-startup` run on a phone means "this surface does not
   notify", never "no frame was drawn".
4. **Hardware, added 2026-09-20.** This is the critical path, not code: `adb
   devices` is empty and the local SDK has no `system-images/`, so there is no
   device and no AVD to make one. `cargo-apk` is not installed either
   (`cargo-ndk`, NDK 25–30 and platforms 34–37 are).

## Build tooling sketch

- `cargo apk` (android-activity) or a slint-android template; CI needs an
  NDK job. The lib becomes a cdylib; `src/main.rs` splits into a
  platform entry per OS behind a cfg. *Measured 2026-09-20:* the cdylib shape
  links (192 MB debug/unstripped `libm9_probe.so`, 70 s warm), so "the lib
  becomes a cdylib" is a fact rather than a risk; release + stripped APK size
  is unmeasured. And per this project's no-CI decision, that "NDK job" is a
  local script, not a pipeline.
- Signing/debugging loop is the slow part — prototype on a physical
  device early.

## Suggested first milestone (M9.0)

A read-only Android viewer: load the SQLite file, list pages, render
blocks (no editing). It proves storage + renderer + build pipeline with
none of the IME risk, and gives the team a device target for the editor
spike that follows.

*Revised 2026-09-20: the ordering above was built on a guess that has since
inverted.* The viewer's risk — storage, renderer, build pipeline — is exactly
the part this desktop can already prove, so it should not be first. What cannot
be proven without hardware is text input, and that is the risk that can
invalidate the milestone.

1. **M9.pre (code, no device needed).** cfg-gate `rfd` behind
   `cfg(not(target_os = "android"))` and route its two pickers through
   `platform/`; add a `platform::data_dir()` that feeds the existing pure
   `data_location::decide()`, and make "no per-user directory" a loud error
   instead of a silent in-memory session; fix the Slint feature list per target
   (Android wants `backend-android-activity-06`; `system-tray` must stay on
   desktop, where the tray is real since ADR-0096, and has to come out of the
   Android build). Acceptance: `cargo check --target x86_64-linux-android
   --all-targets` clears our own crates, not just the graph.
2. **M9.0 input spike (hardware).** One `TextInput`, one `PopupWindow` beside
   it, on a real device, running `docs/IME_CHECKLIST.md`'s composition section
   under SwiftKey / Gboard / Samsung. Go/no-go: it passes without patching
   Slint.
3. **M9.1 read-only viewer.** What M9.0 used to be, demoted to second because
   it carries none of the risk it was designed to dodge.
