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

## Overlay layering

Two tiers, and the scrim is what separates them (`AppShell.slint`'s
`modal-open`): a **modal** — the delete confirm, Settings, the Add-link card —
owns the window, so it is drawn over `Colors.scrim` (`#00000059`), which dims
everything below it including the title bar's neighbours. An **anchored popup**
— slash, the "+" insert menu, the block menu, Move to, palette, search, the
page context menu — points at the block or row you are still working on, so it
gets no dim: dimming the page behind a menu that is asking you to pick a type
for *that* line hides the line. Anchored popups instead auto-dismiss on a click
outside themselves (the engine swallows that outside click while a popup is
open), and `AppShell`'s full-window `Rectangle` + `TouchArea` stay in place for
every overlay flag as the safety net against a `UIState`/popup desync — the
Rectangle is simply `transparent` unless a modal is up.

The empty-page panel is the one click target that is not an overlay: it is a
`TouchArea` inside `Editor.slint` whose only job is to fire
`UIState.empty-page-started()` so Rust can append the page's first block
(ADR-0033). Keep it enabled whenever the page projects zero rows — a page you
cannot click into is the defect that ADR-0033 exists to prevent.

## Visual regression

`just shot <scene>` renders the real UI headlessly through the software
renderer (`quire-shot`, ADR-0011) into `.scratch/shots/latest.png`. Scenes
live in `controller::apply_scene` + `apply_scene_overlay` — popups open in
the overlay half so headless two-pass renders see their transitions.
Current set: default, dark, palette, search-notes, menu, rename, settings,
dialog, empty, edit, slash, block-menu, marks, link, find, nest, toggle,
toggle-fold, image, image-half, file, recovered, title-edit, table, table-edit,
columns, columns-3, math, math-inline, toc, embed, embed-empty,
plus dark combos (dark-slash, dark-find, dark-marks, dark-link, dark-block-menu,
dark-title-edit).
`benchmarks/scripts/sweep.ps1` holds the authoritative list — 49 scenes as of
ADR-0040 — and this prose is the summary, so when the two disagree trust the
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
sweep. `table` and `table-edit` are that pair again: the same 2×3 grid with the
caret in cell three or nowhere, so an editing cell that forgets to mirror the
live text shows up as a diff between them. The grid's hover toolbar is *not* in
the sweep — `quire-shot --hover x,y` exists (ADR-0031) and the strip is checked
with a hand-run shot, because a swept scene that parks the pointer would make
every later scene's hover state dependent on it.
`columns` and `columns-3` are the layout pair (ADR-0032): one line converted into
a two-box layout with a word in each box, and the same layout after the strip's
"Add column". They are a pair because the whole point of the scene is the
*tiling* — a third box that arrives without re-splitting the width leaves the
third word where the second one was. `diffbbox.ps1` measured that pair at 1 111
changed pixels, and every one of them inside the layout's own band (y 205..221),
which is the evidence that the flexbox re-flowed rather than the scene drifting. The layout's hover strip is
kept out of the sweep for the same reason the grid's is.
`math` and `math-inline` are the derived-text pair (ADR-0038): the same renderer
seen from its two surfaces, one line converted into a formula block whose source
is `\frac{a+b}{2} \leq \sqrt{ab} \ne 0 \quad \int_0^1 x^2 \,dx` — a shape for
every branch the renderer knows — and one short paragraph with a `math` mark over
`E = mc^2` inside a sentence. The second is the one that earned its place: it is
the first marked line in the set short enough to leave slack inside its frame,
and the slack is what exposed the stretch-alignment defect above. Neither scene
shows the editing state (`quire-shot` never focuses a row), so a math block under
a caret and Ctrl+M over a selection stay human-verified.
`toc` is the derived-list scene (ADR-0039): the intro paragraph of the sample
page converted into a contents block, so the box sits above every heading it
lists and lists four of them — three at level 2 and `Principles` at level 3,
which is what makes the indent two steps rather than one thing repeated. The
scene converts a block instead of seeding a `Toc` row into the fixture on
purpose: ~30 scenes render that page, and a fixture edit would have moved every
one of them for a reason that has nothing to do with the change under test. What
the scene cannot show is the click — `quire-shot` does not hit TouchAreas — so
hover feedback and the caret landing in the listed heading are human-verified,
and so is the jump's known limit: it selects the heading without scrolling the
viewport (see ADR-0039).
`embed` and `embed-empty` are the derived-text pair for a link (ADR-0040): the
same card with an address in it and with nothing in it. Both scenes convert the
intro paragraph rather than seeding a row, for the ADR-0039 reason, and the pair
exists because the two lines of the card come from two different functions — one
that names who the address belongs to, one that shows what pressing Open will
hand the system. The empty arm is the one that catches a fallback that never
fires: a card with no address has to read as a card ("Embed / No address yet"),
not as a blank box with a button in it. Neither scene presses that button —
`quire-shot` does not hit TouchAreas, and a headless press would launch a real
browser — so the actual hand-off, the mid-edit card while an address is being
typed, and the arrow's hover state are human-verified.
The baseline is `.scratch/sweep23` (49 scenes). The set before it, `.scratch/sweep10`
(42 scenes), re-baselined 37 of them for a
reason unrelated to tables: `DocumentRow.head` bound `height` without `y` and so
was centred in its delegate, which had been sitting the page title ~34 px below
its binding in every scene ever shipped (see the trap below). Since then
`.scratch/sweep14` → `sweep15` → `sweep16` each moved 0 of 44 scenes — the
clipboard paste and the media bench batch touched no UI, and that zero is the
reading rather than an assertion — and `sweep16` → `sweep18` moved
`settings.png` alone, which is the one scene the Reclaim button belongs to
(`sweep17` is the same dialog with the layout defect below still in it).
`sweep18` → `sweep21` (math) moved 4 of those 44 and added 2: `slash` and
`dark-slash` at 621 / 2 483 px inside `x 340..618 / y 532..586` (the menu gained
its "Math — or type $$" row above Divider), `plus` at 10 983 px over
`x 600..878 / y 0..798` (the insert menu is one row taller, so every row it draws
shifted — a whole-band box that the change explains), and `settings` at 77 px
inside `x 548..724 / y 490..500` (the shortcut row now reads
"Ctrl+B / I / E / M"). `marks`, `table-edit` and the two `columns` scenes came
through the `alignment: start` change byte-identical, which is the control that
says it only touches lines with slack to waste.
`sweep21` → `sweep22` (contents block) moved 3 of those 46 and added 1: `slash`
and `dark-slash` at 683 / 2 574 px inside the same `x 340..618 / y 564..618`
(the menu gained its "Table of contents" row above Divider), `plus` at 1 249 px
inside `x 612..864 / y 624..792` (the insert menu's own band, ending where the
list does). Nothing else moved — `settings` in particular, whose shortcut row
this slice did not touch — and the two `EditorBlock` edits that ride along (the
box's top margin, the pointer cursor) changed no scene because both are guarded
by `block.kind == 21`, which is the whole point of the new scene.
`sweep22` → `sweep23` (embed card) moved 3 of those 47 and added 2: `slash` and
`dark-slash` at 774 / 2 528 px inside `x 340..618 / y 596..650` — the same menu
band as the last two rows, one row lower because the list grew again — and
`plus` at 1 062 px inside `x 612..860 / y 656..792`, the insert menu's own band.
`dark-slash`'s box is identical to `slash`'s down to the pixel, which is the
evidence that one verdict covers the pair. Nothing else moved, `toc` and `math`
in particular, and the four `EditorBlock` edits that ride along (the row's top
margin, the body height, the input's x and width) changed no scene because every
one of them is guarded by `block.kind == 22`.
A re-sweep after a
layout change is judged by diffing, not by looking:
`benchmarks/scripts/diffbbox.ps1 -OldDir A -NewDir B` reports the bounding box
of changed pixels per scene, and if every box lands inside the region the change
explains, one verdict covers the whole set.

## Slint geometry traps

Measured on 1.18 while chasing a table that appeared to paint over its own page
title; both cost a rebuild to find and neither is in the docs where you look.

* A child of a **non-layout** parent that binds `height` (or `width`) but leaves
  the other axis unbound is *centred* on that axis — offset
  `(parent.height - self.height) / 2`, exact to the pixel. Inside a ListView
  delegate whose height grows with its block, that silently drags siblings
  around as the block changes, so it looks like the neighbour's bug. Spell out
  `x: 0px; y: 0px;` on anything positioned by hand; checklist item 4 below is
  this rule.
* `absolute-position` read **inside a delegate** is unreliable — it reported row
  1's head below its own body with both at `y: 0` — and reading a sibling's
  absolute position from a binding can trip the core's recursion assert. The
  delegate's real geometry is `parent.y` (the ListView's row offset) plus the
  in-row arithmetic you wrote; that is why `EditorBlock` is handed `row-y`
  instead of measuring itself.
* **A `visible: false` item keeps its slot in a layout.** It is not removed from
  the parent's `HorizontalLayout`/`VerticalLayout`: it still contributes its
  height (or width) and its spacing, and the remaining siblings get the
  leftover. The settings dialog's STORAGE row proved it — adding a third
  conditional Button and hiding a two-line caption under the same flag left the
  hidden caption's ~40 px of height in the row and squeezed the labels until
  "Running in memory — no database attached" elided to three words, all of it
  invisible in the code and pixel-evident in `settings.png`. Anything that
  appears or disappears with a condition is a child of the condition, not a
  child of the layout with a `visible:` binding:
  `if UIState.storage-available : HorizontalLayout { … }`.
* **A layout's default `alignment` is `stretch`, and it spends leftover width on
  its items.** Three `HorizontalLayout`s hold a marked line's runs side by side,
  each item's `min-width` pinned to its `Text.preferred-width` and
  `horizontal-stretch: 0` — which reads as "natural width" and is not: with no
  stretch anywhere, the leftover width is divided among the items anyway. Every
  swept marked line overflowed its frame, so the slack never existed and the
  defect stayed invisible through 44 scenes; the first short one (`math-inline`,
  ADR-0038) came back with three runs ~155 px apart. `alignment: start` costs the
  44 baseline scenes nothing — that is the control, not an assumption.
* **`visible: false` does not stop a binding from running.** An element hidden
  by a kind check still evaluates its property bindings every time its inputs
  change, so a derived `Text` (the math row's `UIState.math-render(...)`) has to
  guard the *binding*, not just the visibility: `is-math ? render(…) : ""`.
  Unguarded, a 10 000-row page calls the renderer 10 000 times per projection to
  paint nothing on 9 999 of them.

## Slint language traps (1.18)

Found while building the columns layout (ADR-0032), where the content of a block
has to be drawn inside that block's own delegate. Each of these cost a compile
error or a rebuild to understand.

* **`for … : if … : Component` does not parse.** A repetition cannot be filtered
  by an inline `if` before the element. So a layout does not iterate its items and
  skip the ones that are not in this box; the projection hands the delegate a
  shape table (`ColumnBox { id, column, first, size }`) and the delegate iterates
  `for ri in bx.size` over `block.column-items[bx.first + ri]`. The grouping lives
  in Rust, where it is a sort, not in QOML.
* **Inside a component instantiation, a bare identifier resolves in the
  *enclosing* component.** `ColumnItemRow { item: block.column-items[…] }` reads
  the parent's `block`, exactly like `EditorBlock { block: block }` has always
  done. Writing `root.thing` there does not reach the parent — `root` is the
  component being instantiated, so it becomes a self-reference. Anything a child
  needs from its parent is passed by plain name (`line-y`, `line-x`, `each-width`).
* **A `for` cannot share one two-way binding across N iterations.** `x <-> parent`
  inside a repetition installs one binding per item on the same property. The
  grid's pointer-claim protocol therefore crosses the component boundary as
  `in property pointer` plus `callback pointer-claimed(int)`: read in, written
  out, one writer per event.
* **`FlexboxLayout` defaults to `flex-wrap: wrap`.** A row of boxes that want
  more room than they have drops onto a second line and the layout silently stops
  being a layout. `flex-wrap: no-wrap` is not optional, and `alignment: stretch`
  is what flex-grow is called here — weighted per item by `horizontal-stretch`.

## Adding a component (checklist)

1. Import tokens only (`Theme`/`Colors`/`Typography`/`Icons`).
2. State comes in via `in` properties from `UIState` models; actions go
   out via `UIState` callbacks. No private copies of shared state.
3. Popups: `close-policy: no-auto-close`, never read own `is-open`,
   flag-driven from Rust (ADR-0008).
4. Inside a plain `Rectangle`, give every child explicit `x/y/width/height`
   or layout alignment — Slint centers children that set width without x, and
   the same happens vertically to a child that sets height without y (see the
   traps above; this one moved the page title in every scene).
5. Re-shoot `just shot` scenes and have them reviewed.
