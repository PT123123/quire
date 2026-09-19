# M8 feedback — interface notes from Track B

Nothing here was changed by Track B: `Repository`, `Change`, `core/`, `app/`
and `ui/` are exactly as they arrived. These are the places where the frozen
contract forced a choice, so that changing the contract (Track A only) can
remove the choice instead of repeating it.

1. **No delete for the key/value namespaces.** `Change` has `SettingSet` and
   `MetaSet` but no `SettingDelete` / `MetaDelete`, so `settings_store` writes
   a removal as an empty value (`TOMBSTONE`) and hides empty values on read.
   Cost: "set to empty" and "removed" are the same stored row, and a removed
   key keeps a row in the table forever. With a delete variant the store's
   `remove` would emit it and `TOMBSTONE` disappears (one constant).

2. **`Change::PageCreated` carries an id, `BlockInserted` needs one from
   outside.** Import therefore takes `alloc: &mut dyn FnMut() -> BlockId` for
   blocks while the page arrives fully formed from the caller (see
   `import_markdown`'s signature). The asymmetry is the missing piece: the
   contract never says who owns id allocation, so paste, import and
   block-split each re-derive "one more than the highest id I can see". A
   `Document::next_block_id()` (or an allocator on the persistence side) would
   make those three agree; the injected-allocator signature is the workaround
   and stays compatible either way.

3. **A `dyn Repository` cannot reach the SQLite-only capabilities.** Both new
   query APIs are inherent methods on the concrete type — `SqliteRepository::
   search` (ADR-0014) and, in spirit, the backup family (ADR-0015) — because
   the trait is frozen. Practical consequence for the wiring pass: `AppState`
   stores `Arc<dyn Repository>`, and `Arc<SqliteRepository>` coerces *to* it
   but there is no way back. Keep one `Arc<SqliteRepository>` next to the state
   and hand the trait object to `PersistenceService`; `SearchService::new` and
   `SettingsStore::new` take `Arc<SqliteRepository>` / `Arc<dyn Repository>`
   respectively, so the app layer needs both at hand.

4. **Nothing tells the UI that a restore happened.** Recovery is inside
   `SqliteRepository::open`, which returns only the repository; the readable
   detail today is one stderr line (`quire: recovered … from …`). If the app
   should say "your database was damaged, we restored the copy from <time>",
   the open call needs a result that carries that fact (e.g.
   `OpenReport { recovered_from: Option<PathBuf>, backup_failed: bool }`).
   Track B did not invent a global for it.

5. **Logical corruption is reported by `load()`, not `open()`.** An unknown
   `blocks.kind` string raises `StorageError::Corrupt` from `load` (ADR-0013),
   after a clean open and a passing `integrity_check` — so the new backup
   policy cannot help that file, and `app/state.rs` currently responds by
   dropping persistence for the session while the document keeps the bad rows.
   Two candidate shapes for Track A: retry the open through
   `storage::backup::open_with_recovery` (it will consume a snapshot once the
   main file has been moved aside), or treat `load`-time `Corrupt` as "quarantine
   and start empty" with a user-visible notice.

6. **Setting key spellings.** The M3 fixtures use `settings["dark"] = "true"`
   and `settings["locale"]`, the storage card describes `metadata` as holding
   `current-page` / `font` / `theme`, and `settings_store` standardises on
   `theme` (`Settings::KEY_THEME`) plus `sidebar.expanded`. Pick one spelling
   per key while wiring, otherwise the panel writes a key nothing reads.

7. **Driving the editor from outside needs the `editing-*` properties.**
   `quire-typing` types through `set_editing_id` / `set_editing_text` /
   `set_pending_caret` + `invoke_editing_changed`, i.e. the same path a key
   press takes and the same one the 300 ms debounce watches. If M4's commit
   path changes shape (a different property pair, or a Slint `TextEdit` API
   upgrade), scene E's numbers stop being comparable with the ones in
   PERFORMANCE.md and the bench needs a matching update.

8. **Two small facts worth pinning somewhere closer to the code.** FTS5 is
   already compiled into the bundled SQLite (no Cargo feature to remember),
   and `bench.ps1`'s CPU% is a share of **one** core — the script now says so
   inline, and the Method section in PERFORMANCE.md was corrected to match.

## Resolved on `m8-rc` (Track B, M8 release candidate)

- **#4 — resolved in storage.** `SqliteRepository::open_with_report` hands the
  app an `OpenReport { recovered_from: Option<PathBuf>, backup_failed: bool }`
  (`open()` is unchanged, so no existing call site moved). `main.rs` now calls
  the new entry point and `report.log()` prints both facts to stderr; the UI
  warning is still Track A's to build, from those two fields plus
  `OpenReport::is_clean()` — no storage change needed for it.

- **Markdown import now fills `Block::marks` (D6, M6 tail).** `parse_markdown`
  reads `**bold**`, `*italic*`, `` `code` ``, `~~strike~~` and `[text](url)`
  into the same span shape `Command::ToggleMark` maintains, and
  `import_markdown` puts them on the `BlockInserted` changes it emits — so an
  imported page shows styled text without any UI change, because the renderer
  already walks `Block::marks`. `ParsedBlock` gained a `marks` field (it is
  built by the importer and read by tests; nothing in the app constructs one).
  Export writes spans back, including the two shapes CommonMark cannot hold,
  which are resolved as recorded in ADR-0016.

- **In-page find now has a data layer (D7): `find_service::FindSession`, fed
  the page's blocks rather than `query_in_page`.** The reason is in the
  contract:
  `SearchService` aggregates to *one* `Hit` per page (ranking is a
  whole-workspace concern) and its `snippet` is character-elided with `…`, so
  it cannot answer "which occurrence is this" or give a byte range. A session
  therefore takes `(&term, &[Block])` in display order — the slice the editor
  already holds — and answers `total() / position() / current() / next() /
  prev()`, where each `FindHit` carries `block` plus `start`/`end` byte offsets
  on char boundaries (the same convention `Mark` uses, so the same selection
  model highlights it). No IO, so a debounced rebuild costs one linear scan of
  the page; `query_in_page` stays what it is for the cross-page panel.
