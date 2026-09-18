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
src/
  main.rs            args, window bootstrap, no logic
  app/               controller.rs (callback dispatch), state.rs (view data)
  core/              document model, commands, history  (M3+)
  storage/           SQLite, migrations, repository     (M3)
  services/          document/search/import/export      (M3+)
  platform/          Windows adapters only if forced    (M8)
ui/
  AppWindow.slint    root window, composition
  Theme.slint        spacing / radius / motion tokens, dark flag
  Colors.slint       palette tokens (light + dark)
  Typography.slint   type scale
  Icons.slint        vector path icons, theme-aware
  Types.slint        shared structs + UIState global
  components/        AppShell, TopBar, Sidebar, PageTree, SidebarItem,
                     Editor, EditorBlock, BlockHandle, CommandPalette,
                     Button, IconButton
benchmarks/scripts/  release benchmark harness (see docs/PERFORMANCE.md)
docs/                ARCHITECTURE, PERFORMANCE, DECISIONS, ROADMAP
```

## Milestones

M0 toolchain/renderer baseline → M1 design system + shell (this state) →
M2 mock navigation → M3 SQLite + persistence → M4 block editor →
M5 slash menu/palette → M6 rich text → M7 performance → M8 Windows RC →
M9 Android. Status lives in `PLAN.md`.
