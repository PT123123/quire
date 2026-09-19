# Agent brief — M8 tail package (A1–A5)

You are the storage/packaging-track agent ("Track B") for **Quire** — a Rust +
Slint 1.18, Notion-like, local-first notes app (SPEC: `docs/SPEC.md`). Working
tree: this repo, current branch `master` (at `1985fa1`, suite green: 199 pass /
4 ignored by design). Another agent ("Track A") works in the **same working
tree at the same time** on different files. The split below is designed so you
never touch the same files. Read the ground rules before anything else.

## Ground rules

1. **File ownership.** You may create/edit ONLY the files each task lists.
   Track A owns (never edit, never `git add`): `src/core/**`, `src/app/**`,
   `src/services/import_service.rs`, `ui/**`, `tests/**`, `README.md`,
   `PLAN.md`, `CHANGELOG.md`, `docs/DECISIONS.md`, `docs/M8_FEEDBACK.md`,
   `docs/SPEC.md`. Before starting a task, run `git status`; if a file you
   need is already modified and it is not yours, STOP and report instead.
2. **Git.** One commit per task, on the current branch (`master`), staging
   ONLY that task's explicit file list — never `git add -A` / `git add .`.
   Never `git checkout` / `switch` / `restore` / `reset` / `stash` / `rebase`.
   If `.git/index.lock` is held, Track A is committing — wait and retry.
   Commit only with `cargo check --all-targets` green.
3. **Builds.** `cargo` may block on the target-dir lock while Track A builds.
   Wait and retry; do NOT use a separate `CARGO_TARGET_DIR` (full rebuilds
   would make everything slower for both).
4. **Docs.** You own `docs/ROADMAP.md` and `docs/PERFORMANCE.md`. Anything
   that belongs in PLAN/CHANGELOG/DECISIONS/M8_FEEDBACK goes into your report
   instead — Track A folds it in with correct ADR numbering.
5. **Tests.** New tests ride the already-registered suites
   (`tests/integration/*.rs`, see `[[test]]` in Cargo.toml) or live in-module.
   Do NOT add/remove `[[test]]` entries. In A3 you may edit `[profile.release]`
   in `Cargo.toml` — nothing else in that file.
6. **Scratch data.** Test databases and renders go under `.scratch/` and are
   cleaned up when done. GUI runs for verification use
   `--db .scratch/<dir>/quire.db --auto-exit <secs>`; a killed-process test is
   the exception (kill externally, then clean up). Never touch the real
   per-user library (`%APPDATA%\Quire`) or a bare `appdata/` beside the repo.
7. **Report, don't improvise scope.** If a task turns out to need a file or a
   contract change outside your ownership, stop that task and report what you
   found — that is a valid outcome.

## Context pointers

- Living status: `PLAN.md` (per-round reports, read the last three sections)
- Feedback ledger: `docs/M8_FEEDBACK.md` (your tasks close #13; #1/#9/#10 are
  already resolved — read them for the established patterns)
- Decisions: `docs/DECISIONS.md` (ADR-0001…0023; ADR-0018 logging,
  ADR-0020 data location are background for A1/A2)
- Performance: `docs/PERFORMANCE.md` (method + matrix + ranked follow-ups)
- Scene list: the `apply_scene` / `apply_scene_overlay` match arms in
  `src/app/controller.rs` (read-only reference)
- Shots: `just shot <scene>` = build software renderer + render + convert;
  the pieces are `cargo build --features software --bin quire-shot`,
  `./target/debug/quire-shot.exe --out <bmp> [--scene <name>]`, then
  `benchmarks/scripts/shot2png.ps1`

---

## A1 · Close M8_FEEDBACK #13 — `--portable` plumbing + `migrated_from`

**Current state (verified).** `storage/data_location.rs` resolves placement
(`Fixed`/`Portable`/`Roaming`) partly by re-scanning `std::env::args()`.
`main.rs` resolves a second time to compute the library-moved notice, then
calls `SqliteRepository::open_with_report(&requested)`, which resolves a third
time internally (idempotent). `LaunchArgs` has no `portable` field; an unknown
flag is harmless today only because the re-scan is load-bearing.

**Deliverables.**
- `LaunchArgs` gains `portable: bool` (`--portable` flag).
- `data_location`'s public API takes the inputs explicitly (db path override,
  portable flag) and **stops reading `std::env::args()`**.
- `SqliteRepository::open_with_report` gains `migrated_from: Option<PathBuf>`
  beside `recovered_from` (precedent: #4's `OpenReport`, where `open()` stayed
  unchanged so no call site moved — follow that shape). `main.rs` drops its
  own duplicate resolve + `library_moved` local and builds the notice from the
  report field; the notice text stays the same ("the library moved to your
  user profile").
- Combined-flag semantics (`--portable` with `--db`) — decide, implement,
  document in code, and state it in your report. `--db <path>` must keep
  behaving exactly as today (explicit path wins; bench.ps1 `-PinnedDb` and
  quire-shot depend on it).

**Files:** `src/main.rs`, `src/storage/data_location.rs`,
`src/storage/repository.rs` (OpenReport struct + the open path only),
`src/services/logging.rs` (only if `data_dir`'s resolution must follow).

**Tests + verification.** In-module data_location tests (portable → portable
root; db override wins; resolve is idempotent; legacy `appdata/` migration
still happens exactly once); full suite green; e2e: a `--portable` run writes
db + log to the documented portable root, `--db` still wins, second run on the
same portable root does not re-migrate.

## A3 · Release profile audit (SPEC §二十四)

**Deliverables.** Measure the current `[profile.release]` first: scenes A
(empty shell), D (10 000 blocks), E (typing), plus startup time, idle private
bytes, and exe size — via `benchmarks/scripts/bench.ps1` and Task Manager or
an equivalent scripted probe. Then evaluate the §二十四 levers (`lto`,
`strip`/symbol strategy, `codegen-units`, `panic` strategy) and keep ONLY what
wins on **runtime memory / CPU / startup** — never trade those for exe size
(the SPEC is explicit). A well-measured "current profile is already right" is
a valid outcome. Record the final profile + before/after numbers in
`docs/PERFORMANCE.md`; the ADR text (why each knob moved or didn't) goes in
your report for Track A to number and fold into `docs/DECISIONS.md`.

**Files:** `Cargo.toml` (`[profile.release]` only), `benchmarks/**`,
`docs/PERFORMANCE.md`.

## A2 · First-paint startup measurement (closes a twice-deferred gap)

**Current state.** `startup_ms` measures window-up, not first paint — recorded
as a known gap in PLAN.md since M0/M1 and never closed.

**Deliverables.** A programmatic, **flag-gated** (e.g. `--measure-startup`)
measurement of "first painted frame" latency, recorded like the other bench
numbers (JSONL out of `bench.ps1`, plus a line in PERFORMANCE.md's method +
conclusion). Research what Slint 1.18 exposes (render/frame callbacks); if
public API cannot see first paint, a documented proxy (e.g. first
software-renderer frame, or present-timestamp) is acceptable — state the proxy
and its bias in PERFORMANCE.md. Normal runs (no flag) must not pay anything:
no timer, no thread, no extra frame.

**Files:** `src/main.rs`, `benchmarks/**`, `docs/PERFORMANCE.md`. If the hook
turns out to require `src/app/**` or `ui/**` changes, STOP and report — that
is Track A's side.

## A5 · ROADMAP refresh

`docs/ROADMAP.md`'s status column is stale (M3 still says "next"; M4–M8 rows
don't match PLAN.md's reports). Bring it current from PLAN.md: M0–M7 ✅ with
their dates, M8 in progress naming what actually remains (Link-to-page block,
drag-and-drop file import, rich clipboard paste, the user IME acceptance
pass), M9 blocked on Windows-stable. Keep the table format, the "out of scope"
list, and the "Definition of done" section unchanged.

**Files:** `docs/ROADMAP.md`.

## A4 · Full visual regression sweep (SPEC §二十一)

**Scope.** Every scene in `apply_scene`/`apply_scene_overlay` (~25, including
the `dark-*` variants), rendered light as declared. Build the software
renderer once, then render each scene to `.scratch/sweep/<scene>.png` (you may
extend `shot2png.ps1` to take an argument — `benchmarks/**` is yours). Note
the master commit you swept at in the report; Track A lands UI work in
parallel, so this is a snapshot, and re-shooting a flagged scene after a fix
is a follow-up, not part of this task.

**Review each PNG against the SPEC §二十一 checklist:** spacing/alignment
consistency, typography hierarchy, hover/selected/focus feedback where the
scene shows it, popup/anchor correctness (no clipped or off-window menus),
empty/loading/error states, CJK rendering, scrollbar, light-vs-dark
consistency.

**Output.** `.scratch/sweep/report.md`: one line per scene (pass/fail); every
fail carries what/where (element), the PNG path, and a severity. **Do NOT fix
anything; do NOT edit `ui/**`** — the report is the deliverable, Track A
triages it. Summarize the top defects in your final message.

**Files:** `.scratch/**`, `benchmarks/**` (script tweaks only).

---

## Sequencing and report format

Order: **A1 → A3 → A2 → A5 → A4** (commit early and often per task; A2 is
investigative and may dead-end into a report; A4 is last because it is the
widest snapshot).

Report after each task (and a final one): files changed (path + what/why,
line refs), commands run + evidence, deviations from this brief and why, and
notes for Track A to fold into PLAN/CHANGELOG/DECISIONS/M8_FEEDBACK. Flag
anything you noticed in Track A's territory without touching it. If you cannot
finish a task, report state honestly — an unfinished, honestly-reported task
beats a silently improvised one.
