# UI Architecture

How the `.slint` layer is organized, how state flows, and the rules that
keep it agent-maintainable. Status: M2 (shell navigation complete).

## Layering

```
AppWindow.slint        the only Window; composition + keybindings + popup roots
  ├─ shortcuts FocusScope   KeyBindings (Ctrl K/P/N/B/Shift L) → UIState callbacks
  ├─ AppShell               TopBar + Sidebar + Editor + overlay scrim
  │    ├─ TopBar            window controls, move area, breadcrumb, ⋯ menu trigger
  │    ├─ Sidebar           workspace header, Search/Settings rows, PageTree, footer
  │    │    └─ PageTree     ListView over pre-flattened rows → SidebarItem
  │    └─ Editor            ListView over BlockRow → DocumentRow → EditorBlock
  │                         (+ empty state when blocks.length == 0)
  └─ popups                 CommandPalette (Ctrl K), SearchPanel (Ctrl P),
                            ContextMenu (sidebar rows + ⋯), ConfirmDialog,
                            SettingsDialog
```

## State flow (the one rule)

All transient UI state lives in the `UIState` global (`ui/Types.slint`).
Rust owns the models and the truth; the UI only projects it.

- **Data in**: Rust builds `VecModel<struct>`s (`SidebarNode`, `BlockRow`,
  `CommandRow`, `SearchRow`, `MenuRow`) and hands them to `UIState` as
  `ModelRc` (ADR-0005). The UI never mutates business data except through
  callbacks.
- **Actions out**: every semantic action is a callback on `UIState`
  (`node-clicked`, `page-create-requested`, `menu-action`, …), wired in
  `src/app/controller.rs`. Components contain zero business logic.
- **Popups**: opened by setting a flag in `UIState` (`palette-open`,
  `search-open`, `menu-open`, `dialog-open`, `settings-open`); AppWindow
  mirrors the flag and calls `show()`/`close()` (ADR-0008). A single scrim
  in AppShell dismisses whichever overlay is open.
- **Mirrors**: properties Rust must observe (`palette-query`, window
  size) are re-declared in AppWindow with `<=>` and `changed` handlers,
  because `changed` can only watch locally declared properties.

## Sidebar projection

The workspace tree never exists as a Slint tree. Rust flattens it
(`Workspace::tree_rows` → favorites → recents → tree → "New page" row)
into one `Vec<SidebarNode>` with `depth`, `expanded`, `selected`, and a
cumulative `y` (px) per row. `PageTree` is a single ListView over that
flat model; a collapsed node simply has no rows. The `y` offsets plus the
two-way `content-y` mirror let the controller anchor the context menu to
the clicked row without any absolute-position API.

## Scroll mirrors

`Editor` and `PageTree` bind their ListView `content-y` two-way to
`UIState` properties. Normal wheel scrolling still works (Flickable writes
back through the binding); the mirror exists so the benchmark harness can
drive scene F programmatically and the controller can read scroll state.

## Token discipline

Spacing, radii, durations come from `Theme.slint`; colors from
`Colors.slint`; type scale from `Typography.slint`; icons are original
vector paths in `Icons.slint` (dispatcher component `Icon { name }`, plus
direct `Ic*` components for hot paths). No component hard-codes a second
"8px". Zero-length path segments are forbidden — the software renderer
drops them (see `Icons.slint` comment).

## Visual regression

`just shot <scene>` renders the real UI headlessly through the software
renderer (`quire-shot`, ADR-0011) into `.scratch/shots/latest.png`. Scenes
live in `controller::apply_scene` + `apply_scene_overlay` — popups open in
the overlay half so headless two-pass renders see their transitions.
Current set: default, dark, palette, search-notes, menu, rename, settings,
dialog, empty, edit, slash, block-menu, marks, link, find, nest, toggle,
toggle-fold, image, image-half, file, recovered, title-edit, plus dark combos (dark-slash,
dark-find, dark-marks, dark-link, dark-block-menu, dark-title-edit).
`benchmarks/scripts/sweep.ps1` holds the authoritative list — 40 scenes as of
ADR-0030 — and this prose is the summary, so when the two disagree trust the
script. Every visual change ships with re-shot
scenes; the judge-reviewed set is the regression baseline. `toggle` and
`toggle-fold` are a pair on purpose: the same section open and closed, so a
fold that hides the wrong rows shows up as a diff between the two PNGs.
`image` and `image-half` are the same pair for the width tier — one picture
block at 100 % and at 50 %, so a width setting that only moves the label and
not the raster is caught by the row geometry. `file` is the picture scene's
opposite: a 1.8 MB attachment the renderer never opens, so the row is the whole
feature, and its fixture payload has a fixed length because the size label is
the one number it paints. The fixture's process id goes in the temp *folder*
name, not the file name — a label that carried the pid would change every
sweep.

## Adding a component (checklist)

1. Import tokens only (`Theme`/`Colors`/`Typography`/`Icons`).
2. State comes in via `in` properties from `UIState` models; actions go
   out via `UIState` callbacks. No private copies of shared state.
3. Popups: `close-policy: no-auto-close`, never read own `is-open`,
   flag-driven from Rust (ADR-0008).
4. Inside a plain `Rectangle`, give every child explicit `x/y/width/height`
   or layout alignment — Slint centers children that set width without x.
5. Re-shoot `just shot` scenes and have them reviewed.
