# Editor Architecture

Design for the block editor — M4 core landed (commands, undo/redo, single
TextInput editing), M5 slash/block menus landed, M6 marks model +
persistence landed. This doc records the design AND the platform walls hit
during implementation.

## List nesting (M4)

Tab / Shift+Tab on bullet/numbered/todo items nest and promote (depth 1
max). The model is honest: `parent` + order keys, children sort between
their parent and the parent's next sibling; the projection computes each
block's depth and the renderer indents by 24px per level. Empty list
items leave the list on Enter (convert to a top-level paragraph, one
undo step) and on Backspace-at-start. `MoveBlock` stays within a sibling
run — cross-parent swaps are refused at plan time.

## In-page find (M7)

The Ctrl+F bar consumes Track B's `FindSession` (services/find_service.rs):
every keystroke rebuilds the session (linear page scan, no IO); stepping
navigates hits by routing through the hit block's editing input — the
delegate re-creates with a pending byte-range selection so the hit is
visibly selected. Scroll-to-block is NOT possible yet (ListView delegates
give no absolute geometry) — navigation is selection-based.

## Platform wall: inline rich text rendering

Slint `Text` has no inline formatting (no per-run style, no decoration).
Marks are therefore rendered as a HorizontalLayout of per-mark runs with
`clip: true`. Consequences, accepted for now:

- runs do not reflow across each other — a marked paragraph wraps per run,
  so long marked text shows reflow artifacts;
- italic uses the "Segoe UI Italic" family name (no font-style property);
- strike is a 1px Rectangle overlay; code runs get the code background;
- the live TextInput always shows plain text (marks visible when blurred).

When Slint ships rich text support, `build_runs` in state.rs and the runs
layout in EditorBlock.slint are the only two seams to replace.

## Model

```
Document (M3: one row per page in SQLite)
  └─ Page
       └─ Block tree (flat rows ordered by parent + order key)
            id          stable i64 (ULID/UUID — never an array index)
            parent_id   NULL = top-level
            order       fractional ranking key (insert between neighbors)
            kind        paragraph | heading_1..3 | bullet | numbered | todo
                        | quote | code | divider      (M4 set)
                        | callout | page | link_to_page | toggle
                        | image | file | table | table_cell   (M10)
                        | columns | column                   (M10)
            text        plain UTF-8 (M4) / inline span refs (M6)
            checked     todo only
            folded      toggle only: its subtree gets no editor rows
            attachment  image / file only: id into the `attachments` table
            columns     table only: M of an N×M grid; cells are its children,
                        row-major, so rows = cells / columns is derived.
                        columns layout only: the box count, same integer
```

- The **in-memory truth** is `core::Document` owning a page's blocks in a
  slotmap/id-indexed structure. The UI receives a **projection**:
  `Vec<BlockRow>` exactly like today's mock (`tail` flag included).
- Nesting (bullet/numbered children) is modeled with `parent_id`, but M4
  renders one indent level at most; deep trees wait for M7 virtualization
  work.
- Three kinds have children that must **not** become editor rows: a folded
  toggle's subtree (ADR-0028), a table's cells (ADR-0031) and a layout's boxes
  *and every line inside those boxes* (ADR-0032). `project_blocks`
  deletes them from the `Vec` rather than marking them invisible, and
  `visible_block_indices` is the single row→model translation seam — anything
  that turns a delegate index into a block index goes through it.

## Command system

Every edit is a `Command` in `core::command`:

```
InsertText { id, pos, text }      DeleteText { id, range }
SplitBlock { id, pos }            MergeBlock { id, with_prev }
SetBlockType { id, kind }         MoveBlock { id, before_of }
InsertBlock { after, kind }       DeleteBlock { id }
ToggleTodo { id }                 ApplyMark { id, range, mark }   (M6)
TableAddRow { id, row }           TableAddColumn { id, col }      (M10)
TableDeleteRow { id, row }        TableDeleteColumn { id, col }
ColumnsAddColumn { id }           ColumnsDeleteColumn { id }      (M10)
ColumnsAddBlock { id }
```

- Commands apply to the in-memory document and push an inverse onto the
  undo stack. Undo/Redo never reads the database (SPEC §十四).
- The controller translates UI callbacks into commands; the editor
  produces *no* direct model mutation from `.slint` code.
- A table command is not "edit a grid" — it is the batch of cell
  `BlockInserted`/`BlockDeleted` changes plus one `BlockColumnsSet` that a grid
  edit decomposes into, so the Tab that grows a table past its last cell is one
  undo step. `plan()` refuses (returns `None`, pushes nothing) for a ragged
  grid, a cell as a Turn-into endpoint, and any delete that would take the
  table below 1×1.
- A layout is the same argument one level deeper: `ColumnsAddColumn` is one
  `BlockColumnsSet` plus a `Column` block plus the paragraph that keeps the new
  box non-empty, and `ColumnsDeleteColumn` re-keys the deleted box's lines onto
  the box before it — so a box's words are never on the clipboard of an undo.
  `plan()` refuses a third box on a three-box layout, a delete below two, and
  any of the three on a block that is not the right kind.
- `InsertBlockAfter` reads its anchor's *kind* (ADR-0032): after a container
  (`Table`, `Columns`) the new block lands after the container's whole subtree,
  and after a slot (`TableCell`, `Column`) the command refuses, because such a
  block has no row of its own to sit next to. Anywhere else it inherits the
  anchor's parent — it used to force `None` — so a "Paste below" or a "+" on a
  nested list item stays inside that list instead of promoting the new block to
  the page.

## Editing surface: one TextEdit, rest are Text

Per ADR-0003, a page is `Text` delegates. On focus/click:

1. The clicked block's delegate swaps to a `TextInput` (the only live
   editor instance in the window).
2. The `TextInput` is initialized from the block's text; caret/selection
   stay inside it. IME comes from Slint's winit integration (ADR-0002).
3. Keys that mean structure (Enter, Backspace at pos 0, Tab, arrows
   crossing block borders) are intercepted in `key-pressed`, routed as
   commands, and re-targeted: the editor moves to the neighboring block
   and re-focuses the single `TextInput` there.

`EditorState` (Rust-owned, mirrored read-only into `UIState`):
`focused_block_id`, `selection_anchor`, `selection_cursor`,
`composition_active`, `editing_mode`. No `.slint` file keeps a private
selection.

## Rendering contract (kept from M1/M2)

- `EditorBlock` stays the static renderer (all kinds, `visible` toggles).
- A block being edited swaps to `BlockEditor` (TextInput + block chrome).
- A `table` is the first exception to "one delegate, one block": `EditorBlock`
  hands the whole grid to `TableBlock`, which draws every cell and hosts the one
  live `TextInput` when `UIState.editing-id` names one of them. Tab / Shift-Tab
  route to `UIState.table-cell-move`, which asks Rust for the next cell and grows
  the table when the step runs off the end.
- A `columns` layout is that exception twice over, because Slint has no recursive
  components and a box therefore cannot host an `EditorBlock`: `ColumnsBlock`
  draws the boxes *and* every line inside them (`ColumnItemRow`, the smaller
  sibling of `TableBlock`'s cell editor), and the layout's row stays the only one
  the ListView sees. Tab / Shift-Tab route to `UIState.column-item-move`, which
  stops at either end of the layout instead of walking out of it; an empty box
  routes its click to `UIState.column-fill`, because a layout is the only thing
  on the page a pointer can hit and a caret cannot.
- The ListView keeps virtualizing; `tail` flag logic stays in Rust.
- Typed text flows: `TextInput` edit → controller debounces (≈300 ms) →
  `InsertText` command → document update → **targeted** model row update
  (never `set_vec` on the whole page while typing).

## Testing

- `core/` command application and undo stacks: plain unit tests.
- Round-trip persistence: M3 integration tests over a temp SQLite.
- IME + caret behavior (Chinese composition, Enter/Backspace inside
  composition): manual test checklist at M4, per SPEC §十一; not
  scriptable on this desktop (foreground policy), so it is a named
  acceptance item, not an afterthought.
- Visual: `just shot` scenes gain editor states (focused block, selection
  across blocks) in M4.
