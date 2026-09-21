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
dialog, empty, edit, slash, block-menu, marks, marks-wrap, link, find, find-grid,
find-cols, find-callout, nest, toggle,
toggle-fold, image, image-half, file, recovered, title-edit, table, table-edit,
table-marks, columns, columns-3, columns-marks, math, math-inline, toc, embed,
embed-empty, code-hl
plus dark combos (dark-slash, dark-find, dark-marks, dark-link, dark-code-hl,
dark-block-menu, dark-block-colors, dark-title-edit).
`benchmarks/scripts/sweep.ps1` holds the authoritative list — 57 scenes as of
ADR-0043 — and this prose is the summary, so when the two disagree trust the
script. Every visual change ships with re-shot
scenes; the judge-reviewed set is the regression baseline. `toggle` and
`toggle-fold` are a pair on purpose: the same section open and closed, so a
fold that hides the wrong rows shows up as a diff between the two PNGs.
`find` and its three siblings are the find bar's set (ADR-0043). `find` searches
a term the page says sixteen times, so its evidence is a box on every one of them
that is not under the caret — 9 inside the 800px frame, 14 in a 3 000px one, and
the two that never appear are the ones in the block the bar stepped into, which
answers with the editor's own text selection instead of a cell. `find-grid`,
`find-cols` and `find-callout` each search a word that lives **inside a table
cell**, inside a box of a columns layout, and inside a callout: the first two have
no row of their own and have to ride on the row that paints them (ADR-0028), and
the third is a frame the runs flexbox did not draw at all until this slice. One
box, two boxes, two boxes, on purpose — each scene's only claim is that a box
appears where the delegate had never been asked to paint one.
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
`marks` and `marks-wrap` are the same feature at two lengths (ADR-0041): a
marked paragraph that overflows its frame, and one long enough to need four
lines of it. The pair exists because the wall this slice moved was exactly a
wrap, and one line of runs cannot show a wrap happening — `marks` alone proved
only that the runs still paint, which is why the second scene had to be added
beside the fix rather than instead of it. `marks-wrap` carries one bold phrase,
one italic word, one code span and one link in the same sentence, so a break
that lands in the wrong place shows up as a decoration that no longer sits on
its own words. Its fixture derives every mark offset from the text with
`.expect("needle present")`: the first version used `unwrap_or(0)`, and a needle
that missed ("break between cells" for "breaks between cells") silently marked
offset 0, which is how a scene comes to demo the wrong words and still look
plausible.
`table-marks` and `columns-marks` are the same feature on its other two surfaces
(ADR-0041). A marked line is drawn by three different delegates — a block, a
table cell, a line inside a column box — and the grid's fixture holds one word
per cell while the layout's holds "Column 1 / Column 2", so neither of those two
had ever seen a run that needed wrapping. Each new scene overwrites the
narrowest box it can reach with a sentence too long for it and one bold phrase
inside: the cell at a third of the grid, the line at half the page. They joined
the set because the delegate edits they cover moved **0 of the 50** scenes that
already existed — a correct reading of that zero requires a shot that would have
failed, and there was none. The find bar's marker (ADR-0043) is the same three
delegates a second time, and it learned the same lesson twice. The hit cell's
`Rectangle` had to be added to all three files, and the layout's arm was the one
that nearly stayed unproven: an earlier read of the `columns` fixture had its
boxes rendering empty, so a swept `find-cols` was said to be a scene that would
paint nothing either way. That read was wrong — the boxes do hold "Column 1 /
Column 2", visible in a crop of a scene that had been in the set for two
milestones — and the scene that was written against the wrong read would have
been the hole. The second lesson is the callout's: its tinted box is declared
*after* the runs in the delegate, Slint paints later siblings on top, and so the
first `find-callout` passed every gate while painting an empty callout. The
census caught it (2 hit cells where 4 were due), the crop said why, and the box
moved above the runs.
`sweep25` → `sweep26` (code highlight) moved **0 of the 52** and added 2:
`code-hl` and `dark-code-hl` — one fixture (the same seven lines the memory bench
feeds itself) as a Rust block, and the same block in the dark theme. That zero is
the reading this time, not a hope: the Language row lives in a code block's ⋮ menu
only, so no swept menu band could move, and the six layers hang off a guard that is
false on every swept row. What the two new scenes are for is the thing no hash can
say — that the colours land on the characters they colour. The first shot of
`code-hl` did not: every layer was a line above the text it stacks on, because a
`Text` with no width never wraps (the trap below), and the fix had to be proved a
second time at 760 px wide, where the fixture's long lines really do break. A
colour census (count the pixels within 3 of each palette slot) is what settles it
without an eyeball: all five token colours present in both themes, and the same
five absent from `default` and `dark` down to 4 and 1 stray pixels.
`sweep26` → `sweep27` moved **all 54**, and that is the one sweep whose whole
purpose was to move them: every shot had been captioning itself `FemtoVG · GL` in
the sidebar footer while `quire-shot` installs `HeadlessPlatform`, which is Slint's
software rasterizer whatever features are compiled in (`--features software` keeps
the `femtovg` default on, so the compile-time `renderer_name()` answered for a
renderer that never ran). A sweep is evidence *about a renderer*, so the shot
binary now overwrites the property with `Software · headless` after `wire()`, and
the app's own label is untouched because the app really does pick the feature it
was built with. `diffbbox` is what makes that a verdict rather than a claim: 53 of
the 54 moved 118–125 px inside `x 96..192 / y 778..784` — the footer's one caption
line — and `settings` moved 244 px across `x 96..722 / y 692..784`, which is that
same line plus the dialog's Renderer row. Two places read the property; two boxes,
nothing else.
`sweep27` → `sweep28` (the light palette measured) moved **45 of the 54** and left
all 9 dark shots byte-identical — the reading a colour-token change has to earn,
since three literals moved and all three are light-only. (Both new arms are the
working tree at `2f1ab57` plus this uncommitted diff: the manifest's commit line
names the last commit, never the tree it was cut from, so the identity of a
pre-commit sweep is its md5 column, not that header.) A palette diff has no
bbox worth quoting: `text-muted` is the block handle, the footer, the sidebar and
every hint row, so `diffbbox` hands back the whole window. The confinement
argument is directional instead. Across all 54 scenes **no pixel got brighter on
any channel** (every token moved down), and the largest single-channel drop
anywhere is 38 — exactly `text-muted`'s red delta, 155 → 117; the two scenes that
cap at 25, `dialog` and `link`, are the two with 0 pixels of full-coverage muted
text in the baseline (`default` has 575), so their ceiling is the same 38 scaled
down by whatever the antialiasing left. Per pixel, a classifier that asks
whether one coverage `a` and one surface `s` put `a·T_old + (1-a)·s` →
`a·T_new + (1-a)·s` there attributed 38 176 of 38 283 sampled changed pixels in
six scenes to one of the three families. The 107-pixel residue is all in `code-hl`,
all ≤4/3/1 per channel, all on an orange glyph edge over a tinted background where
the plain sRGB mix the model assumes is not what the rasterizer did. That model
gets its control from the slice before: run on `sweep26` → `sweep27`, which these
tokens did not author, it calls 897 of `settings`' 1 215 changed pixels
unexplained. It can say no. The old value's own census reads the same way: exact
`#9b9da2` goes 575 → 0 px, and the 1 214 pixels still "within 4" of it are
antialiased edges of the *new* gray at ~73 % coverage, which passes numerically
through the old value's neighbourhood on its way to white.
`sweep28` → `sweep29` moved **2 of 54** and added no scene: the `block-colors`
fixture gained an orange-on-orange and a yellow-on-yellow row, in both themes,
because the two slots this slice darkened had never been painted as anything but
their own menu dot — 120 px of chip, no glyph to read. The census now finds 289 px
of `#bd6408` and 164 px of `#a87718` where the sweep before had 0 of either, with
the dark arm's `#e28d50`/`#dcae3f` at the same rows confirming dark moved nothing.
`sweep29` → `sweep31` (the find bar's own marker, ADR-0043) moved **2 of 54** and
added `find-grid`, `find-cols` and `find-callout`: `find.png` and `dark-find.png`
each gained exactly 4 428 px of their fill (`#ffe9a8` / `#3d3413`) and exactly
756 px of their border (`#bd6408` / `#a87718`). The two themes agreeing to the
pixel is the point — 9 boxes at identical coordinates in both, and only the four
literals differ — and the three new scenes are the shapes a box has where the
text it marks is not a block's own line: one 39×30 box in a grid cell, two 52×30
in the boxes of a columns layout, two 21×30 inside a callout's tinted frame. The
other 52 scenes are byte-identical, which is what "only the rows the bar touched
get rebuilt" has to look like from outside — and which is also, on its own,
worthless as evidence for the three new scenes, since none of them existed to move.
The baseline is `.scratch/sweep31` (57 scenes). The set before it, `.scratch/sweep10`
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
`sweep23` → `sweep24` (marked lines wrap) moved 3 of those 49 and added 1:
`marks` and `dark-marks` at 1 942 / 1 918 px inside the *same* box,
`x 390..1148 / y 196..240` — one paragraph band, two lines tall instead of one
clipped line, and `dark-marks` matching its light twin to the pixel again — plus
`math-inline` at 360 px inside `x 420..664 / y 194..210`. That third one is the
scene that had to be argued with: a marked line that already fit on one line
should not have moved at all, and it had. A shift probe (try dx/dy offsets, look
for the offset that zeroes the difference) said it was not a translation, and an
ink-edge scan said what it was — the right-most painted column went from 661 to
664, ≈3 px of extra spread across the gaps of a line now measured word by word
rather than as one string. Sub-pixel per gap, so the verdict is "wider by the
rounding of eleven measurements", not "broken". 46 scenes were byte-identical,
the two `table` and both `columns` scenes among them — and their being identical
is a statement about their fixtures, which hold no marks at all, not about the
change: the next sweep had to prove that the same wrap reaches those delegates
(`sweep24` → `sweep25`, above).
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
* **A `Text` with no bound `width` does not wrap.** Its width becomes its
  `preferred-width` — the whole string on one line — so `wrap: word-wrap` has no
  frame to break inside and only the newlines the string itself carries. The five highlight
  layers of ADR-0042 each carry the same text as the layer that measures the row,
  and none of them had a width: the measure wrapped, the colours did not, and the
  first `code-hl` shot came back with every token painted one line above its own
  characters. Bind the same width on every layer, and prove it with a fixture that
  has a line long enough to wrap — with nothing to wrap, all six agree and the
  scene passes while the feature is broken.

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
