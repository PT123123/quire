# Roadmap

Milestone plan from `docs/SPEC.md` with current status. Living status
notes and per-milestone reports live in `PLAN.md`; this file is the
stable map. Priorities (SPEC §三十五): UI quality > editing experience >
low RAM > low CPU > GPU rendering > maintainability > feature count.

| Milestone | Scope | Status |
|-----------|-------|--------|
| M0 Toolchain | Rust + Slint 1.18, MSVC, renderer matrix, benchmark harness, baselines | ✅ 2026-09-18 |
| M1 Design system | Theme/Colors/Typography/Icons, product-grade shell (mock data) | ✅ 2026-09-18 |
| M2 Shell navigation | Live workspace tree (create/rename/duplicate/delete/favorites/recents), page switching with per-page mock content, Ctrl+P search, context menus, settings + confirm dialogs, empty state, headless visual-regression tool, scenes F/G benchmarks | ✅ 2026-09-19 |
| M3 Local documents | `core/` document model, SQLite storage (workspaces/pages/blocks/metadata), debounced batched persistence, transactions, load-on-restart, settings persistence | ✅ 2026-09-19 (wired same day) |
| M4 Block editor MVP | Command system + undo/redo, single-TextInput editing, Enter/Backspace/merge/split, arrows, clipboard, 9 block kinds, Chinese IME acceptance pass | ✅ core 2026-09-19 · the IME pass is a user acceptance item (`docs/IME_CHECKLIST.md`), still open |
| M5 Notion interactions | Slash menu (command descriptors from Rust), palette wiring on the real command registry, block drag/reorder, block menu | ✅ 2026-09-19 (drag/reorder = ADR-0021; markdown shortcuts + curated slash/Turn-into menus = ADR-0022; collab/AI/color/copy-link stay out per SPEC) |
| M6 Rich text | Inline spans (bold/italic/code/link/strike), mark persistence, format-safe save/load | ✅ 2026-09-19 (link UI + import/export marks in; run rendering has a documented wrap limitation) |
| M7 Performance | Virtualization depth, lazy loading, background search (FTS), long-document suite at 1k/5k/10k blocks | ✅ 2026-09-19 · FTS5 index + async search, and the femtovg/skia scene A–G matrix is recorded in `docs/PERFORMANCE.md` (SPEC §六 checklist: all met); the release profile that produced these numbers is pinned by the A3 audit as ADR-0024 |
| M8 Windows RC | Crash recovery, migrations, packaging/installer, Markdown import/export, settings UI, keyboard shortcut pass | 🔄 in progress 2026-09-20 · delivered: rotating snapshots + startup recovery (ADR-0015), the `session.meta` + rotating log + panic hook and the clean-exit marker (ADR-0018), data-location move to the per-user library (ADR-0020), schema migrations to v5, Markdown import/export, installer + `just dist`, LAN share/pull, settings UI incl. the STORAGE row (data folder / open-folder / back-up-now), Page and Link-to-page blocks, rich clipboard paste, Move page, the shortcut pass. The tail A-package is landing: A1 `--portable` ✅, A3 release-profile audit ✅ (pinned as ADR-0024), A5 this refresh, A2 first-paint measurement ✅ plus its phase-split follow-up (the old window-up number understated startup ≈4×; the remaining gap is now owned per phase — a flat ≈410 ms inside `ui.run()` before the first frame, document-independent, and one scaling term `state_new` 5→148 ms; see `docs/PERFORMANCE.md`), a second follow-up that measured the scaling term at four points in one batch and settled it as **linear** — ≈4 ms + 13.1 ms per 1 000 blocks, `repo_open` flat at 51 ms across a 20× document range, and `benchmarks/scripts/audit_results.ps1` now regenerates every table in PERFORMANCE.md from its raw rows), A6 installer end-to-end ✅ (`install/verify-installer.ps1`; re-run green at `6c115b5` with the current release exe — 7.94 MB setup, installed exe exits 0, embedded icon still pixel-matches `quire.ico`, the `--open` verb imported pages 14→15, uninstall left zero residue), A1 follow-up: `--portable` verified end to end ✅ (`install/verify-portable.ps1`, 24/24, each child run with its own redirected APPDATA so the real per-user library is never in play), A4 visual sweep ✅ (34 scenes light + dark at `42cb158`, 13 pass / 21 fail, report in `.scratch/sweep/report.md`, harness `benchmarks/scripts/sweep.ps1`), re-swept at `6c115b5` after Track A's two fix commits — delta report `.scratch/sweep2/report.md`, method: hash every PNG against the first sweep, so only the 11 scenes that actually changed pixels were re-judged. Fixed: Settings' shortcut overflow, the Move-to popup clamp, the slash description column, the insert menu's disabled rows, find's counter, the dialog's destructive colour. Two of my own findings are retracted on that hash diff (bg-colour swatches, the title-edit line box) — both were 1:1 eyeball misreads of unchanged pixels. Re-swept a third time at `81a2937` (delta report `.scratch/sweep3/report.md`, same hash-diff method: 5 of 34 scenes moved, 29 carried their verdict unchanged) and the A4 LOW batch closed — the Add-link dialog's ~50 px dead zone (the popup height double-counted its own layout padding, so the VerticalLayout spent 32 px of slack as one gap: card 174 px → 141 px, gaps back to the designed 12/16 px, diff bbox exactly the old popup), mid-word snippet elision (both snippet builders now drop the partial word at each edge and leave CJK alone; the shot exercises the repo-less local path, which is why an FTS-only fix would have left the PNG byte-identical), the background-colour swatch rings, the notice band that had pushed the frameless title bar off the window edge (now y=40..69 under the 40 px bar), and the `recovered` fixture's invented copy citing `appdata/quire.db.bak1` — a path shape that predates ADR-0020 — which now mirrors the string `main.rs` really shows. A third of my own framings died in that re-sweep: the notice band does not "clip the sidebar's last row", because the tree list folds mid-row in the default scene too. Still open: the user IME acceptance pass (M4); the A4 defect list — 1 HIGH (a paragraph carrying inline marks loses its word wrap and is clipped mid-word, while the identical unmarked paragraph wraps; the "overprint" half of the original finding was the scene fixture's hardcoded mark offsets, now derived from the text, so the inline-code run is off-screen and untested — and the wrap itself is already documented as a Slint platform wall in `docs/EDITOR_ARCHITECTURE.md` §"Platform wall", i.e. design debt rather than a regression), 2 MEDIUM (find counts 16 matches and paints none of them on a blurred block — its selection-based hit routing is the same documented wall, so this one needs a focused manual look before it is called a defect; the empty state still says the block editor "arrives in a later milestone"), plus the LOW bundle (no scrim behind modals, Ctrl+B labelled both "Toggle Sidebar" and bold, dimmest-text hint rows in both themes, weakest colour pairs) and one known limitation (`renderer_name()` and `renderer_id()` are compile-time, so a `--features software` shot labels itself "FemtoVG · GL" — deduped to one function this round, but the feature ordering still lies for any build that picks its backend at runtime); menu-overflow polish (was the same root cause as the Move-to HIGH — the popup height clamp landed with it); Explorer drag-and-drop import is deferred by a platform limit (Slint 1.18 delivers no external file-drop events — revisit on upgrade or via a Win32 `IDropTarget` in `platform/`) |
| M9 Android | Shared core model + schema, adapted navigation/IME test plan (starts only after Windows is stable) | ⏸ blocked on M8 shipping Windows-stable. Evaluated 2026-09-20 by measurement rather than reading (evidence `.scratch/m9/report.md`, corrections folded into `docs/ANDROID_NOTES.md`): Slint 1.18's `backend-android-activity-06` + its Android renderer + bundled SQLite **compile and link for `x86_64-linux-android` on this machine's NDK 30** — a Quire-shaped `.slint` (window + `TextInput` two-way binding + popup) checks, and the `cdylib` links in 70 s warm. The one hard blocker in our graph is `rfd`, which has no Android backend and fails to *compile* (2 call sites, `controller.rs:1776`/`:1791`); the renderer there is **Skia/GLES, not FemtoVG** (Slint cfg-gates femtovg off Android), which makes the still-open desktop skia baseline in `docs/PERFORMANCE.md` a prerequisite rather than a curiosity; the only environment read in the crate is `%APPDATA%`, whose fallback is a cwd-relative path, so an Android build without a supplied data dir runs silently in memory. UI cost measured: 31 hover affordances, 8 popups, 68 `UIState` properties, and row heights of 28–40 px everywhere except the 52 px search result row — that one row is the only interactive element meeting the 44 dp touch minimum. Residual risk is concentrated in upstream Android text input (11 open `a:platform-android` issues, incl. #9240 SwiftKey and #11810) and in **there being no device**: `adb devices` is empty and the local SDK has no `system-images/`. Recommended order is M9.pre (cfg-gate `rfd`, add `platform::data_dir()`, fix the feature list) → M9.0 input spike on hardware → M9.1 read-only viewer, i.e. the viewer is demoted from first because the desktop can now prove the part it was meant to de-risk. |

## Explicitly out of scope for v1

Sync, collaboration, cloud, plugin market, AI, multi-process IPC, custom
TSF/IME implementation, image/table/toggle/database blocks.

## Definition of done (per milestone)

1. Scope items implemented and wired through the controller (no mocks in
   the UI layer).
2. `just check` green: check + tests + release build.
3. Performance re-measured for affected scenes in `docs/PERFORMANCE.md`.
4. Visual scenes re-shot (`just shot`) and reviewed.
5. `PLAN.md` report written; `DECISIONS.md` updated for any non-obvious
   choice.

## M8 verification snapshot (2026-09-20, at `6c115b5`)

There is no CI on purpose, so this is the manual "definition of done" evidence
for the current head, all of it re-runnable from the repo:

| gate | command | result |
|------|---------|--------|
| tests | `cargo test --all-targets` | green: 209 passed, 0 failed, 4 ignored across 10 binaries |
| release build | `cargo build --release` | clean (4 lib warnings, listed below) |
| visual | `benchmarks/scripts/sweep.ps1` (+ `-Baseline` diff) | 34/34 render; 19 scenes still fail, see `.scratch/sweep2/report.md` |
| performance | `benchmarks/scripts/audit_results.ps1` | every stored summary agrees with its raw runs |
| packaging | `install/verify-installer.ps1` | 4/4 green, zero residue after uninstall |
| data placement | `install/verify-portable.ps1` | 24/24 green, real per-user library untouched |

The four build warnings are one defect, and it is functional rather than
cosmetic: `src/app/controller.rs:550` matches on `CMD_COPY_MD`, a constant that
`src/app/state.rs:2502` exports but the `use` list at `controller.rs:9` does not
import, so the arm is a catch-all *binding*. rustc says so twice
(`unused variable: CMD_COPY_MD`, plus `unreachable pattern` at both `:551` and
`:554`). Consequence: every palette command with an id ≥ 9 — Export page as
Markdown, Import Markdown, Copy page as Markdown, and every "Jump to page" row
(`CMD_PAGE_BASE + id`) — runs `copy_current_page_markdown` instead of its own
action, and no page can be opened from the palette. The suite is green precisely
because nothing tests the palette dispatch, so `just check` cannot catch this
class of breakage. Adding the name to the import list is a one-word fix; a test
that walks the command registry is the durable one.
