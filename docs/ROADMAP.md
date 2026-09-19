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
| M3 Local documents | `core/` document model, SQLite storage (workspaces/pages/blocks/metadata), debounced batched persistence, transactions, load-on-restart, settings persistence | next |
| M4 Block editor MVP | Command system + undo/redo, single-TextInput editing, Enter/Backspace/merge/split, arrows, clipboard, 9 block kinds, Chinese IME acceptance pass | core+UI ✅ (IME pass pending user) |
| M5 Notion interactions | Slash menu (command descriptors from Rust), palette wiring on the real command registry, block drag/reorder, block menu | ✅ (drag/reorder = ADR-0021; markdown shortcuts + curated slash/Turn-into menus = ADR-0022; collab/AI/color/copy-link stay out per SPEC) |
| M6 Rich text | Inline spans (bold/italic/code/link/strike), mark persistence, format-safe save/load | ✅ (link UI + import/export marks in; run rendering has a documented wrap limitation) |
| M7 Performance | Virtualization depth, lazy loading, background search (FTS), long-document suite at 1k/5k/10k blocks | |
| M8 Windows RC | Crash recovery, migrations, packaging/installer, Markdown import/export, settings UI, keyboard shortcut pass | |
| M9 Android | Shared core model + schema, adapted navigation/IME test plan (starts only after Windows is stable) | |

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
