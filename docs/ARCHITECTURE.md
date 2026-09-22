# Quire — Architecture

Quire is a local, GPU-accelerated, Notion-like document workspace.
One process. Rust core, Slint UI, no web runtime anywhere.

## Layering

```
        AppWindow.slint (composition only)
              |
   ui/  — Slint layer: visuals, layout, animation, transient UI state
              |  UIState global (properties + routed callbacks)
              v
   app/ — controller: dispatch UI callbacks; state: view-projection data
              |
   core/ — Document/Page/Block/Inline model, commands, undo, selection (M3+)
   storage/ — SQLite repository, migrations, transactional writes (M3)
   services/ — document / search / import / export orchestration (M3+)
   platform/ — thin Windows adapters, only where Slint has real gaps
```

Hard rules:

1. The UI never touches SQL, disk IO, or business-data consistency.
2. The Rust core never knows what color a button is.
3. Single process; background work uses in-process threads only when a
   feature genuinely needs one (search, persistence flush). No IPC.
4. All transient UI state lives in the `UIState` global (`ui/Types.slint`).
   Components do not keep private copies of selection/focus/flags that
   other components need.
5. All semantic editing operations become Commands (M4) from day one;
   Undo/Redo is built on the command history, never on re-reading the DB.
6. Data flows to the UI exclusively through Slint models (`ModelRc`) owned
   in Rust; delegates stay cheap (Text + shapes, not TextEdit — see
   `ui/components/EditorBlock.slint`).

## Why these choices

See `docs/DECISIONS.md` for the rationale of every non-obvious pick
(renderer, SQLite, block model, virtualization strategy, IME policy).

## Directory map

```
Cargo.toml           one package, plus a pinned git dependency; [workspace.dependencies]
                     keeps rusqlite/image on the same version this side does
                     ── the dependency ──
quire-core           github.com/PT123123/quire-core, rev-pinned — everything that
                     stores, with no window in sight (its own repository; the tree
                     below is what lives there, not a directory in this checkout)
  src/lib.rs         module root; the rule: no Slint, no dialog, no clipboard,
                     no platform API anywhere under here (ADR-0093, shipped ADR-0094)
  src/core/          document model, commands, history, database model
  src/storage/       SQLite, migrations, repository, search index, backups
  src/services/      persistence, import/export, search, attachments, settings,
                     the LAN framing a sync module will grow out of
  src/testing.rs     scratch-directory guard for tests
  tests/             the five integration suites that name only the data layer
                     ── this repository ──
.cargo/config.toml   git-fetch-with-cli: cargo's libgit2 cannot fetch a public repo
src/                 the shell
  lib.rs             app + platform, and `pub use quire_core::{core, services,
                     storage, testing}` so the split is invisible from in here;
                     slint include_modules (one compiled unit)
  main.rs            args parse, 8 MB-stack UI thread (ADR-0009), bench timers
  bin/quire_shot.rs  headless visual-regression renderer (ADR-0011)
  app/               controller.rs (callback dispatch), state.rs (view
                     projection + mock content), workspace.rs (pure page-tree
                     model; the seed of core/'s real model)
  platform/          Windows adapters only if forced    (M8)
ui/
  AppWindow.slint    root window, composition, keybindings, popup roots
  Theme.slint        spacing / radius / motion tokens, dark flag
  Colors.slint       palette tokens (light + dark)
  Typography.slint   type scale
  Icons.slint        vector path icons, theme-aware
  Types.slint        shared structs + UIState global
  components/        AppShell, TopBar, Sidebar, SidebarItem, PageTree,
                     Editor, EditorBlock, BlockHandle, CommandPalette,
                     SearchPanel, ContextMenu, Dialog, SettingsDialog,
                     Button, IconButton
tests/
  integration/       workspace + persistence: the two that name app::state
                     (the other five left with the code, and are flat
                     `tests/` in quire-core now)
  fixtures/          editor/storage fixtures land here in M3+
benchmarks/
  scripts/           bench.ps1 (scenes A–G), redact.ps1 (a committed row never
                     carries a machine path), shot2png.ps1, sweep.ps1
docs/                ARCHITECTURE, UI_ARCHITECTURE, EDITOR_ARCHITECTURE,
                     PERFORMANCE, DECISIONS, ROADMAP
```

## Milestones

M0 toolchain/renderer baseline → M1 design system + shell (this state) →
M2 mock navigation → M3 SQLite + persistence → M4 block editor →
M5 slash menu/palette → M6 rich text → M7 performance → M8 Windows RC →
M9 Android. Status lives in `PLAN.md`.
