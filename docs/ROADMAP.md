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
| M8 Windows RC | Crash recovery, migrations, packaging/installer, Markdown import/export, settings UI, keyboard shortcut pass | 🔄 in progress 2026-09-20 · delivered: rotating snapshots + startup recovery (ADR-0015), the `session.meta` + rotating log + panic hook and the clean-exit marker (ADR-0018), data-location move to the per-user library (ADR-0020), schema migrations to v5, Markdown import/export, installer + `just dist`, LAN share/pull, settings UI incl. the STORAGE row (data folder / open-folder / back-up-now), Page and Link-to-page blocks, rich clipboard paste, Move page, the shortcut pass. The tail A-package is landing: A1 `--portable` ✅, A3 release-profile audit ✅ (pinned as ADR-0024), A5 this refresh, A2 first-paint measurement and A4 visual sweep next, A6 installer E2E environment-permitting. Still open: the user IME acceptance pass (M4); menu-overflow polish — a Move-to/mover list taller than the window overflows (the clamp moves the anchor, cannot shrink a menu; a scrollable menu is the fix); Explorer drag-and-drop import is deferred by a platform limit (Slint 1.18 delivers no external file-drop events — revisit on upgrade or via a Win32 `IDropTarget` in `platform/`) |
| M9 Android | Shared core model + schema, adapted navigation/IME test plan (starts only after Windows is stable) | ⏸ blocked on M8 shipping Windows-stable |

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
