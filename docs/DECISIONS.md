# Architecture Decision Records

Format: decision → context → consequences. Newest first.

## ADR-0049 · A template is a page nobody can open, and that costs one column

Decision: SPEC §三十八's template is `pages.template INTEGER NOT NULL DEFAULT 0`
(schema **v14**) on an ordinary page row. There is no `templates` table, no
`template_blocks` table, and no template file format: a template *is* a page
whose block sequence is a body to copy from, and the one column is the only fact
that page carries which an ordinary page does not. It is invisible everywhere a
page shows up — tree, sidebar, palette, Move-to, recents, search, the LAN share —
and it is reachable from exactly two surfaces: the slash / "+"-menu tail that
inserts it into the page being typed in, and ⋯ → **Templates**, whose six rows
insert, start a page from, save, export, import and delete. Five built-ins
(`core::template::PRESETS`) land on a library's first start through the same
import path the menu's Import row uses.

Why one column is the whole representation: §三十八's hard line is 模板的表示必须是
「块序列的副本」，不得引入第二套内容格式. The cheapest way to obey a prohibition is to
have nothing to violate — a template's body is `Block` rows in the same table, so
every field a block can carry (marks, colors, `lang`, `columns`, `img_percent`, a
`Page` reference) rides along in a copy without a single line of new mapping code.
`fill_template` is twenty lines because it clones rows, mints fresh ids from the
document's own allocator so a copy can never collide with its source, keeps the
order keys (a key only means something inside one page, and a new template has
nothing to collide with), and remaps parent links onto the copies — which is what
holds a table's grid and a toggle's children together. The alternative, a
`templates(id, body_json)` column, would have needed a serializer, a deserializer,
a migration for its own inner format, and a rule about which of the two bodies
wins when they disagree.

Why the flag has one write path: `PageCreated` carries a whole `Page`, so
`create_template` records the flag with no change variant of its own, and there is
deliberally **no** `Change::PageTemplateSet`. A page you can open is not a body to
copy from, and a row that flipped the flag on an existing page would need the
whole invisibility apparatus to follow it in both directions. The way to change a
template is the way the menu says: start a page from it, edit that, save it as a
template, delete the older one — which is also why there is no "edit template"
row. The cost of that choice is written down rather than hidden: saving never
overwrites, so the older copy stays in the library until the user deletes it, and
two templates may share a name because the library sorts by age, not title.

Why invisibility is non-attachment: `Workspace::create_template` makes a page and
flips its flag, and never puts it in `roots` or any parent's `children`. So every
enumerator that walks the tree skips a template for free — there is no list here
to remember to filter, which is the property that makes the feature safe to grow.
Four doors do not walk the tree, and each got its own term: `open_page` returns
before `mark_opened`, because opening would write `recents` and the `current-page`
meta — two more places a template must not appear; `search_index::matches` gained
`AND p.template = 0` in its join; and the LAN share filters its page list, its
child walk, and answers **404** rather than the Markdown for a template id, since
the other machine has no way to say "template" and would land it as an ordinary
page. A fifth door was considered and rejected: not indexing a template's rows.
That would make `insert_block` ask whether the page it is filing under is a
template — a second source of truth about a fact one join already has — and
`rebuild` would have to disagree with `insert` about which rows belong, so a
template would start appearing in search after a rebuild. The read-side term keeps
a template unfindable from both doors, and the test that pins it asserts the raw
`search_blocks` count as well as the empty hit list, so the exclusion cannot be
explained by missing data.

Why the built-ins are seeded and not migrated: migration 14 is `sql: ""` plus one
guarded `ADD COLUMN` through the shared `add_page_columns`, shaped exactly like
migration 13's for the same reason — "not a template" is a value, so no backfill
statement is needed and a v13 library opens with none. Writing the five preset
bodies there would mean hand-keeping order keys, `block_children` rows and both
FTS indexes in step, when `import_template` — the very function the menu's Import
row calls — already does all three. So the built-ins are imported, not invented,
and one code path can be wrong instead of two. Two guards make it once-per-library:
a settings flag (`builtin-templates-seeded`), without which deleting all five and
restarting would resurrect them and the menu's Delete row would be a lie; and a
name check, without which a session that died halfway through the seed lands the
library twice. The flag is recorded *after* the bodies, so a session that flushed
nothing retries rather than records a lie. It refuses outright when there is no
library to seed — which is also why the headless bench scenes and the visual
captures paint their own library instead of seeing these five.

Why insert is one command: `Command::InsertForest` lands a whole forest in one
`Ctrl+Z` step, so undoing a template removes eleven blocks rather than leaving
nine of them on the page, and the change list it produces is ordinary
`BlockInserted`s — the flush, the FTS index and a restart all see a page that
simply grew. An empty paragraph at the anchor is *replaced* in the same batch,
because the "+" line and a brand-new page's first row are both empty and a
template that arrives one row below the caret reads as a miss; that is legal only
because `exec_all` plans every command against the pre-state. It returns the first
inserted id so the caret can follow the copy, and `fill_template` records no undo
step at all, because the history stack belongs to the page the user is typing in
and a template page is never open.

Why the menu is one row, and why its labels are short: the page ⋯ menu gained
**Templates** rather than six entries, because the row's label is the feature's
name and the choices belong in the submenu. The submenu's labels then had to fit
the popup every menu in the app shares — `ContextMenu` is 184px wide and its rows
elide instead of wrapping — so they read "Use as new page", "Save as template",
"Export Markdown", "Import Markdown". That is a real loss of explicitness, and the
first render of the scene showed four of six rows ending in an ellipsis. The
judgement: the object is already named twice over, by the submenu the user is
standing in and by the library picker that follows, so the row that survives is
the one a user can read at a glance. Widening the popup was not on the table: it
moves every menu in the app and re-judges most of the baseline for one submenu's
wording.

What the round trip caught, in someone else's feature: the test that exports a
template and reads it back failed, and the defect was in §二十六's channel, not in
templates — `export_page` dropped an **empty** list item, so the row a template
leaves open for somebody to fill in vanished on the way out and came back as
nothing. `prefix_lines` now writes a marker for an empty block (`-`, `1.`, `>`,
`#`) and the numbered arm keeps its count without a trailing space. This changes
ordinary page exports too: a page with an empty bullet now round-trips through its
own Markdown instead of silently losing the line, which is the same deal an empty
heading already made. The rule is now stated in the export header's layout list,
and `an_empty_block_exports_its_bare_marker_and_comes_back_as_itself` walks all
five kinds that can be empty.

Consequences:

* Storage needed nothing else. A template's rows are block rows in the book, so
  the §三十七 reclaim sweep already counts an image block inside a template as a
  referencer through `doc.all_blocks()` — `cover_ids()` and the undo vote did not
  grow a template term, because the flag itself points at no file.
* The repository reads the column `unwrap_or(0)`: a half-migrated library loses a
  template rather than refusing to load, and that is the right direction of
  failure — a template that reads as an ordinary page is visible and editable,
  where a page that refuses to load is nothing.
* `AppState::delete_page` returns whether the deleted page was the one on screen,
  not whether it succeeded, so it answers `false` for every template. Two of this
  slice's tests asserted on that bool and were wrong twice over; they now ask the
  workspace whether the row is there before and after, which is the assertion the
  menu's Delete row actually makes.
* A locked page refuses an insert like any other edit — the gate is
  `exec_all_on_open_page`'s and needs no template-specific term — and still
  offers Save as template, since copying a body out of a page is not writing to
  it. The refusal line must not overwrite the lock's own line, so the emptiness
  notice is gated on `!page_locked()`.
* Search still *indexes* a template, so a library that gains one pays a little
  index and no results; the palette, the sidebar and the Move-to walks pay nothing
  at all, which is the point of non-attachment.
* Track 1's migration priority is spent a third time: v14 is the template flag, so
  any draft numbered 14 or higher moves up — Track 3's 草案 included.
* The page ⋯ menu is thirteen rows now, and the two scenes that show it moved
  differently: `menu.png` at 6 sampled pixels in one column (x 414, y 682..692 —
  the ListView's scrollbar thumb, because that popup was already clamped by
  `min(rows * 30px + 8px, window-h - menu-y - 20px)` before this row existed), and
  `page-lock-menu.png` at 428 sampled pixels across x 240..422 / y 558..640,
  because that one is anchored high enough to draw all thirteen and every row
  below the insertion shifts. Same menu, two anchors; reading the smaller number
  as "barely changed" would be the trap.
* **Hand-test owed**: ⋯ → Templates → each of the six rows, including the two
  `rfd` file dialogs, which no headless capture can reach. That is on top of the
  cover and colour-emoji arms still owed since ADR-0047.

## ADR-0048 · A lock is one column on the page, and a refusal has to be heard

Decision: SPEC §三十八's lock is `pages.locked INTEGER NOT NULL DEFAULT 0`
(schema **v13**) — one bool per page, no table of what is locked within it. It
covers the document: every block's content and the page's own title. It does not
cover the page's look (icon, cover, Style, favourite stay offered), the tree
(move, delete, duplicate), or reading (navigation, search, fold). One command is
exempt inside a locked page: `ToggleFold`. And a refusal is never silent: the
notice bar says which switch to flip, and the page says it too in a pill above
its title, in the ⋯ row that now reads "Unlock page", and in a ⋮⋮ menu left with
only its two read-only rows.

Why one column and not a flag per block is the shape of the sentence the user
clicks: "this page is read-only" is a statement about the whole document, and a
half-locked document — three rows editable, the fourth not — is not a state
Notion has a word for, and not one a migration could reach. So the gate sits
where the writes already funnel: `AppState::exec_editor`, which the 77 call
sites share, rather than 77 places that each have to remember. `NOT NULL
DEFAULT 0` makes the migration one `ADD COLUMN` with no backfill statement, and
a v12 library opens with nothing locked — "not locked" is a value, not an
absence, which is why this column is not nullable where ADR-0047's cover is.

The audit behind "every write": `exec_editor` is the only route to
`core::command::exec`, and the ninety-odd call sites of it and
`exec_on_open_page` share that gate. `grep doc.borrow_mut()` lists what is left,
by name. Four hits are the
bench scene fixtures, which paint a state rather than accept an edit. Two are the
Markdown import paths: they write blocks into a page they created a moment
earlier, and `create_page` hands out an unlocked row, so a lock can never sit
between a user and their own file. Two more are tree operations the lock
deliberately does not cover (`duplicate_page`, `delete_page`). And three are real
entry points beside the funnel: the todo checkbox, whose Slint callback calls the
command layer directly to update one row; `clear_block_ref`; and
`duplicate_page_block`, which mints a child page through the tree *before* the
funnel ever sees the block command.

The last two were holes, and they are the shape worth remembering: both sit
behind a caller that wrote `let _ = exec(...)`, so the refusal was discarded and
the follow-up write landed anyway — a locked page whose Page block lost its
reference, or which quietly gained a second child page. So the rule this slice
adds is not just "gate the funnel" but **a write that follows a command must be
conditioned on that command's result**, and the three entry points beside the
funnel each carry their own gate. The test that pins it asserts the child-page
count as well as the block list, because a refusal that leaves the tree fatter
than it found it is still a write.

Why fold is the exception: §三十七 already files it as the one command whose
result is persisted *view* state, and locking a page must not cost the user its
outline. A named exception inside a guard is exactly the thing that rots, so it
is pinned by an assertion that `ToggleFold` still returns `Some` while the ten
commands beside it return `None`.

Why the notice bar carries the refusal rather than a disabled cursor: the
section's own words are 不能静默吞输入, and a click that does nothing is
indistinguishable from a click that missed. The bar is sticky until dismissed,
so the dedup reads what the bar currently says — the drag-hover path answers
once per frame of one gesture, and rewriting the same sentence sixty times is
its own defect. The hover check itself stays silent on purpose: the absence of a
drop line is that gesture's feedback, and the drop that follows is the moment
worth a sentence. The ⋮⋮ menu's editing rows are **dropped**, not greyed,
because that menu has no disabled state to grey them with.

Consequences:

* The lock is a page property, so it is not a Ctrl+Z step (ADR-0044's rule for
  a look). That is different from *undo being disabled*: the stack built before
  the lock survives it intact and is refused while it is on, so unlocking hands
  back exactly the history the user had, which the test asserts by re-typing
  into the same rows afterwards.
* A duplicated page does **not** inherit the lock, which is the one place this
  slice departs from ADR-0047: font, icon and cover are parts of what a page
  looks like, so a copy that dropped them would open differently from how it
  looked; the lock is a gate on editing, and duplicating a locked page is what
  a user does to edit something like it without touching the original.
* The refusal is therefore two gates, not one. Rust refuses the write;
  `EditorBlock.editing` carries a `!UIState.page-locked` term so the caret
  cannot survive a lock/unlock/relock cycle into a row that would otherwise
  still be typing. A gate on the command layer alone would leave a live input
  on screen that refused nothing until Enter.
* Storage stays boring on purpose: `locked` joins the guarded
  `add_page_columns` list, `insert_page` and the load `SELECT` carry it, and
  `Change::PageLockedSet` is one `UPDATE`. It reaches neither `cover_ids()` nor
  the undo vote in the reclaim sweep (ADR-0037/0047), because a lock stores no
  pointer to any file.
* It stays out of the Markdown channel (§二十六 carries content; a page's
  permissions are not its content) and out of the LAN server, which builds
  `Page` values to read and export and never writes one.
* Track 1's migration priority is spent again: v13 is the lock, so any draft
  numbered 13 or higher moves up — Track 3's草案 included.
* The page ⋯ menu grows a twelfth row. `ContextMenu` height is
  `min(rows * 30px + 8px, window-h - menu-y - 20px)`, so in the anchored-low
  baseline scene the popup was already clamped before this row existed — which is
  why `menu.png` moves as its scrollbar thumb (8 sampled px, one column at
  x 414) and nowhere else, exactly as it did when the cover added a row. A new
  menu row cannot clip a command; it can only make the thumb shorter.

## ADR-0047 · A cover stores the attachment, and the veil's worst case is arithmetic

Decision: SPEC §三十八's cover is stored as `pages.cover INTEGER NULL` (schema
**v12**) holding **an `AttachmentId`** — never a path, never a filename. The
bytes stay the attachment store's, exactly as they do for an image block. The
hero draws them in a band (`Typography.size-cover-height` 168 px) with the page
title inside the band's bottom edge, under one fixed veil
(`Colors.cover-scrim` = `#0000009e`), and the ink over that veil is
`Colors.text-on-accent` for **both** the title and the page's own emoji. Entry
point: page ⋯ → **Set cover** / **Change cover** (one id, the label answers
"is there one?") and **Remove cover**, which only exists when there is. Like
Style and icon, setting a cover is not a Ctrl+Z step, and a duplicated page
starts with its source's.

Why an id and not a path is the same answer ADR-0046 paid to reach: §三十七's
reclaim deletes "the attachments nothing points at", and it can only answer that
question out of the database. A path in a column is a string no sweep can resolve
back to a file, so the user's cover would be the next reclaim's casualty. The
column is therefore **nullable rather than `DEFAULT 0`** — `''` is unambiguous
for an emoji because no emoji is empty, while `0` is a perfectly good attachment
id, so "no cover" had to stay a third thing. The referencer list got two entries
rather than one: `workspace::cover_ids()` joins the reclaim sweep beside the
blocks, and `Change::PageCoverSet` answers `attachment_ids_in`, because an
outstanding undo step is still a pointer — the test that pins this deletes an
image block, restarts the session so undo no longer votes, and requires the
picture to survive *as the page's cover* and then to go as soon as the page lets
go.

Why one fixed veil instead of reading the picture: §三十八 requires the title's
contrast to pass §二十一 and forbids 最弱配色, but the app cannot ask an arbitrary
JPEG what its brightest pixel is without decoding it at hero size on every page
open, and a per-page adaptive colour would be a derived value wanting storage that
ADR-0039 refuses to give it. A constant veil makes the requirement arithmetic with
a bound that holds for every picture: the scrim is black at α = 0.6196, and because
that α is a byte (`0x9e` = 158) the composite is exact — **any** photo pixel lands
at ≤ 255 − 158 = **97** sRGB, a relative luminance ≤ 0.1195, and white ink on that
is **6.19:1**, against a 4.5:1 floor whose own threshold is α ≥ 0.535. The worst
case is a pure-white picture, so
`create_solid_fixture` writes exactly one and `page-cover-white` /
`dark-page-cover-white` are the control scenes; `benchmarks/scripts/contrast_probe.ps1`
then reads the rendered PNG and reports the ratio off pixels (worst ground
`#616161`, 152 002 px measured), with a known-answer arm inside it — 21:1, 1:1 and
6.19:1 — and a synthetic must-fail scene at 1.61:1 so the gate is shown to be able
to say no.

Consequences:

* A picture has no typeface, so the two band constants live in `Typography`, not
  in the per-page `PageType` — but the band's *height* is derived:
  `max(168px, hero bottom + spacing-md)`, because an icon above the title pushes
  the whole hero down and a constant band would leave the white ink standing on
  the page background, which is the one thing the veil cannot protect.
* The first pixel pass found a defect this ADR had not anticipated: the hero's
  emoji still read `Colors.text-primary`, i.e. a `#1f2328` rocket on a `#232439`
  photo — relative luminances 0.0165 against 0.0191, a **1.04:1** ratio, i.e.
  invisible. It now takes the same conditional as the title.
  The headless software renderer draws an emoji as a monochrome glyph so this is
  measurable; the GPU renderers draw colour bitmaps and ignore `color`, so that
  arm is still a hand test (as it has been since ADR-0045).
* Track 1's migration priority (v11–v13) is spent here: v12 is the cover, and any
  draft numbered 12 or higher has to move up.
* The cover stays out of the Markdown channel (§二十六 carries content, and a
  page's picture is not its content) and out of undo, both per ADR-0044's rule for
  a look.
* The page menu is now eleven rows and its popup was already taller than the space
  below its anchor at ten, so the one baseline scene that moved (`menu.png`, 9
  sampled px) moved as the ListView's **scrollbar thumb**, not as a clipped row.

## ADR-0046 · A page's icon is an emoji, and a picture goes on the cover instead

Decision: SPEC §三十八 asks for "icon：emoji 选择器 **+ 本地图片**". The emoji half
shipped as ADR-0045; the local-image half is **decided against, here, for this
slot** — `pages.icon` holds an emoji or nothing, and a page's picture lives in
**cover**, the next group in the same section. This is the "write the cost down
rather than leave the line open" branch of the choice.

The cost is not the picker, it is ownership and the one tier that exists.
Attachments (ADR-0029/0030) keep one file plus **one** downscaled copy at
`MAX_EDGE = 1280` — sized for the ~780 px editor column, 6.5 MB of RGBA at
full size — and `display_path` hands the UI that copy. An icon slot is 16 px in
the sidebar and 46 px in the hero. Riding the existing channel means either
painting a 1280-edge raster into a 16 px box on every sidebar row (the tree
rebuilds on every mutation, so that working set is paid per rebuild for a mark
the size of a letter), or adding a second tier — a second file per attachment,
a second entry in `remove`, and a second thing the M10 reclaim has to know about.
That last one is the real price: reclaim deletes "the attachments nothing points
at" by walking block references, and an icon is a **page** pointer, so a page's
icon is exactly the kind of file that gets reclaimed as garbage by a scanner that
only knows the referencer it was written against. Making that safe is a
referencer list, not a flag.

The column would change shape too. `pages.icon` today is a glyph, and `""` means
unset; a file reference makes it "either an emoji or an attachment id", which is
the tagged union this schema has avoided everywhere it has a choice (`kind` and
`lang` are strings for that reason). And the value is thin: a photograph reduced
to 16 px is the least legible version of the picture the user just put on the
page, and Notion itself keeps its icon picker emoji-only with uploads on the
cover — so this is not a parity gap being papered over.

Consequences:

* §三十八's icon line now says emoji, and points here; the picture requirement is
  carried by the cover line, which already asks for 换图 / 移除 and a contrast
  check — a raster with somewhere to be legible.
* The reopen trigger is concrete: if a raster ever has to live in the 16 px slot
  for another reason (a workspace mark, an avatar), then the second downsample
  tier and the reclaim referencer list get built **together**, and this ADR is
  where the cost was already written down.
* ADR-0045's "local image belongs to slice 3" is superseded by this line — slice
  3 is cover, and it inherits the picture half of the requirement.

## ADR-0045 · A page's icon is the emoji itself, and the placeholder is read three ways

Decision: SPEC §三十八's icon is stored as `pages.icon TEXT NOT NULL DEFAULT ''`
(schema v11) holding **the emoji character**, not an index into the picker. The
picker's whole catalogue lives in Rust (`core::icon::PICKER`, 96 emoji in 12 rows
of 8, with `PER_ROW` shared with the `.slint` grid) and is copied into a
`ModelRc` at open, so no `.slint` file names an emoji. Entry point: page ⋯ →
**Set icon**, which replaces the menu with a `IconPicker` popup rather than
nesting under it.

Why the glyph and not an index: an index makes the catalogue a wire format, so
adding one emoji in the middle silently re-points every page written before it.
Storing the glyph means this list can grow, shrink or reorder and nobody's page
changes — which is also why the entries are written as `char` escapes in the
source, several being a base code point plus a variation selector that an
"invisible characters" cleanup pass would quietly drop.

The harder decision is what "未设置时用标题首字符占位" means in three places,
because the empty slot does not mean the same thing in all of them:

* **Tree rows** show the title's first character (`icon::slot`). That slot held
  a generic page glyph, which said nothing about *this* page; an initial says
  something.
* **Favorites / Recent rows** show the stored emoji *only*, and otherwise keep
  the star and the clock. Those two marks are the section's own identity — a
  placeholder there erases information rather than standing in for missing
  information. Hence two functions, `slot` and `icon_mark`, not one.
* **The editor hero** draws nothing above an iconless title. Repeating the
  title's first character at 46 px directly above the title at 40 px is an echo,
  not a placeholder.

The initial is the first non-whitespace **scalar value**, not the first grapheme:
a title opening on a ZWJ sequence would split, which for a 16 px slot is a
cosmetic risk taken on record rather than a new unicode dependency.

Migration 11 is one conditional `ALTER TABLE` and no rows, and it goes through a
new shared `add_page_columns(conn, &[(&str, &str)])` that migration 10's body was
rewritten to call too. The helper exists because every late `pages` column asks
the same question ("is it there yet?"), and a partially-migrated file — a column
an older build added by hand, a backup restored mid-step — should converge
instead of erroring on a duplicate name.

Consequences:

* The sidebar's parent rows had to give up a second box: a parent already spends
  its 16 px slot on the chevron, and adding an icon box next to it pushed every
  parent's label one indent past its own children. The emoji now shares the
  chevron's box and hands it back on hover, so the tree's geometry is unchanged
  for every row that has no icon.
* Emoji are drawn through one named face, `Typography.emoji-font`
  ("Segoe UI Emoji"), which is the only place in `.slint` that names a font —
  Slint resolves per glyph, and the token exists to make the choice visible
  rather than to be load-bearing. Under the headless **software** renderer the
  emoji come out monochrome and follow the text colour, so `dark-page-icon` was
  added to the sweep to answer whether the mark survives the dark theme. The GPU
  renderers were **not** measured here — whether they paint the colour layers is
  a hand-test item, not a claim in this file.
* Setting and clearing an icon are persisted as `Change::PageIconSet` and are
  **not** on the Ctrl+Z stack, same as Favorite and Style before them: a property
  of the page is not an edit to the document. `duplicate` carries the source's
  icon in memory and in the persisted `PageCreated`, which is the bug ADR-0044
  caught once already.
* The picker's grid is data, so the `.slint` side has no emoji list to drift:
  `fill_icon_picker` copies `PICKER` into the model on every open, and adding a
  row to the catalogue is a one-line change in one file.
* Gate: **no RAM bench owed**, and the reason is the shape — one short string per
  page, one property write on open, and one `Text` per row that replaced a `Text`
  that was already there. The pixel evidence is the substitute, and it is the
  wide kind: **all 64 existing scenes moved**, 72 937 px between them, because
  every iconless tree row now shows an initial where it showed a generic glyph.
  The movers are the icon column and nothing else — the modal scene is
  1 153 px inside **x 13..52 / y 365..713**, which is the 16 px slot at depths
  0–2 — and re-running the same comparison restricted to **x 53 and beyond**
  returns **0 px for 63 of the 64 scenes**, with `menu.png` the exception at
  345 px inside x 254..415 / y 723..775 (its own popup, one row taller). So the
  labels did not shift a pixel — the indent fix below is measured, not assumed —
  and the document area moved by zero everywhere, which is the number that says
  an iconless row costs exactly what it cost before.
* Sweep 64 → **67** scenes (baseline `.scratch/sweep33`): `page-icon`, `icon-picker`
  and a dark arm. The picker scene deliberately opens on an iconless page, since
  that is the state a user is in when they reach for it.
* The hero icon is display-only this slice: there is no hover "Add icon" strip
  above the title, so the menu is the only way in. That affordance and the cover
  behind the title are the next slice's, and they want the same seam.
* Remaining §三十八 groups: **cover**, **lock**, **version history** (still owes
  the disk-and-RAM retention numbers), **templates**. The local-image half of
  icon is not on that list — ADR-0046 decided against it the same day, and the
  picture requirement now sits on cover.

## ADR-0044 · A page's look is derived, so no block ever holds a size

Decision: SPEC §三十八's three switches — font (default / serif / mono), full
width, small text — are stored on the **page** (`pages.font TEXT`,
`pages.layout INTEGER`, schema v10) and reach the screen through one new global,
`PageType`, that *derives* the document tier from three `UIState` properties. No
block gains a size, a family or a flag. Every document-tier call site was
repointed from `Typography.*` to `PageType.*` — 119 of them across four files —
and the chrome (sidebar, menus, palette, settings, and a block's own caption
line) still reads `Typography`, which is why it provably cannot move.

Why a second global instead of overriding at the call sites: the alternative was
119 ternaries on a page property, i.e. the same token question re-asked at every
site, which is the rule the type scale exists to enforce. And why not mutate
`Typography` itself: it is the one place chrome and document share, so a page
that wants small text would shrink the sidebar with it. `PageType` is a *child*
of `Typography` — it multiplies the document tier by one factor and re-points
the family — so the two tiers stay separable and the derivation is readable in
one file.

The three switches are stored as one string and one bit field, not three columns.
`font` is TEXT because every other catalogue in this schema is a string
(`blocks.kind`, `blocks.lang`), and because the failure mode of an integer code is
that a reordering silently re-points every existing row: `PageFont::try_from_str`
turns `"comic sans"` into `Default`, so a page written by a future build opens as
an ordinary page rather than as an error. `layout` is one INTEGER because the
two switches always travel together — `Change::PageLayoutSet` carries both, the
menu writes both, and the reader tests two bits — and two BOOLEAN columns would
add a second `ALTER TABLE` to a step that already has to be conditional on each
column's absence.

Small text is one factor, 0.87, applied to the whole document tier including the
headings. That is 13.5 / 15.5 — the ratio between this app's own code and body
sizes, which keeps the two tiers' relationship intact. Line heights are **not**
scaled: they are factors of the natural line box, so a smaller face already gets
a proportionally smaller box, and multiplying them too would tighten leading on
top of shrinking glyphs.

Two seams needed a second look. Marked runs name an italic *family* rather than
setting `font-italic`, so a serif page would have italicised its emphasis in
Segoe while every other word was Georgia — hence `PageType.italic-font-family`.
And the app-wide `code-probe` measures the monospace advance that the highlight
layers (ADR-0042) wrap against: it now reads `PageType.size-code`, so a small-text
page's colour layers stay measured against the glyphs they colour.

Consequences:

* The page menu gained a Style submenu (Back, three fonts with a check on the
  live one, Full width, Small text) and the whole rest of the sweep is
  byte-identical: 56 of 57 scenes unchanged, `menu.png` the only mover at 1 430 px
  inside x 240..423 / y 542..779 — the submenu's own box. A token change that
  touched chrome would show up as 56 movers.
* The seven new scenes are the menu's own shot plus six measurements, each taken
  against `default.png` with a self-vs-self control at 0 px: serif 63 638 px,
  mono 62 780, small text 55 285, full width 51 533, all three at once 68 779
  against the serif shot, and the dark arm 63 140. Full width is the only one
  whose bounding box starts at x 284 (sidebar 260 + `Theme.spacing-xl` 24) where
  the others start at 390 — that is the gutter moving, and it is the number that
  separates a real layout switch from a font swap.
* `duplicate_page` carries the source's style, and `workspace.duplicate` had to be
  fixed to match: it created the copy with defaults while the persisted
  `PageCreated` carried the source's look, so a duplicated page read differently
  before and after a restart.
* `open_page()` re-applies the three properties, so the derived tier follows
  selection with no per-page model churn, and a page with no stored look costs
  three writes of the defaults.
* Migration 10 adds two columns and no rows: a v9 library's every page reads
  `font = ''`, `layout = 0`, i.e. exactly what it looked like before, which is
  what `the_v10_step_adds_the_page_look_to_a_v9_database` insists on.
* The remaining §三十八 groups are untouched: icon, cover, lock, version history
  and templates still need their own decisions — `pages.layout` is a bit field
  with room left, but this ADR does not spend it.

## ADR-0043 · A find hit is a cell, and its border is the marker

Decision: the in-page find bar (SPEC §二十) paints every occurrence as **one
word-run cell** (ADR-0041). `TextRun` gains `hit: bool`; `build_runs` takes that
block's hit ranges as two more byte boundaries each; the projection carries them
as `FindHits = HashMap<i32, Vec<(usize, usize)>>`; and the delegate draws a
`Rectangle` under the glyphs of any cell whose `hit` is set. A hit that starts or
ends mid-word **splits the word**, because a cell is the smallest thing this
layout can paint and a cell painting half a match would be the same defect with a
tint on it.

Why not ADR-0042's colour layers: that trick works because a code block is
monospace, so six copies of one string stack glyph for glyph. The editor body is
proportional — there is no byte offset that predicts a pixel — so a per-character
colour is not available here. A background is: the runs flexbox already cuts a
line into cells at word boundaries, so adding the hit's two offsets as boundaries
makes a cell that is exactly the occurrence, and a `Rectangle` behind it is the
same decoration the inline-code box already uses.

Two rules fall out of the cell model. A hit **never cuts a mark**: a marked
stretch is one `Text`, and a formula's cell shows glyphs that its own byte space
does not describe (`\alpha` reads α), so a hit that lands inside a mark tints the
whole mark instead of part of it. And a row with neither marks nor hits keeps
`runs: []`, which is what tells the delegate to draw one wrapping `Text` — so a
search that never started costs nothing, and one that is closed costs a re-paint
of the rows it had touched.

The repaint is targeted rather than a re-projection. `paint_find_hits` walks the
row model once and rebuilds the rows that carry a hit **now or carried one on the
previous search** (`AppState::find_painted`), because the bar does this on every
keystroke and a 10 000-block page must not re-project to repaint a dozen cells. A
hit in a grid cell or a layout box has no row of its own (ADR-0028), so
`row_id_of` walks it up to the row that paints it and rebuilds that row's flat
`table_cells` / `column_items` lists whole — the hit rides on a row that is not
the one the block owns.

The palette pair is `Colors.find-hit` (fill) and `Colors.find-hit-border`, and the
split is measured, not styled: a fill bright enough to be a 3:1 marker on its own
would take every coloured-text pair below the floor ADR-0023 accepted (an amber
that reaches 1.39:1 on white drags the palette's own yellow text to 2.85). So the
fill stays pale and says only "this cell" — light #ffe9a8 keeps body text at
13.14:1 (from 15.80) and its worst pair at 3.29 (from 3.95); dark #3d3413 keeps
10.02 (from 14.06) and 3.83 (from 5.37) — and the **border** carries the marker:
#bd6408 and #a87718 read 4.21 and 4.39 on their page and 3.50 and 3.13 on the
fill they sit in, so the marker clears 3:1 against both things it touches.

Consequences:

* One hit is always exactly one cell — a hit's own range is `covered`, so the
  word-cut never runs inside it, and two adjacent boxes mid-match cannot happen.
* The hit box and the inline-code box are the same geometry, and the hit is
  declared second. A search that lands on a code span shows the match and loses
  the span's gray background: the bar's job is to say where the text is.
* `project_blocks`, `table_cells` and `column_projection` each take a `&FindHits`,
  which is `&FindHits::new()` at every call site that is not the find path. An
  empty map costs a row one failed hash lookup, so the closed bar is not free but
  is one lookup per row wide.
* The active hit is **not** distinguished from the other hits by this slice. It
  already is by the editor's own selection (the bar steps the caret there), and
  the two overlays read differently in both themes — so a "current match" colour
  is a separate decision with a separate measurement, not a missing flag.
* `find`, `find-grid`, `find-cols` and `find-callout` are the pixel evidence, one
  per surface a match can land on: a block's own line, a grid cell, a box inside
  a columns layout, and a callout's tinted frame. Each of the last three was a
  scene before it was a passing scene — `find-grid` needed the search term
  changed to a word that only exists inside the grid, because with the bar's own
  "the" it painted seven boxes on the page and none in the cell it was meant to
  prove.

* The runs flexbox is now the text renderer for a **quote and a callout** as
  well, not just for the plain kinds. They hold one line of body text each, and
  each draws it through its own single `Text`, so a match inside one had nowhere
  to sit while the counter still counted it: on the swept page that was 2 of 16
  hits, found by measuring the shot rather than by reading the code. They share
  the flexbox and bring only their own frame — `runs-x` / `runs-y` /
  `runs-width` / `runs-height` answer per kind, and the plain case reads exactly
  the expressions it used to.

* Paint order is part of that frame. The callout's tinted box was declared
  *after* the runs, and Slint paints later siblings on top, so the first version
  of this ADR's callout arm passed every gate and painted nothing: the box
  covered its own cells. The box and its emoji moved above the runs; the emoji
  and the text still sit on top of the box, which is the only ordering the block
  actually needs. The symptom was an empty callout — the single `Text` had
  stepped aside for runs, and the runs were there but hidden — which is the
  shape this trap always takes.

* A hit whose block is **being edited** is not a cell, and that is not a gap.
  Stepping the bar moves the caret onto the match, so that one block answers
  with the editor's own text selection; its other occurrences lose their cells
  for as long as it holds the caret. On the swept page 16 hits are 14 cells plus
  the 2 in the block under the caret, counted at a 3 000px-tall window so that
  nothing was simply scrolled off.

## ADR-0042 · One colour is one layer, not one run

Decision: a highlighted code block paints as **six copies of the same string**.
`core::highlight::layer(text, lang, frame, advance, kind)` lexes the block, gives
every character one of six colours, and returns the whole block with every
character that is not `kind` replaced by `U+00A0` — plus a hard newline wherever a
line passes a column budget (`floor(frame / advance)`; ASCII is one column, a tab
eight, anything else two). `code-text` asks for kind 0 and five `Text`s in one
conditional `Rectangle` ask for the other five, drawn 1, 3, 4, 5 and then 2. All
six have the same length, the same characters per line and the same hard newlines,
so they stack glyph for glyph and no second layout engine has to agree with the
first.
The row's height stays `code-text`'s, which is the point of counting kind 0 as a
layer: the string that measures the row is one of the strings that colour it.

Two seams hold it up. The advance of one character comes from **one** invisible
probe `Text` in `Editor.slint` that writes `UIState.code-advance` from a `changed
width` handler — not from a row, because which rows a `ListView` realizes depends
on the scroll position and where a block's lines break must not; `0px` before the
first measurement means "add no hard breaks", which is the safe answer since
Slint still wraps. And the colours are `Colors.code-token`, which maps the five
token kinds onto five slots of the block palette this app already has (keyword =
purple, comment = gray, string = green, number = orange, name = blue) rather than
shipping a second palette tuned for the same two backgrounds.

Language is the one new fact a block stores: `Lang` (Plain / Rust / Python / Js /
Ts / Md / Json / Bash) in `blocks.lang`, migration v9, undoable as
`Change::BlockLangSet`, picked from a Language submenu that only a code block's
⋮⋮ menu shows, and carried by the fence's info string in both Markdown directions
(```` ```rs ```` comes back as Rust and leaves as ```` ```rust ````; a language with
no lexer folds to `Plain`, which is no colour rather than a broken block).

Why: Slint 1.18 `Text` paints one colour, and ADR-0041 had just made the runs
channel break at word boundaries — but a run is still one layout *cell*, so a
per-token colour could not wrap and highlight was parked behind that wall. Six
whole-block strings dodge the wall instead of climbing it: wrapping is Slint's
job on each layer, the lexer never has to know about words, and typing pays
nothing because the layers exist only while the block is not being edited
(`is-highlighted && !editing`), so no keystroke lexes.

Consequences:

* **A character whose width this model cannot know is copied rather than blanked.**
  A non-ASCII character goes into all six layers verbatim, so on a line that
  carries both a comment and non-ASCII prose the comment colour wins — it is drawn
  last. That is the documented boundary (a CJK comment reads gray, a CJK string
  reads green, a CJK string on a commented line reads gray), and it errs towards
  showing the character rather than towards the wrong colour.
* **The model assumes a monospace font**, and `clip: true` on the layer Rectangle is
  the floor under that assumption: if the theme's font is not quite one, a layer
  ends up taller than the measure and the row keeps the measure's height.
* **Two defects that only a shot could catch, both invisible in the code.** A layer
  `Text` with no `width` does not wrap — it breaks only at the lexer's hard
  newlines — so the five layers drifted a line away from the measuring layer, and
  the first `code-hl` shot printed text that read as scrambled. The fix is
  `width: root.code-width` on every layer. And `visible: false` does not stop a
  binding from running, so those five lexer calls fired on every repaint of every
  row of the page until the Rectangle became a conditional element. Both rules are
  now in `docs/UI_ARCHITECTURE.md` with the box they showed up in.
* Six `Text` elements on a highlighted row (the measuring layer plus five colours);
  a plain code block has one, and every other row of the page gains none, because
  the `if` that guards the layers is the same guard the six are behind.
* No JS/WASM runtime and no new dependency, which is what SPEC §三十七 asked for.
  The lexer is ~800 lines of switch over six language families; a real tokenizer
  would be its own ADR.
## ADR-0041 · A marked line breaks where its words do, because a run is one layout cell

Decision: `build_runs` (src/app/state.rs) emits one run per **word** inside an
unmarked stretch, and leaves a marked stretch whole; all three places that draw
runs — `ui/components/EditorBlock.slint`, `TableBlock.slint`,
`ColumnItemRow.slint` — become `FlexboxLayout { flex-wrap: wrap }` where they
were a `HorizontalLayout`. Nothing else about the channel changes: each row's
height is still the invisible plain `Text` beside it — the one that already
measures the line count for unmarked text at the same width — the runs box is
still `clip: true` at that height, and the live `TextInput` still shows plain
text while the block is focused. Whitespace stays attached to the word it
follows, so the cells re-join to the original string byte for byte, which is
what the two new tests assert (one with a fixed expected cell list, one over a
leading mark, CJK text and a mark at the end).

Why: Slint 1.18 `Text` has no inline formatting at all — no per-run style, no
decoration, no `TextFormat` — so a mark can only be painted as a separate item,
and that was the documented platform wall (`docs/EDITOR_ARCHITECTURE.md`
§"Platform wall", A4's one open HIGH): a paragraph carrying marks lost its word
wrap and clipped mid-word, while the identical unmarked paragraph wrapped. A
layout cell cannot break, so the old per-mark run "the same delegate that
paints a ten thousand line page" was one unbreakable item. Cutting the plain
stretches to words hands the flexbox the same break opportunities the shaper
has. Marked stretches stay atomic on purpose: a link split at every space gives
each fragment its own underline and its own click target, and a code span split
per word is a row of separate boxes.

Consequences:
- **The wall moved; it did not vanish.** Two shapes still clip, and both are
  now the *narrow* case rather than the normal one: a marked phrase long enough
  to exceed the line on its own, and a marked line whose runs need more lines
  than the same words unmarked — bold and mono are wider than regular, and the
  height authority is the plain-text measure, so the extra line has nowhere to
  go. Fixing the second means giving the runs container its own height, which
  this slice tried first: binding `body-height` to the flex's
  `preferred-height` is a Slint compile error (`Cannot access id 'runs-flex'` —
  an element declared inside an `if` cannot be named from outside it), and
  hoisting the `if` so the flex always exists costs two items on all 10 000
  bench rows. The word cut is the version that fits the existing geometry.
- **No migration, no new kind, no model field.** `user_version` stays 8, the
  runs channel is still `Vec<TextRun>`, and storage, undo, Markdown and the LAN
  export are untouched — this is one pure function's output shape and one
  layout element.
- **The gate had to be fixed before it could be read.** Scene D has no marks at
  all, so the old 10 000-block RAM gate was blind to this change; publishing its
  ratio would have been a measurement that only looked like one. So the harness
  gained `--marks N` (main.rs, `HandleArgs.marks`, `bench_marks`, quire-typing,
  `bench.ps1 -Marks`) and `--dump-state` now prints `marked=N`, which lets each
  arm prove its own fixture instead of taking the script's label on faith. All
  twelve rows read `gs+atlas-blocks=10024 marked=1000`.
- **Gate: 1.03× and 1.02× the control's private bytes, in two batches** (raw
  rows `benchmarks/results/2026-09-21-m8-wordwrap-ram.jsonl`, 12 of them).
  Control = `2c5a25f` built in a clean worktree with *only* the bench knob
  ported, so its runs still render on one line (md5 `cdf6da5a…`, 22 074 368 B).
  Scene D, 10 000 blocks, 1 000 of them marked, arms alternating, one pinned
  database per arm, each arm's seeding run excluded:
  batch 1 (this tree md5 `f59edb1b…`, 22 081 024 B) control 112.7 / 111.4 →
  this tree 115.2 / 115.9 = **1.031×**; batch 2, after the table and column
  delegates joined (`4f9205ca…`, 22 087 680 B) control 114.3 / 111.1 → this
  tree 114.8 / 115.7 = **1.023×**. Both are well inside the ≤1.2× gate, and the
  honest reading is at the gate's own resolution: the +2.5…3.5 MB between the
  two means is smaller than the 3.6 MB the *control* arm spread across by itself
  in batch 2. What the two batches do pin down is that the paragraph change has
  a shape — the marked rows go from 3 runs to 11 cells each, and a cell is an
  item with its own measured `Text` — and that the two delegates scene D never
  realizes cost nothing on it, which is exactly what they should.
- **The projection cost of the cut, measured in both arms** (an `#[ignore]`d
  timing test in `state.rs`'s module — the same shape ADR-0039's projection
  number takes, and re-taken at another commit by copying that one test into a
  worktree, which is `#[cfg(test)]` code and touches nothing under measurement;
  50 rounds of `project_blocks` over 10 000 rows): unmarked 41.686 ms control /
  41.504 ms this tree — the two arms agree to 0.4 %, which is the control that
  the fixture and the machine are the same — and 1 000 rows marked 44.147
  (+2.461) / 46.942 (+5.438). So the word cut costs ≈2.8 ms per projection of a
  page whose every tenth line carries a mark, ≈2.8 µs per marked line, and
  nothing at all on an unmarked page.
- **Pixels.** `sweep23` → `sweep24`, 49 → 50 scenes: 3 moved, 46 byte-identical,
  1 new. `marks` and `dark-marks` both at 1 918–1 942 sampled px inside
  x 390..1148 / y 196..240 — one paragraph band, and the new shot shows the
  marked line as two complete lines instead of one clipped one. `math-inline`
  moved 360 px inside x 420..664 / y 194..210, which needed a second probe
  because it is the scene where the change should be invisible: an ink-edge scan
  puts the right-most glyph column at 661 before and 664 after, i.e. ≈3 px of
  extra line spread from measuring words separately instead of one 40-word
  string — sub-pixel per gap, and the shift probe says it is not a translation
  (a pure dx/dy shift does not zero the difference). New scene `marks-wrap`
  exists precisely because the old `marks` fixture could not show four lines of
  mixed marks: a ~380-char sentence with a bold phrase, an italic word, a code
  span and a link, and its needles now `.expect("needle present")` instead of
  `unwrap_or(0)` — silently marking offset 0 is how `marks` once demoed the
  wrong words.
- **Pixels, second half: the other two run rows, and a zero that had to be
  argued with.** `sweep24` → `sweep25` moved **0 of the 50** existing scenes and
  added 2 (`table-marks`, `columns-marks`). That zero is not the evidence — the
  two new shots are, because a wrapped cell and a wrapped column line are
  exactly what no scene in the baseline contained: the grid's fixture holds one
  word per cell and the layout's holds "Column 1 / Column 2", so the delegates
  changed and nothing was there to show it. Both new scenes put an over-long
  marked sentence into the narrowest box each surface has — the cell at a third
  of the grid, the line at half the page — and both render their wrapped lines
  with the bold phrase on the line the shaper put it on, the row grown to
  `cell-text.height` / `plain-text.height` around them. A scene that could not
  fail is how this slice would have shipped a silent no-op.
- **Still needs a human window.** No headless scene proves: clicking to a caret
  position on a *wrapped* marked line (the overlay is hidden while editing, so
  the mapping is plain-text, but it should be watched), a marked paragraph that
  grows a line while you type in it, a link whose cell is now one word rather
  than the whole phrase (hover target, and whether a single-word underline
  reads as a bug), and a marked CJK paragraph, which has no ASCII spaces to cut
  on and therefore keeps the old one-cell-per-mark behaviour entirely.

## ADR-0040 · A link card derives its two lines at paint time and hands the address to the system

Decision: `BlockKind::Embed` (row id 22, database string `"embed"`) stores the
address in `blocks.text` and nothing else. The card's two lines are computed
while drawing, through two pure callbacks on `UIState` — `embed-label(string)`
and `embed-url(string)` — whose Rust side is `core::embed::describe` and
`with_scheme`: the headline is the provider the host belongs to (21 named hosts,
plus Google's products read from either the subdomain or the first path
segment), or the host itself when nothing matches, or `Embed`/`Link`/`Email` for
the three shapes that have no host to name. The second line is the address the
Open button will actually use, so a bare `example.com/a` reads as
`https://example.com/a`. Clicking the card edits `block.text` the way a paragraph
does; clicking the arrow fires `open-link` and leaves the app. There is no
WebView, no fetch, no favicon and no oEmbed (SPEC §二 and §三十三), which means
the card *is* the feature rather than a stand-in for one. Markdown writes the
address alone on a line; import reads back only a line that is one token and
carries an explicit `http(s)://`.

Why: a link block that stored a title would be a copy of something that can
change, with no way to refresh it and no owner to invalidate it — the same
argument ADR-0039 makes for the contents block, and here it is cheaper still
because the derived value is a string function rather than a page walk. Storing
the raw address in the existing `text` column, rather than a url field or a JSON
payload, is what keeps this the fourth new kind in a row with no migration. And
the bare-address Markdown shape is the one a GFM renderer already turns into a
link: a `<…>` wrapper or a `<!-- quire:embed:… -->` marker would be an extra
thing to corrupt when the text is not a well-formed address, and it would render
as nothing everywhere else.

Consequences:
- **No migration, as a checked fact.** `user_version` stays 8. There are now 23
  block kinds, `BlockKind::ALL` lists them, and an unknown kind is still
  corruption-on-load, so an older build that meets an embed fails loudly instead
  of dropping the row.
- **Nothing is added to the row model.** The card's lines are pure callbacks
  evaluated for the rows that draw them, so a page with no embed pays nothing
  and a page with one pays two string functions per visible embed row — the
  ADR-0038 lesson applied where the alternative was per-row model fields.
- **The address is exported verbatim, without inline-mark rendering.** A url is
  full of `_ * ~ &`, and running it through the mark renderer would both change
  the address and make the card open something else than it shows.
- **Import is deliberately narrower than export.** `is_bare_address` demands one
  token and an explicit scheme, so "see https://example.com for details" stays a
  paragraph. The control test asserts exactly that, because the easy way to make
  the round trip pass is to widen the rule and lose prose.
- **A link's target is data, so the shell branch is now allowlisted.**
  `open-link` reaches `cmd /C start` on Windows, and `start` will *run* a path, a
  UNC share or an installed protocol as readily as it opens a url. `core::embed::
  is_openable` (http/https/mailto) now gates that branch. This is defence in
  depth, not a fix: a compiled probe showed std quotes the argument, so
  `… & echo PWNED` reached `start` as one literal string and did not split — the
  injection was never there. What did change is behaviour: `file:///…`,
  `\\share\x`, `javascript:…` and a bare `C:\…\calc.exe` in a link now do
  nothing at all, which is the intended trade for a document that can arrive
  from an import or the LAN server.
- **The gate found nothing again.** Control `4fa2b7b` from a clean worktree
  (md5 `9c5e153f…`, 22 017 024 B) versus this tree (md5 `19aea07c…`,
  22 072 320 B), scene D at 10 000 blocks, arms alternating, one pinned database
  each (raw rows `benchmarks/results/2026-09-21-m11-embed-ram.jsonl`). Steady
  private bytes: control 110.9 / 112.1, this tree 111.8 / 112.3 — **1.005×**,
  inside the ≤1.2× gate, and the 0.55 MB gap between the two means is *smaller
  than the 1.2 MB spread inside the control arm alone*, so the reading is no
  measurable cost rather than a small one. Each arm's first run (109.7 / 111.3,
  startup 807 / 1104 ms) is the pass that seeds its database and is excluded. The
  exe grew 55 296 B for one module, two callbacks and the card.
- **Pixels.** `sweep22` → `sweep23`, 49 scenes: 3 moved, 2 new (`embed`,
  `embed-empty`), 44 byte-identical. The three are the menus that gained a row —
  see `docs/UI_ARCHITECTURE.md` for the boxes.
- **Still needs a human window.** No headless scene presses Open, so these are
  unverified by this build: the browser actually coming up, the card mid-edit
  (typing an address and watching the headline re-derive), hover on the arrow,
  the dark-theme card, and Turn into → Embed → back to Text.

## ADR-0039 · A contents block stores that it is one, and reads its list off the page

Decision: `BlockKind::Toc` (row id 21, database string `"toc"`) holds no
content. Its body is the page's own headings, collected in `project_blocks` by
`toc_entries(blocks, shown)`, which walks the very list of visible indices the
projection just computed and keeps the rows whose kind answers
`BlockKind::heading_level()`. The row carries them as
`toc-entries: [TocEntry]` — `{ block, label, level }` — and the delegate paints
one line each, indented `(level - 1) * 16px`. Clicking a line fires
`callback toc-jump(int)`, and Rust walks the path a `quire://block/` anchor
already walked: `flush_pending_edit` → `set_editing_id(-1)` (recreate the
delegate so the input takes over) → `focus_block`. Markdown writes one marker
line, `<!-- quire:toc -->`, and reads that line back as the block.

Why: SPEC §三十七 批次 C says 派生数据不入库, and a copy of the headings is the
one derived thing in this app that is guaranteed to go stale — renaming a
heading would leave a directory listing the old name, with no edit that fixes
it. Deriving at projection time makes the copy impossible by construction and
costs no migration, because kinds are strings in the database (ADR-0030's
argument, now on its fourth kind). Reusing `shown` rather than re-walking the
page is the same argument one level down: a heading behind a fold or inside a
container has no row to scroll to, so a contents list built from a second,
independent walk would advertise links that go nowhere — and the two walks
would eventually disagree.

Consequences:
- **No migration, as a checked fact.** `user_version` stays 8. There are 22
  block kinds, `BlockKind::ALL` lists them, and an unknown kind is still
  corruption-on-load, so an older build opening a library containing a TOC
  fails loudly instead of dropping the row.
- **The walk is guarded by kind on the Rust side.** `toc_entries` runs only for
  a row whose kind is `Toc`, so a page with no contents block pays nothing for
  the feature and a page with one pays exactly one page scan per projection —
  the lesson ADR-0038 learned for the math binding, applied where the cost
  would be a scan rather than a string conversion.
- **Not editable, still selectable.** `editing` excludes kind 21 the way it
  excludes a divider, a picture and a page block: the row has no text to hold a
  caret, and the live TextEdit would show a source string the block does not
  have. Its TouchArea reports `MouseCursor.pointer`, because a contents line is
  a target rather than a place to type.
- **A converted line keeps its words, unpainted.** `SetBlockType` into `Toc`
  leaves `blocks.text` alone (the same precedent `Divider` set), so a paragraph
  turned into a contents block keeps its text in storage and in the FTS index
  while nothing draws it. That is inherited, not new, and it is why export
  writes the marker rather than the text: the marker is the only part of the
  block that means something outside this library.
- **The click moves the caret and does not scroll.** Slint 1.18's plain
  `ListView` has no `bring-into-view` — that function lives on
  `StandardListViewBase`, the fixed-row-height list a `ListView` gets its
  scrolling from — and nothing in `i-slint-core` moves a Flickable's viewport
  when a child takes focus. So a jump to an off-screen heading selects it
  without revealing it. `quire://block/` anchors from M8 have always behaved
  this way; the TOC inherits the limitation rather than hiding it, and a
  variable-height `reveal` is its own slice (it would fix both).
- **The gate ran against a same-session control and this time found nothing.**
  Control `cc7ccf0` from a clean worktree (md5 `a81ce6df…`, 21 928 960 B) versus
  this tree (md5 `f6d9a576…`, 22 017 024 B), scene D, arms alternating, one
  pinned database each (raw rows
  `benchmarks/results/2026-09-21-m11-toc-ram.jsonl`). Steady-state private
  bytes: control 110.5 / 112.1, this tree 112.5 / 111.6 — **1.007×**, inside the
  ≤1.2× gate, and the 0.75 MB gap is smaller than the 1.6 MB spread inside the
  control arm alone, so the honest reading is *no measurable cost* rather than a
  small one. Each arm's first run (112.7 / 110.0) is the pass that seeds its
  database and is excluded. The exe grew 88 KB: one enum arm, one struct, one
  callback and the delegate's contents list.
- **Pixels.** `sweep21` → `sweep22`, 47 scenes: 3 moved, 1 new (`toc`), 43
  byte-identical. The three are the menus that gained a row — see
  `docs/UI_ARCHITECTURE.md` for the boxes.
- **Still needs a human window.** No headless scene clicks a contents line, so
  hover feedback, the caret landing in the heading, and a contents block on a
  page with no headings yet (it paints "No headings on this page yet") are
  unverified by this build.

## ADR-0038 · A formula is stored as source, and its picture is derived on the way out

Decision: math is two surfaces over one renderer. `BlockKind::Math` (row id 20)
keeps LaTeX-subset source in `blocks.text`; `MarkKind::Math` keeps the same
source inline, as a span over the bytes *between* the `$`s. Neither stores a
rendered string. `core::math::to_unicode` is the only renderer, reached twice:
the block row asks for it through a `pure callback math-render(string) ->
string` on `UIState`, and `build_runs` calls it while projecting inline runs, so
the `TextRun` it hands the UI already holds glyphs. No layout engine, no schema
change, no migration. And because this scene is the first marked line in the
sweep short enough to leave slack inside its frame, all three run rows gain
`alignment: start` — a Slint layout's default `stretch` had been spending that
slack as gaps between the runs.

Why: SPEC §三十七 批次 C asks for a LaTeX subset and sets the bar at "渲染优先
Unicode 近似排版", gating a typesetting engine behind its own ADR plus memory
numbers. Unicode approximation crosses the bar without triggering that gate: no
engine, so no numbers to owe. Storing source rather than glyphs is what makes
the renderer replaceable — when an engine does arrive it replaces one function,
and no library written before it needs migrating, re-exporting, or a different
search index. It also keeps the two-way `text <=> UIState.editing-text` binding
honest (the user edits the formula, never its picture) and keeps Markdown a
round trip instead of a one-way render.

The renderer's contract is three rules, and they are what its tests pin:
- **the output never loses what the user typed.** An unknown command comes back
  as its own source, an environment likewise, so the worst reading is "this did
  not render" and never "this vanished";
- **whitespace in the source is content, not syntax.** TeX discards spaces in
  math mode; this does not, because in a single-line Unicode fallback the space
  the user typed *is* the only surviving expression of their spacing
  (`\alpha + \beta` and `\alpha+\beta` render differently, on purpose);
- **it is idempotent.** `to_unicode(to_unicode(x)) == to_unicode(x)`, which is
  what lets a row re-derive its text on every binding evaluation without
  drifting.

Consequences:
- **No migration, as a checked fact.** Kinds and mark kinds are strings in the
  database (`BlockKind::as_str` / `MarkKind::try_from_str`, read back in
  `storage/repository.rs`) with no CHECK list to widen — ADR-0030's argument for
  `file`, repeated because it keeps paying. `user_version` stays 8. There are 21
  block kinds now, and an unknown kind is still corruption-on-load, so an older
  build reading a math library fails loudly instead of silently dropping rows.
- **A derived binding is a per-element cost, not a per-kind one.** The math Text
  is `visible: false` on every other row, and invisible elements still evaluate
  their bindings, so the text reads `is-math ? UIState.math-render(…) : ""`.
  Without the guard a 10 000-row page would call the renderer 10 000 times per
  projection for paragraphs that have no formula in them. The cost is measured,
  not assumed: ≈0.61 µs per formula (`to_unicode` over five representative
  sources, release build, `core::math::tests::cost_per_formula`, `#[ignore]`d
  because it prints), i.e. ≈6 ms per projection for a page whose every line
  holds one inline formula — arithmetic on that measurement, not a frame this
  build has been observed to miss, and no whole-page projection is on record. Inline runs pay it at *projection* time for the same
  reason: a binding would pay it per repaint instead.
- **An empty formula still has to look like one.** A `math` block with no text
  renders `$$`, so the row has height and reads as a formula slot rather than a
  blank band; the block stays in `editing`'s editable set, so clicking it opens
  the source in the live TextEdit and the rendered Text hides itself.
- **The `$` guard is a pair, and both halves are the same predicate.** Import
  opens a span only TeX's flanking rule allows — a `$` followed by a non-space,
  closed by a `$` preceded by a non-space — so "costs $5 and $10" stays prose.
  Export escapes a `$` only when a pair could really re-form on the way back
  (`dollar_pair_ahead`), so prose dollars survive without a `\$` on every price.
  A `$$ … $$` fence is verbatim the way a code fence is, because `\alpha` must
  arrive with one backslash.
- **A formula span is the outermost thing on its range.** `kind_order` /
  `mark_order` put Math last (5), and export drops any mark a Math span
  contains: `$**x**$` has no reading in a renderer that does not parse markup
  inside a formula. A space-padded Math mark has no Markdown spelling either, so
  it is dropped rather than exported as a fence that would not re-open.
- **The new scene caught a real defect, and the pixel evidence is the
  baseline.** `math-inline` is a short marked line — the first in the sweep —
  and it came back with ~155 px gaps between three runs. Cause: the run
  `HorizontalLayout`s default to `alignment: stretch`, so leftover frame width
  was divided among the runs; the 44 existing scenes never showed it because
  every one of their marked lines overflows its frame and gets clipped
  (that clipping is the separate, already-documented platform wall in
  `docs/EDITOR_ARCHITECTURE.md` §"Platform wall"). `alignment: start` on all
  three run rows (block, table cell, column line) moved **0 of the 44 baseline
  scenes**, which is the control that says the fix only touches lines that had
  slack to waste.
- **The gate ran with a same-session control build, which 批次 B owed.** Both
  arms measured in one sitting, alternating, on their own pinned databases
  (raw rows `benchmarks/results/2026-09-21-m10-math-ram.jsonl`): control =
  `2e9de99` from a clean worktree (md5 `57eefe68…`, 21 873 152 B), math = this
  tree (md5 `5bfeceea…`, 21 929 472 B). Scene D steady state: control 135.7 /
  135.8 MB WS and 109.8 / 110.7 private, math 136.6 / 136.7 and 111.7 / 112.4 —
  **1.016× the control's private bytes**, inside the ≤1.2× gate. The +1.8 MB is
  on a page containing no formula at all, and the within-arm spread is 0.9 MB,
  so it is real but small and unattributed (the exe grew 56 KB; the rest is
  assumed to be the symbol tables and the extra model arm). Each arm's first run
  (143.3 / 143.7) is the seeding pass and is excluded.
- **What it does not do** (boundaries, not defects): no display-style layout, so
  `\frac{a}{b}` is one-line `a/b` and `\int_0^1` is a glyph plus Unicode
  sub/superscripts where the font has them, else `^(…)`; `\begin{pmatrix}`
  echoes verbatim; no KaTeX parity; no math font (the row uses the UI face); no
  `\( … \)` or `\[ … \]` delimiters; the `file`/`image` style of per-block
  affordances is absent — a formula has no menu of its own beyond ⋮.
- **Still needs a human window.** No headless scene shows a math block *being
  edited* (source in the live TextEdit) or Ctrl+M over a selection, because
  `quire-shot` never focuses a row. The two new scenes prove the derived
  rendering; they do not prove the editing path.

## ADR-0037 · Orphaned attachments get one reclamation path, and it cannot outrun undo

Decision: `reclaim_attachments()` in `AppState`, reachable as a **Reclaim**
button on the settings dialog's STORAGE row, deletes every `attachments` row
nothing can reach any more and, once the row is gone, the files beside it. The
reachable set is deliberately wider than "what is on screen": every block of
every page in `Document` (not just the open one), every attachment id named
inside any `Entry` on any page's undo **or** redo stack, and the block sitting in
the internal clipboard. It is the only emitter of the new
`Change::AttachmentDeleted`, and it reports through the same `db-notice` toast as
"Back up". The ordering rules are the substance: flush the debounced queue first,
write the row deletions synchronously, then delete bytes.

Why: orphans have three producers and no consumer. Undo removes a *reference*
and never the file, which `Change::AttachmentAdded` has said out loud since
ADR-0029 ("an orphaned picture is recoverable, a deleted one is not");
`delete_page` is not an undo step, so its pictures are unreachable the moment
the page goes; and `replace_all` leaves the whole table alone because it cannot
see the incoming references — the comment there called the cost "orphans, which
no user action can see". A local-first app with no cloud quota can live with
that for a while, but not forever, and the file it cannot see is the one it
cannot explain. The scan errs generous because the two failure directions are
not symmetric: leaving an orphan costs disk, deleting a picture Ctrl+Z was about
to restore costs the user's bytes.

Consequences:
- **The undo contract is unchanged, and now has to be proved.** A step still on
  the stack protects its ids from the sweep, in both directions — redo is one
  keystroke from putting a block back exactly like undo is. `History` grew the
  enumeration API it never had (`referenced_attachments()`), because until now
  nothing outside the module needed to know what the stacks hold.
- **The cap is the boundary of the promise.** 100 steps per page is what a
  picture is protected for, not "forever". `both_stacks_protect_and_the_cap_ends_
  the_protection` pins that boundary, and the settings row says in one line what
  the button deletes, because a toast is not a warning the user reads first.
- **One reader for "does this change name an attachment".**
  `core::persistence::attachment_ids_in` matches the arms that carry an id
  (`BlockInserted`, `BlockAttachmentSet`, `AttachmentAdded`) and nothing else, so
  the undo-shape tests and the reclaim cannot disagree about which arms count.
  `BlockDeleted` is not one of them: it is the change that *drops* a reference,
  and its undo carries the `BlockInserted` that names the id again.
- **Rows before bytes, queue before either.** Applying a DELETE out of order
  ahead of a still-queued `AttachmentAdded` would re-create the row after the
  sweep deleted the files it points at — a permanent dangling reference. So the
  reclaim flushes first and refuses to sweep if that write fails. A failure in
  the other direction only leaves bytes no row claims, which the next sweep
  removes.
- **What it does not reclaim: a file with no row.** A directory sweep looks
  tempting and is the one way to lose a library: if `load_attachments` failed
  this session, the in-memory book is empty, every file on disk looks
  unreferenced, and "I cannot see the references" would be answered by deleting
  them. The reclaim therefore reads the book and never the folder, so a broken
  load removes nothing. Unrowed bytes stay a known limitation.
- The decode cache gives its weight back for a deleted row (`evict_image`), so
  the 32 MiB ceiling ADR-0036 measured keeps accounting for rasters that can
  still be asked for.
- A reclaimed id can be **minted again** after a restart: `next_attachment_id`
  is `max(row ids)+1` at load, so reclaiming the highest row lets the next
  session reuse that number and its `<id>.png` name. Inside one session it
  cannot happen, and the session's own cache entry is evicted, so no stale
  raster survives the reuse.
- **It runs on the UI thread, so its cost is a frozen window and it was
  measured.** 1 000 orphans sweep in 549.9 / 556.7 / 567.6 ms (raw rows
  `benchmarks/results/2026-09-21-m10-reclaim-timing.jsonl`; an independent
  earlier batch read 530.6 / 542.3 / 573.0), and the same call with an empty
  book reads 0.0 ms — so the reachability scan is not on the clock and ≈0.55 ms
  per orphan is the `DELETE` transaction plus the `remove_file` calls,
  unattributed between them. That is a fraction of a second to a couple of
  seconds for a library a user actually accumulates, which is why there is no
  progress UI: the notice bar is the feedback, and a second click on a sweep
  that already ran deletes nothing new. Numbers in `docs/PERFORMANCE.md`.
- **The sweep cannot show the new button, and showed the row's other half
  anyway.** `storage-available` is false for the headless shot tool (it builds
  `AppState::new(&args, None)`), so no scene renders Reclaim — the settings
  dialog with a real database still needs a human window. What the re-sweep did
  catch is that a `visible: false` item keeps its slot in a Slint layout: the
  two hidden buttons were already costing the label width (it elided two words
  early) and the third made it worse, while the hidden caption below them cost a
  band of empty height. Both are visible in sweep17's `settings.png`; the row's
  actions are conditional children now, and sweep18 is the baseline.

## ADR-0036 · A picture page is measured by scrolling it, and the scroll had the wrong sign

Decision: give the media benchmark a real scene instead of another deferred
bullet. `--pictures N` turns every `rows/N`-th row of the bench page into an
`image` block; the fixtures behind them are `create_fixture`-generated 1280×720
gradient PNGs, written **only when the library is fresh**, so the measured passes
load a page that already exists rather than re-encoding one. `bench_picture_plan`
is the single source of the stride and the 200-file pool, which is why the row
builder and the seeder cannot drift apart. The app reports its own decode-cache
state — `attachment_cache_report()` prints one JSON line to stderr, and `--dump-state`
is what makes the bench harness read it — because a process working-set counter
cannot see a cache the size of a raster. `docs/PERFORMANCE.md` carries the nine
scenes.

Why: four consecutive batches ended with the same sentence — *a page of pictures
being scrolled is unmeasured, and the 32 MiB ceiling is a construction bound plus
a unit test, not a reading* — and a unit test is exactly the wrong instrument for
a claim about a ListView, a frame clock and a GPU texture upload. The seeding rule
is the honest half of the design: if the fixtures were built inside the timed
window, the arm would measure PNG encoding. The pool is capped at 200 rather than
one file per picture row because 5 000 rasters of distinct 1280×720 PNGs would be
a disk and warm-up artefact, not a memory measurement — and the cache cannot tell
the difference anyway, since it bounds bytes, not identities.

Consequences:
- **Scene F had been scrolling nowhere since M2.** `ListView`'s `viewport-y` is
  negative downward; the timer did `cur + 8.0`, so every 16 ms tick wrote a value
  the clamp immediately rounded back to 0, and the "continuous scroll" arm measured
  a stationary page. This is not a reading to reinterpret — it is every scroll
  number in this document taken before today. The fix is `cur - step`, plus a stall
  counter that only wraps when a position the app actually *put* there is the one
  that comes back.
- **A null reading was the bug, and pixels were the only proof.** The batch's first
  two runs printed an unchanged cache and the same CPU as the static scene. Rather
  than conclude "images are cheap", `--scroll-y` was added to `quire-shot` as a
  pixel control: at `0` and `+2000` it wrote **byte-identical PNGs**, and only
  `−2000` differed. That is the harness defect, photographed. It also means the
  contaminated rows were deleted and the whole batch re-run on the fixed binary;
  the pre-fix file is kept out of the repo (`%TEMP%\media-ram-pre-fix.jsonl`) as
  the record that the instrument, not the app, was wrong.
- **Scroll tests now need a direction test.** "The property changed" is not
  evidence of motion. `--scroll-step` exists so a flick can be measured separately
  from a wheel tick (83–88 % vs 36–38 % of a core), and `--scroll-y` stays as the
  control any future scroll arm should run before quoting its CPU.
- **The readings, briefly.** The cache stops at 9 rasters / 33 177 600 of
  33 554 432 bytes (98.9 %) in every scrolled media arm, at 500 or 5 000 pictures
  and on 1 000 or 10 000 rows — the ceiling is reached before the viewport could
  show a tenth frame, so it needs no raising. 500 pictures on a page you never
  scroll to is indistinguishable from no pictures. And an on-screen picture costs
  ≈9 MB of process memory, not the 3.7 MB of its raster: **the viewport, not the
  32 MiB cache, is what constrains a photo page**, which is the number §二十二's
  arithmetic now has to carry.
- `MUST NOT` the media arms into the §三十七 ≤1.2× gate. They are reported outside
  it deliberately — that gate's job is to catch a per-row projection regression on
  an ordinary page, and a scene that adds GPU textures would fail it for the wrong
  reason. ADOPT the same-session control for the batch's own arms (A, D and the
  media shapes all ran in one sitting), which is what the layout batch asked for.
- **Invalidating a number obliges re-running it, in both arms.** `scroll_ab.ps1`
  measures the fixed scene against femtovg and skia/GL in one sitting, each
  binary pre-flighted against its own reported renderer so a skia build that kept
  the femtovg default cannot pass for an A/B. The result: the verdict's direction
  survives — skia is ≈18 % cheaper at a wheel tick and ≈21 % at a flick — but the
  size halves (the M7 row read 18.2 % vs 27.7 % on a page that was not moving),
  and the price is ≈+34…43 MB of working set while skia's *private* bytes are a
  wash or lower. femtovg therefore stays the default on the same grounds as
  before, with "skia for scroll-heavy use" now a proven and smaller offer. The
  media ceiling turned out to be renderer-independent — skia reports the same 9
  rasters and the same 33 177 600 peak — because the LRU lives in `AppState`,
  above whichever renderer is painting.
- **A filter that matches nothing is not a measurement, and it used to exit 0.**
  Reproducing the new matrix scenes through `-Only A,B` found that
  `powershell -File` hands a comma list to a `[string[]]` parameter as **one
  string**, so a multi-token filter matched no scene, wrote no rows and reported
  success — the silent-probe shape this harness has been bitten by before.
  `bench_matrix.ps1` now splits the tokens and throws when nothing matched. The
  header comment has advertised the two-token form since scene E's follow-ups, so
  any older batch that claims a `-Only A,B` re-run is worth checking for rows
  before its numbers get quoted.
- `create_fixture` writes a gradient, which compresses well and is therefore not a
  photo; the RAM arms above are unaffected (weight is `w·h·4` regardless) but the
  disk footprint and any future decode-time arm are optimistic. Recorded as a
  limitation, not fixed here.

## ADR-0035 · A screenshot decodes in a file that has never seen the clipboard

Decision: 批次 A's last open code item — paste a picture from the clipboard with
Ctrl+V (SPEC §三十七, §二十七's "clipboard rich content") — is split into three
parts that can each be tested without the others. `platform::read_clipboard_image()`
touches Win32 only: it picks `CF_DIBV5` over `CF_DIB` with
`IsClipboardFormatAvailable`, retries `OpenClipboard` five times like the text
reader does, and copies `GlobalSize` bytes out. `platform/dib.rs` turns those
bytes into PNG and contains no Win32 at all. `AppState::paste_image()` decides
which block receives the picture. In `on_rich_paste` **text outranks image**: the
picture branch runs only when `read_clipboard()` returned nothing. No clipboard
crate, no new `image` feature — the PNG encoder the paste needs was already
compiled in for the store.

Why: ADR-0025 settled that clipboard access is hand-declared FFI, and this is
that decision's second half rather than a revision of it. The split is what makes
the risky half testable: a DIB is a header plus a bottom-up pixel array whose
masks, palette, row padding and bit widths are all chosen by whatever process
wrote the clipboard, and every one of those is a decoder bug — while none of it
needs a clipboard, so eight asserting tests build DIBs by hand and run on any
machine without touching the user's real one (the standing rule here is no
scripted desktop UI: no synthetic key events, no foreground stealing).
`CF_DIBV5` is
tried first because it is the only one of the two that can carry a real alpha
channel; `CF_DIB` is what everything actually writes.

Consequences:
- **The zeroed-alpha trap.** A 32-bit `BI_RGB` screenshot has no alpha, so its
  fourth byte is whatever the source had — usually all zero — and PNG *keeps*
  transparency: pasted through unchanged it renders an invisible picture, which
  the user reads as a failed paste. `decode` therefore paints the raster opaque
  when an alpha mask is declared and *every* alpha byte is zero, and leaves a
  picture with one non-zero alpha byte alone. Two tests pin both sides.
- **A hostile header stops being our problem.** `parse` accepts header sizes
  40/56/108/124, bit counts 1/4/8/16/24/32, and only `BI_RGB`/`BI_BITFIELDS`;
  it refuses `width > 16 384`, `height > 16 384` or `width × height > 64 M`, and
  checks the pixel array is actually there before allocating it. Without those,
  one buggy or malicious clipboard write — a header claiming 30 000×30 000 —
  would be an out-of-memory exit on the paste path, i.e. §二十二's low-RAM
  promise broken by a Ctrl+V.
- **Text wins because pasting a paragraph as a picture of itself loses the
  paragraph.** A "Copy" from a rich app puts both formats on the clipboard, so
  the order is a behaviour decision, not an accident of implementation.
- The paste reuses `AttachmentStore::import_bytes`, which is how a picked file is
  stored, so a screenshot gets the same `MAX_EDGE = 1280` downscaled display
  copy, the same content-sniffed extension and MIME (always `.png` here), and the
  same undo contract (ADR-0029: undo drops the reference, never the bytes). It
  stores under the name "Pasted image". The cost of the reuse is two decodes for
  one picture — DIB→RGBA→PNG, then the store's PNG decode — timed in
  `docs/PERFORMANCE.md` rather than assumed: 19 ms / 74 ms for the first leg at
  1080p / 4K, and 59 ms / 196 ms for the store leg at the same sizes with
  deliberately incompressible pixels. Both ends of that range are labelled as a
  floor and a ceiling rather than passed off as the real number.
- Where the picture lands follows the caret, so a paste never leaves a stray
  empty line: an empty block *becomes* the image, a block with text gets the
  image below it — the same shape the "/" and "+" doors have.
- Windows-only, like `read_clipboard`: other targets return `None`.
- Evidence: 8 decoder tests on hand-built DIBs (bottom-up flip, row padding,
  zeroed and real alpha, 5-6-5 vs 5-5-5 masks, palette with 2 and with 256
  entries, 1-bit MSB-first unpacking, truncated and RLE payloads refused), one
  state test with a real SQLite library in a scratch folder (block kind,
  attachment name/MIME/size, bytes on disk under `attachments/`, text preserved
  below, one paste = one undo step, undo keeps the bytes), and one `#[ignore]`d
  test that reads the *user's* clipboard — the only proof the FFI sees bytes
  another process wrote; on this machine it printed `clipboard picture: 1280x800
  from 134168 PNG bytes`. A green sweep proves none of this: the slice changes
  zero pixels (`.scratch/sweep15` vs `.scratch/sweep14`: 0 of 44 moved).

## ADR-0033 · An empty page writes its own first block

Decision: the M8 A4 item filed as "the empty state still says the block editor
arrives in a later milestone" was a functional dead end, not stale copy, and it
is closed with a new command. `Command::AppendBlock { kind, text }` puts one
block at the end of the page with no anchor; `AppState::start_page()` calls it
with an empty paragraph and hands back the new id so the controller can focus
it. Two doors use it: the empty panel is now a `TouchArea` (`empty-page-started`),
and committing the page title on a page with zero rows starts the first block
under it. Both are one undo step, and `plan()` refuses a container kind, the
same rule `InsertBlockAfter` carries.

Why: every existing insert command takes an anchor `BlockId` — after a block,
into a table cell, into a column box — and a page with no blocks has nothing to
anchor to, so `create_page` produced a page that could not be typed into at all
(`open_page` seeds nothing either). The alternative fixes were worse: seeding a
paragraph at `create_page` would litter the library with empty blocks on every
cancelled "New page", and doing it at `open_page` would make the empty state
unreachable and break the workspace test that asserts a fresh page projects zero
rows. Making the page produce its own first row on the user's explicit
interrupt keeps the document honest — a page is empty until someone means to
write in it.

Consequences: the empty state's copy is now an instruction ("Click here to
write, or press Enter in the title above.") and the panel must stay a click
target, so it cannot be turned back into a passive placeholder. Chasing that
one string surfaced a second stale promise of the same family in the demo
pages' placeholder paragraph — "changes live in memory for now; SQLite
persistence lands with the next milestone", false since M3 — now rewritten as
what actually happens; it is fixture text in the memory-only session the shot
harness runs, so it moves no scene pixels and no sweep can confirm it.
`AppendBlock` is deliberately
page-scoped and always appends — it is not a general "insert at position" and
does not become one; the 10 000-block page's "+" and Enter paths still go
through the anchored commands. Verified headlessly: 3 command tests (empty page
takes one paragraph and undo empties it again, an append lands below a
container's whole subtree, a grid or a layout is refused), the workspace test
now asserts `start_page()` yields exactly one row with the returned id, and the
click is proven on pixels — `quire-shot --scene empty --click 640,458` renders
the title plus one focused empty row with a caret where the panel used to be.
The sweep's `empty` scene moved 724 sampled pixels, all inside x 506..1032 /
y 456..466, i.e. only the copy line.

## ADR-0034 · Palette ids resolve through a Rust function the tests can walk

Decision: the command palette's integer id space is now decoded by
`palette_action(id) -> PaletteAction` in `src/app/state.rs`, and the
controller's dispatch is a `match` over that enum **with no wildcard arm**. The
`OpenPage(i32)` case carries the page id; an id that is neither a declared
command nor ≥ `CMD_PAGE_BASE` maps to `PaletteAction::None`. Two tests guard it:
one walks `mock_commands()` and asserts every row resolves to its own distinct
action (and no row to `None`), the other pins the boundaries (0, 14, 9 999, −1).

Why: the palette rows arrive in Rust as a bare `i32`, and the dispatch used to
be `match id { 1 => …, 2 => … }` with the named constants written as bare
identifiers. A constant that is not imported silently becomes a *catch-all
binding* in pattern position, so `aaa3763` shipped with every id ≥ 9 running
`copy_current_page_markdown` — no page could be opened from the palette — while
`cargo test` stayed green because nothing walked the dispatch. rustc warned
(unreachable pattern) and the warning was the only correct signal in the
pipeline. The repair ROADMAP.md:81 named as "still not written" is a test over
the registry, and a test can only exist if the mapping is a function: an enum
arm that cannot be shadowed by a missing `use`, and a match the compiler forces
to stay exhaustive when a variant is added.

Consequences: adding a palette command is now three edits in one file (a
constant, a `PaletteAction` variant, an arm) plus the action's body in the
controller, and forgetting the middle one is a compile error rather than a
silent hijack. The id constants stay in `state.rs` because that is where
`mock_commands` builds them. This closes the class for the palette only — any
other callback that crosses the Slint boundary as a bare `i32` has the same
exposure, so a future one should ride an enum the same way. Proven by the
registry test rather than by pixels: this fix changes 7 sampled pixels in the
`palette` scene (the sidebar hint now reads `Ctrl+\`), which is exactly the
evidence that a green visual sweep says nothing about behaviour.

## ADR-0032 · A layout is two levels of ordinary blocks, and it draws itself inside its own row

Decision: `columns` (SPEC §三十七 批次 B, M10's fifth slice) stores a layout as
**two levels of child blocks** and adds **no schema**. The `columns` block carries
its box count in the very same `blocks.columns` column v8 introduced for a grid;
each box is a `BlockKind::Column` child of it; each line inside a box is an
ordinary child of that box. One `Change` still carries the shape
(`BlockColumnsSet`) and every other edit is `BlockInserted` / `BlockDeleted` /
`BlockMoved` / `BlockTextSet` / `BlockKindSet`, so undo, redo and §十八 keep
working without knowing what a layout is. The UI projects the whole layout into
the layout's **single** delegate as a flat `[ColumnItem]` plus a `[ColumnBox]`
shape table (`id`, `column`, `first`, `size`) that slices it, and
`visible_block_indices` hides both the boxes and their lines — the same filter
ADR-0028 uses for a folded subtree and ADR-0031 for cells, so a layout is one
row however wide it is. Tiling is Slint 1.18's `FlexboxLayout` (one row,
`flex-wrap: no-wrap`, `alignment: stretch`, `horizontal-stretch: 1` per box), not
hand-written widths. The hover strip reuses `TableEdge` for Add / Delete column,
and `plan()` refuses below two and above three boxes.

Why: Slint has no recursive components, so a box cannot hold an `EditorBlock`
that itself holds boxes — the only place a layout's content can be drawn is the
layout's own row delegate, which is precisely what §三十七's "分栏只在可见窗口内
展开" asks for and what §十二's virtualization premise requires. Reusing v8's
integer is the same argument ADR-0030 made about v7: a box count and a column
count are one fact ("how many boxes does this container hold"), and a schema
version for a rename is not worth a migration. A box always holds at least one
line, and `ColumnsAddBlock` exists because an empty box is the one thing on the
page a click cannot put a caret in: the empty box says so, and the click asks for
its first paragraph.

Consequences: a box has no row of its own, so nothing outside the delegate can
address its lines by row index — Tab walks them through `column-item-move` and
stops dead at either end, ↑/↓ move the caret inside the line rather than leaving
the layout, and leaving a box is a click. Markdown export flattens: the
containers write no marker of their own and their lines come out at page depth in
reading order, which a test pins in both directions (re-import keeps every word
and loses only the shape), and the importer learns no columns syntax — the same
degradation ADR-0031 accepted for a grid. `InsertBlockAfter` had to stop treating
a container's row as its own slot: it now inserts after the container's whole
subtree and inherits the anchor's parent, which closes the identical hole for a
grid — "+" on a table used to be able to wedge a top-level block between the
table and its first cell, and a page's blocks are one flat slice sorted by
`order`, so a container whose subtree is not contiguous stops being one
container: `subtree()` walks off it, and every index-based reader with it. Two QOML findings came out of this slice and both
belong in UI_ARCHITECTURE.md: `for … : if … : Component` is a parse error, so a
filtered repetition has to become a shape table the delegate slices
(`ColumnBox.first` / `size`), and an identifier on the right-hand side of a
property inside a component instantiation resolves in the **enclosing**
component, so a child cannot be handed its own parent's `root.x` — the layout
passes `line-y`, `line-x` and `each-width` by plain names. The pointer-claim
protocol ADR-0031 needed for a grid crosses a component boundary here as
`in property pointer` plus `callback pointer-claimed(int)`, because N two-way
bindings onto one parent property is not something Slint lets you write. And the
"/" popup anchored itself with a hardcoded 350 px window reserve, which this
slice is the first to outgrow — the anchor now measures the real list height the
way the "+" menu already did. Deliberately not built: per-box width control
(Notion's resize handle), more than three boxes, and a layout nested inside a
box.

## ADR-0031 · A table is a block that owns its cells, and the grid is one editor row

Decision: `table` (SPEC §三十七 批次 B, M10's fourth slice) stores the grid as
**child blocks**, not as a payload. `BlockKind::Table` gains one stored number —
`blocks.columns INTEGER NOT NULL DEFAULT 0`, schema **v8** — and each cell is a
`BlockKind::TableCell` child of it, kept in row-major order by its ordinary
`order` key. Row count is therefore *derived* (`cells / columns`), never stored,
so adding a row is inserting `columns` blocks and nothing else moves. One new
`Change` carries the shape (`BlockColumnsSet`); every other table edit is plain
`BlockInserted` / `BlockDeleted` / `BlockTextSet` / `BlockKindSet`, which means
undo, redo and the §十八 storage contract all keep working without knowing
what a table is. The UI projects the whole grid
into the table's single delegate as a flat `[TableCell]` + `columns`, and
`visible_block_indices` filters cells out the same way ADR-0028 filters a folded
subtree — a table is one row, always, however big it gets.
Why: the alternatives are a JSON blob column (a cell would stop being a block,
so inline marks, undo granularity and §三十九's later relation/rollup work all
have to be reinvented outside the model) or a real table-per-row schema (§三十九
owns that, and SPEC is explicit that this kind is *not* a database view). Cells
as blocks cost one integer column and reuse everything. The two rules ADR-0028
added for dynamically-rowed kinds apply here and are honoured: the projection
really drops the hidden rows, and every row-index consumer goes through
`visible_block_indices`. A grid whose cell count is not a multiple of `columns`
is refused by `grid()` rather than repaired — `plan()` says no instead of
guessing which row is short, and `DuplicateBlock` will not copy such a table at
all, because a cell-less grid opens broken.
Consequences: cells never appear in the row list, so no code that reads
`rows[i]` can be handed a cell — `SetBlockType` refuses a cell as both source
and target, and a cell's kind belongs to its grid, not to the Turn-into menu.
Turning a block *into* a table moves its whole line into the top-left cell of a
fresh 3×2 grid rather than dropping it, because a table block draws no text of
its own; flattening it back turns each cell into a paragraph, so the words
survive both directions — the *marks* do not, since `new_cell` starts clean and
a mark range pointing at the old block's text would mean nothing inside one
cell. Adding a column is the one command that must insert
`rows` blocks into `rows` different gaps at once, which is why
`OrderKey::STRIDE` is 1<<16 and why `renumber_page` now spreads with it: one
midpoint per insert halves a gap, so a batch has to be keyed whole. The
projection reads a table's cells by scanning the page, so it is guarded on
`kind == Table` — unguarded it ran that scan for all 10 000 rows of a bench page
and allocated an empty model with each, which measured ≈2 MB of working set on
D and nothing on the 24-row A (`docs/PERFORMANCE.md`). Markdown
export writes GitHub-flavoured tables (first row is the header, `|` escapes to
`\|`, a newline in a cell becomes a space); **import does not read them** — a
`|a|b|` line stays a paragraph, pinned by a test in `markdown_test.rs`, because
the importer's shape is line-at-a-time and a table needs lookahead. Deliberately
not built: Enter splits a cell, backspace merges, Up/Down cross rows, Ctrl+L
marks a link in a cell, and a cell's text width does not feed back into column
widths.
Hover-only UI needed a new shot capability: `quire-shot --hover x,y` dispatches
`PointerMoved` without a button, which is the only headless way to light the
edge toolbar; `table` and `table-edit` are in the sweep but the 22 px strip they
show is verified by a manual `--hover` shot, and hovering a table reflows the
content below it by exactly that much. Two Slint 1.18 geometry findings came out
of this slice and both belong in UI_ARCHITECTURE.md: an element in a non-layout
parent that binds `height` but leaves `y` unbound is rendered **centred** in that
parent, so `DocumentRow.head` had been sitting 34 px below its binding in every
scene ever shipped and the table — the first tall *first* block — finally painted
over the page title; the fix is explicit `x: 0px; y: 0px;`. And `absolute-position`
readings inside a ListView delegate are not trustworthy (row 1 reported its head
below its own body with both at `y: 0`), so the delegate's real geometry is
`parent.y + head.height`, which is what `row-y` already passes down. Verified
headlessly: `cargo test --features software` green, and the 37-of-42 changed
scenes in the new baseline all differ **only** inside the title band, which is
the fix above and not the table; the projection guard was re-swept afterwards
and came back 42-of-42 byte-identical, so it is a memory change and not a
visual one.

## ADR-0030 · A file attachment is stored without ever being looked at

Decision: `file` (SPEC §三十七 批次 A, M10's third slice) adds **no schema**. A
file is a block kind, and the v7 columns ADR-0029 introduced for pictures —
the `attachments` row plus `blocks.attachment` — describe it exactly as well,
so the slice ships with `user_version` unchanged. What is new is that the bytes
never enter the process: `AttachmentStore::import_any_file` `fs::copy`s the
picked file straight to `<library>/attachments/<id>.<ext>` and records its
length, with no decode, no `image` call and deliberately **no size cap** — a
2 GB attachment is stored in 2 GB of kernel-side copying and 0 of working set.
The row paints three things from the database line alone: the name, a
`format_size` label, and two buttons. Open goes to `platform::open_with_default`
(a hand-declared `ShellExecuteW`); Save-as goes to `rfd` + `export_to`, which
copies the stored bytes out under `save_name`. Undo drops the reference and
never the bytes, unchanged from ADR-0029.
Why: the priority list in ROADMAP.md puts low RAM above feature count, and the
§二十二 promise is the one this kind would break first — the obvious
implementation, read the file into a `Vec` and write it back, is a 2 GB working
set for a note. The picture path solves the same problem by downsampling; the
file path solves it by never looking. `ShellExecuteW` rather than
`spawn("explorer.exe", path)` because explorer returns 0x1 **on success** by
design, so a subprocess can never tell a launched app from a blocked file
association, while the FFI answers `> 32` only for a real launch — and one
extern declaration keeps the `windows` crate out, the same rule ADR-0025
follows for the clipboard.
Consequences: the label keeps its extension while a picture's drops it, because
for a picture the content is the truth and for a `.zip` the name is; that split
is why `save_name` exists (it rejoins stem + extension for pictures, and leaves
a file name alone — case-insensitively, since `file` lowercases the extension
and `name` keeps the user's). Open is an explicit button rather than
click-the-row, because a row that launches an arbitrary executable on a stray
click is not a row you can safely select text in; the two buttons stay drawn
rather than hover-revealed, since a file block that shows nothing to press
reads as a broken attachment. The size is a `pure callback attachment-size(int)`
rather than a `BlockRow` field so the string is not rebuilt for every row on
every keystroke — the same viewport-not-document argument ADR-0029 makes for
the rasters. Markdown export writes `[name](quire://attachment/<id>)`, a
*link* rather than the picture's `![](...)`, and that is the one place the two
kinds differ in the exporter: the importer has no picture shape but it does have
a link shape, so a file's reference survives an export/import round trip and a
picture's does not. Still open here when this was written: orphaned bytes (no
FK, no cascade, same gap as pictures — ADR-0037 gives it one reclamation
path), clipboard-bitmap paste (ADR-0035), and the PDF first-page thumbnail,
which the user deferred on 2026-09-20 — until it lands a PDF and a `.zip` look
identical apart from their names. Verified headlessly, not by eye: 36 of 40
scenes byte-identical against the previous sweep, `slash`/`plus`/`dark-slash`
moved only because the menus gained the File row, and `file` is new — a
760×48 px box at x 390..1149 with the label left, "1.8 MiB" right-aligned, and
two 26 px buttons at x 1086..1112 / 1116..1142. The fixture's first draft put
`std::process::id()` in the file name, which made the label different every
run; the id moved to the temp *folder* so the scene is reproducible.

## ADR-0029 · Pictures are files beside the database, and the editor never loads the original raster

Decision: `image` (SPEC §三十七 批次 A, M10's second slice) stores its bytes in
`<library>/attachments/<id>.<ext>` and keeps only a reference in SQLite — a new
`attachments` table plus `blocks.attachment INTEGER` (schema v7). The column
carries **no foreign key on purpose**: a block whose file row has vanished must
still load and render as a missing picture, because a user who copies the `.db`
without its folder owns a library, not a crash. Import downsamples anything
longer than `MAX_EDGE = 1280` px into a second file, `<id>.cache.png`, and
`AttachmentStore::display_path` hands the UI only that copy — the original
stays byte-identical on disk, so viewing never degrades the user's file. The
decoded rasters live in a cache owned by `AppState`, keyed by attachment id and
capped at 32 MiB weighted by RGBA bytes, evicting the least-recently-realized
entry first. Rows reach it through the `image-for` / `image-aspect` **callbacks**
rather than model fields, because Slint invokes a binding only for the rows it
realizes. Undo drops the reference and never the bytes: `Change::AttachmentAdded`
is an upsert and there is deliberately no `AttachmentDeleted`.
Why: Slint's own decode cache is a thread-local `CLruCache` capped at 5 MiB
weighted by decoded bytes and keyed by path + mtime (`i-slint-core-1.18.0`,
`graphics/image/cache.rs`). One 1280×720 RGBA frame is 3.7 MiB, so that budget
is a single photograph — scrolling a page of pictures through it re-decodes on
every frame. The obvious alternative, an uncapped cache of our own, is how a
low-RAM app dies quietly, and §二十二's promise sits on the first slot of the
priority list in ROADMAP.md. Callbacks are what keep the cost proportional to
the viewport instead of to the document: a page with five hundred pictures
holds about ten.
Consequences: the 32 MiB ceiling is ≈eight full-width frames, and it is a
construction bound, tested as one (`the_picture_cache_spends_its_budget_and_
drops_the_stalest_first` also pins LRU over FIFO by re-showing an evicted id
mid-scroll). A failed decode caches as a zero-weight blank so a missing file is
not re-opened per repaint — which also means replacing a file on disk in place
needs a restart to show up, since our key is the id and not the mtime. The
extension and MIME come from sniffing the bytes, so a screenshot saved as
`.jpg` but encoded PNG is stored as the PNG it is. `blocks.img_percent`
(25 / 50 / 100, default 100) is the width tier; setting it back to the default
plans no change, so the menu cannot leave an undo step that does nothing.
Markdown export writes `![name](quire://attachment/<id>)`; the importer has no
picture shape, so the line survives as literal text and the file name is never
lost — the reference goes out, nothing comes back, which is the same asymmetry
§三十七 accepted for toggle folds (ADR-0030 gives `file` the link form, so that
one does round-trip). Of what this kind left open, the clipboard-bitmap paste
has since landed (ADR-0035); the PDF first-page thumbnail reuses this store
unchanged and is what the user deferred on 2026-09-20.

## ADR-0028 · A folded subtree gets zero realized rows, so row indexes stop being model indexes

Decision: `toggle` (SPEC §三十七, M10 批次 B's first slice) persists its fold
as a `blocks.folded` column (schema v6) and hides its subtree by **filtering
it out of the projection** — `project_blocks` builds rows through
`visible_block_indices`, so a hidden block has no `BlockRow`, no delegate and
no height. There is no `visible: false` row. Because that makes the editor's
row numbering independent of the document's, every consumer that treats a row
index as a model index must translate first; the one that exists today (the
§八 drag landing) goes through the new `AppState::drop_index_for_row`, which
shares `visible_block_indices` with the projection.
Why: Slint's `ListView` realizes exactly one `for` child per item, so a
0-height delegate would still cost a component, a binding and a slotmap
lookup per hidden block — on a page whose top section is folded that is the
whole point of folding, paid for and thrown away. Deleting the row is also
what makes the behaviour correct by construction: a hidden block cannot be
tabbed into, dragged, found-by-⌘F or renumbered if it is not in the model.
The cost is that two numbering systems now coexist, and the repo had been
using them interchangeably.
Consequences: fold is undoable view state, not content — `Command::ToggleFold`
emits `Change::BlockFoldedSet`, the same shape `PageExpandedSet` already uses
for the sidebar, so it rides the persistence queue and never bumps a document.
Only a Toggle draws the chevron, so `SetBlockType` away from Toggle clears the
fold (undo restores it) rather than stranding a subtree with no way back;
`SplitBlock` and `DuplicateBlock` force `folded: false`, because neither one
copies a subtree and a fold over nothing is a trap. Markdown export degrades a
toggle to a quote line — CommonMark has no fold syntax, so it degrades exactly
like a callout already did, while the subtree still rides along indented and
import never restores the fold (§三十七 asks for precisely that asymmetry).
And SPEC §三十七's 硬性约束 gained a rule for the next dynamic-row kind
(`table`, `columns`): the projection must really delete rows, and new
row-index consumers must translate. Verified headlessly, not by eye: 32 of 35
baseline scenes are byte-identical after the change, `slash`/`plus`/
`dark-slash` moved only because the menus gained the Toggle row, and
`toggle` vs `toggle-fold` differ by 6 006 px in exactly four places — the
triangle glyph, the vanished child line, the one-row reflow below it, and a
2 px taller scrollbar thumb.

## ADR-0027 · The parked feature set is unscheduled, not cancelled; only six capabilities stay out

Decision: of everything the SPEC had deferred, only sync, real-time
collaboration, cloud, plugin market, AI, publish-to-site and
comments/discussions/reactions remain out of scope (SPEC §三十三, extended
2026-09-20 with the last two). Everything else that §九 and §十七 had
written as "后续再加" is now a scheduled phase with a milestone: §三十七 →
M10/M11 (image, file, PDF, table, toggle, columns, then highlighting,
bookmark, embed, math, TOC), §三十八 → M12 (icon, cover, page font /
full-width / small-text, lock, version history, templates), §四十 → M13
(@-mention, backlinks, synced block), §三十九 → M14 (Database). Ordering
follows "how fast a daily note hits the wall", not Notion's feature
alphabet: media and structure blocks before the database, and the database
only after the reference layer exists, because `relation` and the simple
table would otherwise each get their own ad-hoc version of it.
Why: an audit of SPEC.md against Notion found three different things being
reported as one — items genuinely ruled out (§三十三), items deliberately
deferred with a spec line (§九), and items never written down at all
(page-level properties, backlinks, templates, version history, all views
and property types of the database layer, i.e. the largest single gap).
The third group was the problem: §三十五's "feature count ranks last"
had been read as licence to leave them unwritten, so they could not be
planned, estimated or refused. Recording them costs nothing now and makes
"第一期不做" an explicit statement per item instead of a blanket.
Consequences: §三十五's priority order still binds each of these phases,
so every one carries a measured gate rather than a checkbox — M10 must
keep the 10 000-block scene inside 1.2× the current RAM baseline (image
downsampling is where a low-RAM app normally dies), M14 must do filter and
sort in SQL and never realize 10 000 rows (SPEC §三十九's red lines), and
math/highlighting/embed may not smuggle in a JS, WASM or WebView runtime
that §二 forbids. `person` degrades to a local name list because there is
no account model to point at. New sections were appended as §三十七–§四十
rather than inserted in phase order: existing cross-references (ROADMAP
cites §六/§十六/§三十五, ADR-0026 cites §八) are all by section number, and
renumbering would have silently broken every one of them.

## ADR-0026 · Page and Link-to-page blocks share blocks.page_ref; ownership is a kind-level contract
Decision: both page-bearing block kinds — `Page` (kind 11) and `Link`
(kind 12) — point at a page through the SAME nullable `blocks.page_ref`
column (schema v5) and the same `BlockRefSet` change; no per-kind column or
variant. What differs is **ownership**, decided by the kind:
- `Page` **owns** its child page (created in the same batch as the block).
  Deleting the block deletes the child; duplicating the block deep-copies
  the child and retargets the copy; pasting one lands the title as plain
  text (two blocks must never share an owned page). One `BlockRefSet`
  without a live `PageCreated` is still legal — storage stores the pointer,
  the lifecycle is the caller's composition.
- `Link` **references** an existing page picked in the page picker. Delete,
  duplicate and paste never touch the target; sharing a reference is the
  point. Turning either kind into another kind via the ⋮⋮ menu drops the
  reference (`BlockRefSet` → `None`); a Page's child survives in the tree,
  unowned.
Why: Notion's Page block and Link-to-page differ exactly in lifecycle, not
in data shape — one column keeps schema and export uniform (`[title]
(quire://page/<id>)` for both, the ownership difference deliberately does
not survive Markdown), and the contract stays at one change variant
instead of two. The page picker reuses the slash popup in a
`slash-pick-page` mode rather than a fourth popup, so anchor/keyboard/
close behavior has one implementation.
Consequences: a dangling `page_ref` (child deleted from the sidebar) renders
"(deleted page)" muted, and the block still opens nothing — no cascade from
block to tree outside the explicit delete path. A duplicated page's own
embedded Page blocks still reference the ORIGINAL children (recursive copy
is v2). Keyboard focus-move can land on a page row; the row is
never-editable by kind, so the input simply does not appear.

## ADR-0025 · Clipboard reads are direct Win32 FFI — no clipboard crate, no subprocess
Decision: `platform::read_clipboard` opens the clipboard through
`OpenClipboard` / `GetClipboardData(CF_UNICODETEXT)` / `GlobalLock` declared
in-module (`user32`/`kernel32`, already linked for winit) — writes stay on
`clip.exe` stdin.
Why: rich paste (§二十七) needs the text *any* source app put on the
clipboard. The zero-dependency alternatives both fail: `clip.exe` is
write-only, and a `Get-Clipboard` subprocess measured **7–10 s** on the dev
desktop (PowerShell startup under AV) — no paste can wait for that. A
clipboard crate (arboard) would be the first new runtime dependency since
M3 to buy ~40 lines of FFI the toolchain already links. CF_UNICODETEXT is
the format every text source supplies; UTF-16 → `String` via
`from_utf16_lossy`, and a 5×2 ms retry rides out transient clipboard locks
held by other processes. Writes keep `clip.exe` because the only payloads
the app writes (`quire://` links, ADR-0023) are ASCII, so the
OEM-codepage stdin caveat is moot.
Consequences: reads are microseconds and CJK-safe from any source app;
writing non-ASCII *from the app* would garble through `clip.exe` — the day
a feature needs that ("copy page as markdown"), switch the write path to
the same FFI (`SetClipboardData`) rather than adding a crate.

Update (2026-09-20, same day): the day came with "Copy Page as Markdown" —
the write path now runs through `SetClipboardData`/`GMEM_MOVEABLE` FFI too
(`copy_to_clipboard` swapped its clip.exe internals for the FFI; callers
unchanged, no crate). `clip.exe` is fully retired from the codebase; the
round-trip test (`clipboard_write_and_read_round_trip_unicode`) pins CJK
through write → read.

## ADR-0024 · The release profile stays as shipped — no fat LTO, no panic = abort
Decision: `[profile.release]` keeps thin LTO + `codegen-units = 1` +
`strip = "debuginfo"`. Fat LTO is rejected, `panic = "abort"` is rejected,
`codegen-units = 16` is rejected.
Why: SPEC §二十四 only keeps levers that win on runtime memory / CPU /
startup — exe size ranks last. The 2026-09-20 four-way comparison
(`docs/PERFORMANCE.md` "M8 · release profile audit", Track B A3) measured
idle private bytes 88–90 MB, a 10 000-block page at 97–99 MB, idle CPU
0–0.6 %, typing 24–30 % and warm window-up 85–192 ms across all four
profiles — one noise band. Fat LTO saves 2.5 MB of exe for a ×2.5 build
time; `codegen-units = 16` *adds* 2 MB; `panic = "abort"` is the only
"smaller and faster" option and is vetoed on behavior, not numbers: abort
does not unwind, so `logging::install`'s panic hook never runs,
`panic-report.txt` never lands and `last_session_aborted` is blind exactly
when it matters — the ADR-0018 / SPEC §二十五 crash-recovery chain dies
with it. `strip = "debuginfo"` is the symbol policy: linker debuginfo
stripped, the COFF symbol table kept; "debug artifacts separated" is
`just dist` preserving `target/release` originals, not symbol stripping.
Consequences: none at runtime — this ADR mainly pins what must NOT change.
The conclusion expires when a lever moves idle private bytes or typing CPU
beyond ≈2 MB / ≈3 pp of the measured noise floor; re-run
`benchmarks/scripts/profile_bench.ps1` per profile before believing any
single-run delta (the audit caught a parallel-build-polluted batch that
looked like a 20 % win and was not).

## ADR-0023 · The ⋮⋮ menu gets Notion's remaining items; block color crosses the persistence contract
Decision: the block handle menu carries Copy link to block, Move to, and
Text/Background color (Comment / Suggest edits / Ask AI stay out with the
rest of collab+AI). "Copy link to block" puts `quire://block/<id>` on the
system clipboard via `clip.exe` (ASCII-only payload, so no clipboard crate
— dependency policy holds); `open-link` now resolves `quire://block/` and
`quire://page/` in-app first (jump to the page, focus the block) and only
shells out for foreign URLs, which also makes Ctrl+L links to internal
anchors clickable. "Move to" lists every other page depth-indented and
commits `Command::MoveBlockToPage`: the root lands appended to the target
page's top level, the subtree travels as one `Change::BlockMovedToPage`
per block (children keep their parent pointers and orders), one undo step
end to end. Color is a block-level pair (`ColorKind` text + background,
10 slots, Default = theme), one `SetBlockColor` command, one
`BlockColorSet` change, and schema v4 (two TEXT columns on `blocks`,
'' = default; the ALTER runs conditionally in code because SQLite has no
`ADD COLUMN IF NOT EXISTS` and the migration test legitimately re-runs v4
on a hand-downgraded file). A Callout block joins the kind set — the last
Basic kind that neither a symbol shortcut nor the v1 exclusions cover;
it renders a tinted rounded box with an emoji and exports as a quote.
Why: the user asked for the ⋮⋮ menu to match Notion's, minus what typing
symbols already reaches (ADR-0022 curation unchanged and now spanning the
new kinds).
Consequences: colors are cosmetic by design — no Markdown representation,
so export/import round trips drop them (callouts degrade to quotes);
`clip.exe` means copy-link is a Windows-only nicety until a clipboard
crate earns its place; menu popups size their anchor clamp from the live
row count, so the tall root menu and the 11-row color palettes stay
on-screen; the "current color" check is drawn as an icon because the
software renderer has no font fallback (a ✓ glyph rendered as nothing in
headless captures).

Amended 2026-09-21 · the ten slots and `text-muted` are now measured, not
approximated. WCAG relative luminance over the four light surfaces said the
third text tier sat at 2.51–2.71:1 and the orange and yellow slots at 2.81
and 2.48 on their own tints — the two weakest pairs in the palette, and the
reason the A4 sweep's "dimmest text in a light UI" finding was real. Light
`text-muted` is `#75787d` (4.43 on white, 4.10 on the sidebar), slot 3 is
`#bd6408` (3.61 on its tint, 4.21 on plain), slot 4 is `#a87718` (3.56,
3.95). Dark did not move: its muted tier had always read 3.9–4.4, which is
now the band both themes agree on. Two consequences worth stating: the
hierarchy is capped by that band, not by AA — pushing muted past ~`#71747a`
closes the gap to `text-secondary` (5.91) and there is then nothing left
that reads as a whisper — and the same arithmetic retracted the finding it
was written to check, because the pairs the sweep named by eye as the
weakest (green-on-olive, red-on-black) measure 3.86 and 4.72 and were never
the problem.

## ADR-0022 · Markdown line-shortcuts; the menus list only what symbols can't reach
Decision: typing a trigger at the block start converts the block live —
`# `/`## `/`### ` → Heading 1/2/3, `- `/`* ` → Bullet, `12. ` → Numbered,
`[] `/`[ ] ` → To-do, `[x] ` → checked To-do, `> ` → Quote, `---` →
Divider, ` ``` ` → Code — as one undo step (`exec_all`, which now skips
no-op parts of a compound command instead of failing wholesale). The slash
menu and the ⋮⋮ "Turn into" list carry only Text/Code/Divider: every other
kind has a symbol path. Notion's Comment / Suggest edits / Ask AI / Color /
Copy-link / Move-to stay out (collab+AI are v1 out-of-scope; no block
anchors or cross-page moves yet).
Why: the user wants the pickers short — entries reachable by blind typing
are noise; the symbol is the only path to the removed kinds, which keeps
the menus honest.
Consequences: code/divider blocks are exempt from conversion (their text
legitimately starts with these characters); converting back among symbol
kinds works by typing the other symbol at the block start. Fixed while
wiring: the ⋮⋮ popup was driven by a `block-menu-open` bool only the bench
scene ever set — a real grip click raised just `block-menu-open-id`, so the
menu never appeared and had no coordinates. The popup now follows the id
and the bool is gone; menu geometry anchors beside the handle from
delegate-reported viewport coordinates (DocumentRow layout-y + handle
offset), which also fixes the slash menu anchoring every block at the first
row's offset.

## ADR-0021 · Drag-reorder rides Slint's built-in `DragArea`/`DropArea`
Decision: the grip handle wraps its TouchArea in a 1.18 `DragArea`
(`allow-move`, payload = plain-text `slint-notion/block:<id>` built in the
controller); every editor row is a `DropArea` whose `can-drop` validates the
landing through a read-only `can_move_block_to` check and parks the accent
drop line in `UIState.drop-line-row`; the drop commits one new
`Command::MoveBlockTo` (insert-above flat index), so one undo step and one
persistence entry cover a multi-row move.
Why: SPEC §1564 says to prefer Slint's built-ins over hand-rolling; the
in-window drag path lives in core (press-filter → threshold → DragMove/Drop
routing), so it works with every renderer on Windows without OS DnD, and a
click below the threshold still reaches the menu TouchArea. Reordering on
drop (not per-hover swap) sidesteps the ListView delegate-reuse trap, where
mutating the model mid-drag would swap the data under the dragging delegate.
Consequences: the landing must not split a subtree (top-level inserts may
only sit above another top-level block; nested items stay adjacent to their
sibling run) and nested-swap `MoveBlock` now re-sets the same parent instead
of flattening children to top level (pre-existing bug found while planning).
Dragging across a virtualized viewport does not auto-scroll the ListView
yet; revisit when long-document drag matters.

## ADR-0020 · The library moves to `%APPDATA%\Quire`; `--db` and `--portable`
stay
Decision: `storage::data_location` is the single answer to "where is the
library". Opening the pre-D12 default — `appdata/quire.db`, relative to the
working directory — is redirected to `%APPDATA%\Quire\quire.db`, and the first
such redirect carries the old library across whole: the main file, the
`.bak<N>` family, the `-wal`/`-shm`/`.corrupt` sidecars a crashed session left,
and the `quire.log` family that shares the folder. A caller that names a file
of its own (including `--db <path>`) is taken at its word and never rerouted;
`--portable` asks for the old behavior out loud, which is what a stick install
wants and what the benchmark harness uses to keep out of real notes. Each file
moves as copy → open-the-copy → one rename → delete the source, staged through
a `<destination>.migrating-<pid>` name. The idempotence key is the destination:
once `%APPDATA%\Quire\quire.db` exists there is nothing to migrate, so a restart
— or a move interrupted after the first file — cannot overwrite a library that
has since been edited.
Why: with the installer in place (ADR-0017) the working directory is whatever
the shortcut's "Start in" happens to say, so launching Quire from two folders
creates — and seeds — two unrelated libraries, and the user finds out when a
note is missing. That same relative path is also what forces the per-user
install; once the data has its own home the premise behind ADR-0017's
"the install folder must be writable" disappears. The copy is opened before the
source is retired because a `fs::copy` of a database another session is writing
can be torn, and a damaged library discovered at the new path has no source left
to fall back to; refusing the move keeps the old folder — snapshots included —
exactly where `backup::open_with_recovery` expects it. Falling back to the
legacy path on any error rather than opening the empty per-user file is the
difference between "it did not move" and "my notes vanished".
Consequences: the log follows the database, since `services::logging::data_dir()`
now asks storage — a `--db` run writes its log beside that file, which also
keeps the benchmark runs out of the checkout. Three things this track could not
finish: `main.rs` still creates `appdata/` beside the working directory before
opening it, `LaunchArgs` has no `portable` field so the two flags are re-read
from the command line here (`scan`), and `OpenReport` has no field to say a move
happened, so the user only ever sees one stderr line. All three are in
`M8_FEEDBACK.md` #13 with the lines to write. `%APPDATA%` roams with the
domain profile, which is what the milestone asked for and is fine at this
database size; if a roaming profile ever turns out to be slow for a live SQLite
file, `roaming_root()` is the one function to change.

## ADR-0019 · Backup retention is two windows; the periodic snapshot rides the
flush tick
Decision: `storage::backup` keeps five generations (ADR-0015 had three) *and*
drops any generation whose modified time is older than `MAX_AGE` = 7 days. Both
are applied by one `prune(path, now)` that `snapshot()` calls after a successful
copy and whose return value is discarded: cleanup is housekeeping, and a locked
or vanished old file must not turn a good snapshot into a reported failure.
`prune` scans `KEEP + 2` slots so a family left behind by a larger `KEEP` cannot
survive unboundedly (`recover` only walks `1..=KEEP`, so anything past it is
unreadable weight). Mid-session insurance is `PersistenceService`'s: a snapshot
hook attached with `with_snapshotter(interval_ms, hook)` — or the app's one-liner
`with_database_snapshots(&repo)` — runs from the flush path that is already
ticking (`flush_if_due`, `force_flush`), gated on two facts: the period
(default `DEFAULT_SNAPSHOT_INTERVAL_MS`, ten minutes) elapsed since the last
snapshot, *and* something was written since then. `SqliteRepository` now records
the path it opened and exposes `snapshot()`, so the hook is a method call on the
`Arc` the app already holds; a failure is stored for the UI to take
(`take_snapshot_error`) and logged, never returned as the flush's error.
Why: the milestone forbids a second resident thread, and the app already arms a
timer per recorded burst — a snapshot that rides that tick costs no new
scheduling and, because of the `pending` gate, no work at all in a session that
changed nothing. Count-only retention keeps a five-week-old `.bak5` alive for a
user who opens Quire once a week; age-only keeps five copies of a scene-D
workspace forever, which is the memory ADR-0015 accepted at three generations
and should not silently triple. Age comes from the file's own `modified()`
because the snapshot's whole life is that one write, and `now` is a parameter for
the same reason the debounce window takes a clock: a test ages a file with
`File::set_times` rather than waiting a week. Gating on the write rather than
running on the wall clock is what keeps a 2.3 MB `VACUUM INTO` from repeating
while the user reads.
Consequences: ADR-0015's "deliberate omission" (a long session had no insurance
until the next open) is closed — the loss window is now "since the last tick",
but only for a session that keeps editing, because the app's timer is
single-shot: an idle window takes no snapshots, and ten minutes is a minimum gap
rather than a cadence. Making it exact is an app-side `TimerMode::Repeated`
(M8_FEEDBACK #12 records the wiring line and this reading). A monthly user can
end up holding one generation instead of five — age wins, which is the point of
the second window. `services/persistence.rs` now names `SqliteRepository`
concretely, the same trade ADR-0014 made for `search`, and `snapshot()` takes the
connection mutex, so a snapshot serialises against writes by construction rather
than by a new lock. Retention covers `.bak<N>` only: the `.corrupt` corpse and
D9's log family still have their own rules.

## ADR-0018 · Logging is one rotating file plus a panic report the next start
reads back
Decision: `services::logging` owns the app's only log file, `quire.log` beside
the database, with a family of three (`quire.log`, `.1`, `.2`) and a 1 MB ceiling
per file — size is checked before each append, so a line may overshoot by its own
length and nothing else does. Records are one physical line: `2026-09-19T08:21:55
.169Z [info] …`, UTC, with newlines in a message escaped to the two-character
`\n`. `init()` is the single line `main` calls; it creates the directory, writes
the startup record, and installs a panic hook that chains whatever hook was
already there. On panic the hook appends a `[panic]` line and writes
`panic-report.txt`; the *next* `start()` reads that file, records it as the
`last_session_aborted` entry of `session.meta`, logs the same fact, and deletes
the report so a restart cannot blame the old crash twice. `session.meta` is a
line-oriented key/value file — the file-backed twin of the `metadata` table — and
`meta_entries()` hands it to whoever holds a repository.
Why: the alternatives all cost more than a crash trace is worth. A `log`/`tracing`
facade would add dependencies to a four-crate tree to gain levels nobody tunes,
and an async writer would need the thread this milestone is explicitly avoiding.
Writing the abort *metadata* on the next launch rather than inside the panic is
the same argument in miniature: a panicking process may die at any moment, so it
gets two plain writes while the read-modify-write of a metadata file runs on a
thread that is known to be healthy. UTC rather than local time because a log
timestamp and a file's modified time must be comparable when the two disagree
about nothing else. The one line in `main.rs` is the ceiling the milestone allows,
which is also why the logger cannot reach the database: it starts before the
repository exists, and `src/app/**` is off-limits (M8_FEEDBACK #9 records the
wiring Track A still owes).
Consequences: a hard crash (access violation, kill, power loss) leaves no report,
so `last_session_aborted` means "a Rust panic", not "the session did not end
cleanly" — closing that gap needs an exit-path call in `main.rs` (one more line)
or a clean-shutdown hook in the app layer. Logs currently land in the working
directory's `appdata/` until D12 moves them into `%APPDATA%\Quire\` with the
database, which also means a portable run keeps its log beside its own database.
The `Logger` is constructed per directory and every write opens the file, so
tests rotate against a 200-byte limit instead of a megabyte.

Update (2026-09-20, D13): the exit-path call landed — `main` notes
`END_RECORD` ("session ended") after the final flush, and `start()` reads its
absence, with no panic report, as an unclean end. The gap this ADR originally
recorded is closed: `last_session_aborted` now means "the session did not end
cleanly", with panic summaries still verbatim ("panicked at …") and the new
shape worded apart ("did not shut down cleanly (killed, crashed natively, or
lost power)"). The tail check follows the rotation family, so an end-record
that shifted into `.1` still reads as clean, and an empty family is a first
run, not a kill. Two residual false-positive sources are accepted: a second
instance reading a live session's tail (no single-instance guard), and any
external kill of a bench/scene run — both *were* unclean ends, the banner
just cannot say who did the killing. The app consumes the session.meta entry
on the next start (M8_FEEDBACK #9 wiring): it lands in the `metadata` table
and in the notice bar, then is deleted so it cannot re-report forever.

## ADR-0017 · Shell identity is a build-time resource; the installer installs
per-user
Decision: the exe's icon and version block come from a resource script that
`build.rs` generates — `IDI_MAIN` pointing at `install/quire.ico` plus a
`VS_VERSION_INFO` whose numbers it reads from `CARGO_PKG_VERSION` — and hands
to `rc.exe` through the `embed-resource` build-dependency. `install/make_icon.ps1`
renders that `.ico` with GDI+ (rounded gradient tile, ring + tail for the Q,
seven PNG-compressed frames from 16 to 256 px), so no image tool enters the
repo. `install/quire.iss` (Inno Setup 6), driven by
`install/build-installer.ps1`, produces `dist/Quire-<version>-windows-x64-setup.exe`
with a Start-menu entry, an optional desktop shortcut, an uninstaller, and an
unchecked task that adds Quire to the "Open with" list for `.md` — never a
default handler.
Why: two crates can embed a resource; `winres` wants a resource compiler
already on `PATH`, while `embed-resource` locates `rc.exe` through `vswhom`
and reports `NotAttempted` on a machine without the SDK instead of failing the
build — a clone that cannot package still compiles. The version stays written
once: `build.rs` reads `Cargo.toml`, and the `.iss` reads it back off the built
exe with `GetFileVersion`. Per-user install (`PrivilegesRequired=lowest`,
`{localappdata}\Programs\Quire`) is forced by the app itself: its default
database path is relative to the working directory (`appdata/quire.db`), so a
Program Files install would start with a non-writable one and fall back to
memory without saying so. (ADR-0020 moved the library to `%APPDATA%\Quire`, so
that premise is gone; the per-user install stays, because it is also what keeps
uninstalling from touching the notes.) Keeping the whole tree in the user
profile also means uninstalling leaves the notes alone — Inno removes only
directories it emptied.
Consequences: the window icon and the `--open <path>` dispatch the `.md`
association invoke both live in files this track may not edit (`ui/**`,
`src/app/**`, `src/main.rs`), so they went to Track A in `M8_FEEDBACK.md` with
the exact lines; until they land, the association only launches the app. An
`rc.exe` rejection now fails the build rather than shipping a plain exe.
`make_icon.ps1` is a manual step: change the palette and the `.ico` does not
follow until it runs again.

## ADR-0016 · Inline marks are the Markdown interchange format, spans in and
spans out
Decision: `import_service::parse_markdown` reads `**bold**`, `*italic*`,
`` `code` ``, `~~strike~~` and `[text](url)` into `core::types::Mark` spans
(byte offsets on char boundaries, the same sorted shape `Command::ToggleMark`
maintains), and `export_service::export_page` writes those spans back as
markers. Neither side builds an AST: the importer recurses into the span it
just matched, the exporter walks the mark *boundaries* and opens/closes markers
there, so nesting is expressed by the ranges — which is the shape the renderer
already consumes. Two shapes have no CommonMark spelling, because its inline
tree is strictly nested: marks of different kinds that only partially overlap,
and styling inside a code span (whose content is literal). The exporter cuts
the first at the crossing — every character keeps its text and each piece
keeps its kind — and drops the second. A literal marker in text is written as
an escape (`\*`), and a code span whose content starts or ends with a backtick,
or is itself padded with spaces, takes the space wrapper that CommonMark
strips back off.
Why: the document model is a flat span list, so anything that parsed a real
CommonMark tree would have to flatten it straight away; keeping the flat list
on both sides makes the pair testable by round trip (`export ∘ import` and
`import ∘ export` land on the same blocks) instead of by a conformance suite
this app cannot afford. Degrading an unrepresentable span into pieces, rather
than dropping it silently, keeps the text byte-exact — the property users
notice when they re-import their own notes.
Consequences: the round trip is a fixpoint from the second pass, not
byte-identical on the first for the crossing case (it comes back as two bold
and two italic pieces). Block-level ambiguity is out of scope and stays
literal: the exporter escapes inline markers but not a line-initial `#`, `-`,
`>` or `1.`, so a paragraph that *starts* with list syntax does not survive
re-import as a paragraph. Tables, images, setext headings and footnotes are
imported as text for the same reason.

## ADR-0015 · Crash recovery: rotating `VACUUM INTO` snapshots, restore at
open; settings and metadata as a diffed key/value layer
Decision: the durability story from M3 stands and is now on the record as
measured: `journal_mode=WAL` is stored in the file header (a later open reads
back `wal`), `synchronous=FULL` (=2), `locking_mode=normal`, `page_size=4096`,
`wal_autocheckpoint=1000` pages ≈ 4 MB, so SQLite folds the WAL back into the
main file by itself during a long session and a clean close checkpoints and
removes `-wal`. On top of that, `src/storage/backup.rs` gives SPEC §二十五 a
backup policy: every successful open shifts `<path>.bak1 → .bak2 → .bak3`
(dropping the oldest) and writes a fresh `.bak1`, and `SqliteRepository::open`
goes through `backup::open_with_recovery` — a main file that fails the startup
`integrity_check` is repaired before the app ever sees an error. Recovery
walks `.bak1 … .bak3`, validates each candidate by really opening it
(migrations + `PRAGMA integrity_check`), moves the unreadable main file aside
as `<path>.corrupt` and deletes its `-wal`/`-shm`, then *moves* (not copies)
the good snapshot into the main path and opens that. `Database::open` itself is
unchanged, so the existing "corruption is reported, not hidden" behavior is
still reachable (and still asserted by `storage::database::tests`).
For the UI's remembered state, `src/services/settings_store.rs` wraps the
frozen contract: `Settings` is a `BTreeMap` with the two keys the panels need
(`theme`, `sidebar.expanded`), and `SettingsStore` over `Arc<dyn Repository>`
offers `load_settings`/`load_meta`, `save_settings`/`save_meta` and the
non-writing `settings_changes`/`meta_changes`, which diff against what the
repository holds and emit only `SettingSet`/`MetaSet` changes — so a burst of
window-resize saves can be queued through `PersistenceService` and stay inside
the debounce window instead of writing per event.
Why `VACUUM INTO ?1` rather than a file copy or rusqlite's `Backup`: a copy of
`workspace.db` misses whatever still lives in `-wal` and can catch a torn page,
while `VACUUM INTO` reads through the live connection (WAL included), writes one
self-contained compacted file with no sidecar, and is a single statement on the
connection the snapshot already locks — so it is one consistent point in the
change stream, not a race. rusqlite's `Backup` offers the same consistency
page-by-page but needs a second destination connection; the statement is
simpler. The copy runs with `synchronous=OFF` and restores `FULL` afterwards,
which is safe because a snapshot is expendable (a torn copy fails its own
integrity check when recovery tries it, and the next open rewrites it) while
`VACUUM INTO` only reads the main database — measured at 2.3 MB: ≈23–49 ms
relaxed vs ≈45–256 ms at `FULL`, same process alternating rounds.
Consequences: startup pays ≈40 ms per 2.3 MB of workspace (PERFORMANCE.md,
M8 addendum) and the folder holds up to 3 extra copies of the database (5 since
ADR-0019) — both are the price of never opening a blank app after one bad
write. The loss
window is by design: `.bak1` is the database *as of the last successful open*,
so a corruption that arrives mid-session costs the edits made since startup;
closing that would mean rewriting the whole file every flush, which §三十三
rules out in spirit — Track A can call `backup::snapshot` from a "save a copy"
menu item if a real case appears. (ADR-0019 later closed this from the inside:
the family is five generations deep and `PersistenceService` snapshots on the
flush tick, so the window is "since the last snapshot", not "since startup".)
Recovery only answers *structural* damage: an
unknown `blocks.kind` still surfaces as `Corrupt` from `load()` (ADR-0013)
after a clean open, and that path is the app's to handle (M8_FEEDBACK.md). A
snapshot failure is logged and ignored — a read-only or full directory must
never block opening the document — so tests that want the unrecoverable case
have to delete the `.bak<N>` family first. Because the FTS5 mirror lives in the
same file (ADR-0014), a recovered database is searchable immediately, with no
rebuild; and because `Settings` treats an empty value as absent (the contract
has no `SettingDelete`), a stored-but-empty setting is indistinguishable from a
removed one.

Addendum (M8, branch `m8-rc`): that recovery is now *reportable*.
`backup::open_with_recovery` returns `(Database, OpenReport)` with
`recovered_from: Option<PathBuf>` (the snapshot that was moved into the main
path) and `backup_failed: bool` (this session has no snapshot behind it),
reached through the new `SqliteRepository::open_with_report`; `open()` keeps
its old signature and discards the report, so every existing caller and test
stays as it was. `OpenReport::log()` prints the two facts in the `eprintln!`
convention startup already uses, which is what `main.rs` calls now — a UI
warning can be built from the same fields without touching storage again
(closes M8_FEEDBACK #4's "no way to tell the user").

## ADR-0014 · Full-text search: FTS5 mirror inside the apply transaction,
CJK indexed by hand-built segmentation
Decision: search (SPEC §二十) uses two FTS5 virtual tables added by schema
version 2 — `search_pages(rowid = page id, title)` and
`search_blocks(rowid = block id, page_id UNINDEXED, text)` — written by
`src/storage/search_index.rs` from *inside* `SqliteRepository::apply`'s and
`replace_all`'s transaction: `insert_page`/`insert_block` index as they
insert, `PageTitleSet`/`BlockTextSet` re-index by rowid, and one orphan
sweep (`prune`) runs per batch that contained a delete, so the FK cascades
that remove blocks and pages need no per-row bookkeeping. The index can
therefore never drift from the document: an aborted batch rolls the index
back with the rows (ADR-0012). `Repository` and `Change` are untouched —
the query API is `SqliteRepository::search(&SearchRequest)` plus
`src/services/search_service.rs`, which aggregates raw matches into one
ranked `Hit` per page (bm25, block matches beat title matches for the
snippet slot) and offers `search_async` → `PendingSearch::poll` so the UI
thread never waits on SQLite (ARCHITECTURE hard rule 1).
Tokenizer: `unicode61`, which never splits inside a run of Han characters
("写作与中文测试" is one token, so "中文" would never match). Indexing
therefore stores a *segmented* copy — `segment()` gives every CJK character
(Han incl. ext. A/B–E, kana, Hangul) its own token — and queries are
segmented the same way, then issued as a *phrase* (`"中 文"*`) so only
adjacent characters match, reproducing the substring semantics of the M2
in-memory scan. Single words keep a trailing `*` for type-ahead.
Why not FTS5's `trigram` tokenizer (the usual CJK answer), measured with
the `#[ignore]`d `search_index::tests::fts5_capabilities` probe on the
bundled SQLite 3.53.2: trigrams need ≥ 3 characters, so a two-character
Chinese term — "中文", "字体", "行高", by far the common case — matches
nothing (`trigram "中文" -> []`, `"中文测" -> [1]`). It also indexes every
offset, inflating the DB for Latin text. Segmentation costs one extra pass
per write and keeps exact adjacency. The probe further confirms
`bro*` → "brown" (prefix works), `"quick br*"` → ∅ (`*` is only legal
*after* a whole phrase), and that an *unsegmented* Chinese phrase matches
nothing — i.e. query segmentation is mandatory, not cosmetic. FTS5 needed
no new Cargo feature: rusqlite 0.40 `bundled` (libsqlite3-sys 0.38) already
compiles SQLite with `-DSQLITE_ENABLE_FTS5`.
Consequences: a keystroke rewrites exactly one index row by rowid, so the
debounced write cost stays O(edited blocks) rather than O(page size) — the
10 000-block page of SPEC §二十二 keeps typing cheap (numbers in
PERFORMANCE.md "M3 · save latency"); the M2 linear-scan search in
`app/workspace.rs` stays in place until Track A wires the panel (both are
consistent with each other, no behavior change in this branch); migrating a
v1 database runs a one-time `search_index::rebuild` backfill after the
step commits (it needs its own transaction), and `check_schema` now also
requires the two FTS tables; `Arc<SqliteRepository>` must be kept around
by the app layer to build a `SearchService` (unsized coercion gives the
`Arc<dyn Repository>` the persistence pipeline wants — a plain
`Arc<dyn Repository>` cannot be downcast back); index rows for pages that
hold no text are simply absent, so an empty page is unsearchable by body
and by title alike; and the index is derived data — a rebuild is always
safe, which D4's recovery path relies on.

## ADR-0013 · Storage schema: cascade-FK tree + split block_children, one
transaction per contract call
Decision: the SQLite file uses the six SPEC §十八 tables — `workspaces`
(kept as the forward-compatible root, single row for now), `pages`
(self-referencing `parent` FK), `blocks` (identity + payload: page, kind,
text, checked), `block_children` (tree placement: `parent` FK + `ord`),
`metadata`, `settings`. Ids are the `core` u64 newtypes stored as `INTEGER`;
`OrderKey` maps u64→i64 by flipping the sign bit so signed storage keeps
unsigned ordering. Every mutation funnels through `Repository`: `apply`
takes one ordered `Change` list inside a single transaction (SPEC §十八),
deletions are recursive-CTE subtree deletes with `ON DELETE CASCADE` as
backstop, `replace_all` defers FK checks to commit and adds an explicit
acyclicity check (FKs alone cannot see an A→B→A cycle). Durability is
WAL + `synchronous=FULL` with a `PRAGMA integrity_check` at open
(SPEC §二十五); schema upgrades are forward-only steps in
`src/storage/migrations.rs` tracked by `PRAGMA user_version`.
Why: the split matches the SPEC's table list and keeps the hot payload
(text) on its own table for cheap `BlockTextSet` writes; cascade + CTE
makes delete semantics single-statement and testable; FULL is paid for
once per debounced burst (≈2–3 ms measured), not per keystroke (SPEC
§三十三 forbids the latter); cycle validation protects the sidebar tree.
Consequences: unknown `kind` strings or unreadable pages surface as
`StorageError::Corrupt` at startup instead of silent data loss; the
`workspaces` table is a placeholder until a real multi-workspace model
lands (feedback to Track A if M4+ needs it in `PersistedState`); a newer
`user_version` refuses to open rather than downgrade the file.

## ADR-0012 · Persistence contract lives in `core/`, storage implements it
Decision: `src/core/types.rs` defines the persisted model (`PageId`,
`BlockId`, `OrderKey`, `BlockKind`, `Block`, `Page`, `PersistedState`);
`src/core/persistence.rs` defines `Repository` (`load` / `apply(&[Change])
/ `replace_all`) plus the `Change` mutation enum. The M3 storage layer
(`src/storage/`, SQLite) implements the trait; the M4 editor emits
`Change`s from commands. The contract was frozen in one commit before the
two work streams started in parallel.
Why: two agents can then build storage and editor simultaneously without
interface drift; `core` stays free of Slint and of SQL details, and undo
(M4) replays inverse `Change`s rather than re-reading the DB (SPEC §十四).
Consequences: sibling order is a `u64` `OrderKey` with midpoint insertion
(`OrderKey::between`, renumber on exhaustion) — no per-insert row shifts;
deletes cascade in storage (undo replays captured `BlockInserted`s); `text`
is plain UTF-8 until M6 inline spans extend it. Any contract change goes
through one owner only (Track A) with the other side filing a feedback
note, never editing both sides at once.

## ADR-0011 · Headless visual regression via `quire-shot`
Decision: UI screenshots are produced by a second binary
(`src/bin/quire_shot.rs`) that installs a custom `Platform` whose window
adapter is Slint's `MinimalSoftwareWindow` (software renderer), renders the
real `AppWindow` into an RGB buffer, and writes a BMP; `just shot <scene>`
converts to PNG. Scenes (`--scene menu|rename|…`) set `UIState`/controller
state directly — no input injection.
Why: OS foreground policy blocks headless SendKeys, screen capture loses
to overlapping windows, and `PrintWindow` returns black pixels for
GPU-composited (GL) windows. The offscreen path is deterministic,
occlusion-proof, and doubles as the visual-regression harness for M3+.
Consequences: `quire-shot` builds only with `--features software`
(`required-features`), so the shipped binary stays lean; PopupWindow
show/close is exercised through the same code path as production.

## ADR-0010 · Page search = substring over a text blob; palette = commands only
Decision: search (Ctrl+P) scans `Page.search_text` (title + block text,
ASCII case-folding, char-boundary snippets, capped at 20 hits); the
command palette (Ctrl+K) lists commands only, generated from the
workspace. Both share `fuzzy_subsequence`-style matching only where it
helps (palette).
Why: content search must find unopened pages, which requires an inverted
index (SQLite FTS, M3+) or a flat blob; the blob is honest, fast at this
scale, and swappable. Separating palette and search mirrors the Notion
model and keeps command resolution unambiguous.
Consequences: `Workspace::search` is the single seam — M3 replaces its
body with FTS queries without touching UI or controller.

## ADR-0009 · UI event loop runs on an 8 MB-stack thread
Decision: both binaries spawn the Slint event loop / render on a thread
with an 8 MB stack (`std::thread::Builder::stack_size`).
Why: Slint 1.18 evaluates the component tree's initial property and
layout bindings recursively on the C stack; Quire's shell (sidebar tree
delegates + editor + five popup trees) needs slightly over the 1 MB
Windows default in debug builds, and popup open/close adds depth at
runtime. Verified by bisection: any single component removed masks it,
512 MB survives it — finite but deep. 8 MB is the standard Linux default
and costs only address-space reservation.
Consequences: crashes-in-the-field from stack exhaustion are off the
table for the planned M3–M6 growth; if a future Slint flattens binding
evaluation, the wrapper can be dropped in one place.

## ADR-0008 · Popup lifecycle is state-driven, never `is-open`-read
Decision: `CommandPalette` uses `close-policy: no-auto-close`; show/hide is
driven exclusively by `UIState.palette-open` (mirrored in AppWindow with
`show()`/`close()` handlers).
Why: Slint 1.18.0's const-propagation pass panics (`const_propagation.rs:
464`, no diagnostics) whenever a popup component *reads* its own `is-open`.
Verified by bisection with the `QUIRE_PROBE` subset-compile hook in build.rs.
Consequences: closing logic lives in UIState, which also makes the palette
testable from Rust; revisit if a future Slint fixes the crash.

## ADR-0007 · Page chrome rides inside delegates (ListView single-`for`)
Decision: a `ListView` may contain exactly one `for`; the page title and the
bottom spacer therefore live inside the first/last `DocumentRow` delegate
(`first` index check, `BlockRow.tail` flag set by Rust).
Why: 1.18 markup rejects sibling elements of a ListView `for`, and models
expose no `.length` to compute "last row" in the UI.
Consequences: Rust owns the `tail` invariant; re-sorting or appending blocks
must re-flag it (see `with_tail` in state.rs).

## ADR-0006 · M1 UI state via a single `UIState` global
Decision: transient UI state (dark, sidebar-open, palette state, selection
ids) lives in one Slint global; business actions are callbacks on that same
global, wired in `src/app/controller.rs`.
Why: components stay independently restorable; Rust reaches everything
through one generated accessor; there is exactly one place an agent must
read to understand UI state flow.
Consequences: property names are a contract — renaming requires touching
controller.rs and any component in the same change.

## ADR-0005 · Mock content is served through Slint models
Decision: sidebar tree, blocks, and command rows arrive from Rust as
`ModelRc<VecModel<struct>>`, even for M1 mock data.
Why: establishes the node-editor-cpp pattern (backend owns models, UI only
renders) before any real document model exists, so M3/M4 replace data
sources, not plumbing.
Consequences: mock data has the same shape discipline as real data.

## ADR-0004 · Renderer is a per-binary compile-time choice
Decision: `slint` is compiled with `default-features = false`; exactly one
renderer feature is enabled per build (`femtovg` default; `femtovg-wgpu`,
`skia`, `skia-opengl`, `software` selectable). Benchmarks build one binary
per renderer into its own target dir.
Why: comparing GPU stacks at runtime inside one binary would distort
memory numbers; separate binaries keep idle-RAM measurements honest.
Consequences: CI builds at least two renderer configurations.

## ADR-0003 · No TextEdit per block
Decision: static blocks render as `Text` + shapes; only the focused block
will own a real editing surface (M4), plus ListView-based virtualization
from the first commit.
Why: 10 000 interactive widgets is a guaranteed memory/CPU failure mode.
Consequences: caret/selection rendering inside the focused block must be
built deliberately (Slint TextInput + our selection model), not assumed.

## ADR-0002 · Windows IME = Slint + DirectWrite, no custom TSF
Decision: rely on Slint's winit text input (which uses OS IME composition
events); write a platform adapter only if a reproducible Slint/Windows bug
forces it.
Why: the previous input-method project proved TSF integration is the most
expensive kind of platform coupling; this app must stay agent-maintainable.
Consequences: Chinese input quality is a test item at M4, not a feature to
build.

## ADR-0001 · Slint + Rust, single process, no web runtime
Decision: Slint 1.18.x UI in the same process as the Rust core; SQLite for
persistence (M3); no Electron/Tauri/WebView/React/Vue anywhere.
Why: GPU-accelerated native rendering with real low-RAM/low-idle-CPU
behavior, and one language boundary (`.slint` ↔ Rust) that coding agents
can maintain for years.
Consequences: rich text editing and IME polish must be built rather than
inherited from a browser engine; this is accepted deliberately.

## Dependency policy
Every crate must answer: why needed / can std do it / runtime memory cost /
extra threads / build complexity. Current set: slint + slint-build (M1),
rusqlite with the bundled SQLite (M3 — persistence has no std answer), rfd
(M8 dialogs — native file pickers, no UI toolkit dependency), and
embed-resource as a *build* dependency only (M8 installer — it runs rc.exe and
adds nothing to the binary). Anything else waits for a milestone that cannot be
built without it.
