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

- **Windows packaging (D8) lives in `install/`, and asks Track A for two
  lines.** `build.rs` writes the exe's `.rc` (icon + version block) and
  `install/quire.iss` builds the setup program; both are outside `ui/**` and
  `src/app/**`. These two are not:

  1. **Window/taskbar icon.** Slint exposes the window icon as a *property*, so
     it has to be set where the window is declared — in `ui/AppWindow.slint`
     (the root `Window`, or the `AppWindow` element that owns it):

     ```slint
     icon: @image-url("../install/quire.png");
     ```

     `install/quire.png` is the 256 px frame `make_icon.ps1` writes next to the
     `.ico`; Slint embeds it at compile time, so no loose file ships and the
     path is relative to the `.slint` file.
  2. **`.md` "Open with".** The registry verb the installer's optional task
     writes is `"{app}\quire.exe" --open "%1"`. `parse_launch_args` in
     `main.rs` ignores unknown arguments today, so double-clicking a `.md` file
     only launches the app. To finish it: add a `("--open", Some(v))` arm, and
     after `controller::wire(&ui, &state)` hand the path to the same sequence
     `import_markdown_dialog` runs (read file → `import_service::import_markdown`
     → apply the changes) minus the `rfd` picker.

## Track A resolutions (post-merge)

- **#4 addendum (Window::icon):** `Window::set_icon` does not exist in
  Slint 1.18 — implemented in a later release. The user-visible icon is
  already covered by the exe's embedded resource (D8): taskbar, explorer,
  installer shortcuts. Revisit when Slint ships the API.
- **#5 resolution:** `load()`-time `Corrupt` currently quarantines the
  session to memory (state.rs) while D4's restore-at-open covers physical
  damage; a full "quarantine file + reopen from snapshot + banner" chain
  lands with the OpenReport consumption in app/state.rs.

## M8 hardening round (D9–D12) — new notes

9. **The panic metadata needs one app-side line to reach the database.**
   `services::logging` runs before any repository exists (`main.rs` allows exactly
   one init call, and `src/app/**` is off-limits here), so `last_session_aborted`
   lands in `<data dir>/session.meta` rather than the `metadata` table. The
   follow-up is one statement where `AppState` has its `Arc<SqliteRepository>`
   (state.rs, next to the `PersistenceService::with_default_clock` call):

   ```rust
   if let Some(logger) = quire::services::logging::current() {
       let changes: Vec<Change> = logger.meta_entries().into_iter()
           .map(|(key, value)| Change::MetaSet { key, value }).collect();
       if !changes.is_empty() { let _ = repo.apply(&changes); }
   }
   ```

   Until then the fact is on disk and in the log, but not queryable, and no
   banner shows it. A `MetaDelete` (see #1) would let the app consume the entry
   after writing it.
10. **A panic is the only unclean end the logger can see.** The hook is the whole
    mechanism, so a kill, an access violation or a power loss leaves no report and
    the next start says nothing. Closing that needs either a second `main.rs` line
    on the exit path (`logging::note("session ended")` after the final flush, so a
    missing end-record is the abort signal) or a `Drop` guard the app layer owns.
    ADR-0018 records the trade-off; this round kept the one-line budget.
11. **`Cargo.toml` is outside the allowed set, so new integration-test files
    cannot be registered.** `tests/` here is not cargo-auto-discovered (the
    existing suites live in `tests/integration/` behind explicit `[[test]]`
    entries), so D9–D12 tests are unit tests inside their modules — the same place
    `services::persistence` and `services::settings_store` keep theirs. Adding a
    `logging_test` / `data_location_test` target is two lines in `[[test]]` when
    someone may edit `Cargo.toml`. D10 worked around it by extending the two
    already-registered suites (`backup`, `persistence`); a module with no suite of
    its own still has nowhere to go.
12. **The periodic snapshot needs one app-side line to be live.** The entry
    point is opt-in (`PersistenceService::with_snapshotter`, plus the
    `with_database_snapshots(&repo)` shortcut), and `app/state.rs` builds the
    service with `with_default_clock(r)` — which stays snapshot-free so a test
    double behaves exactly as before. The wiring is inside that same expression:

    ```rust
    let persistence = repo.clone().map(|r| {
        Arc::new(
            PersistenceService::with_default_clock(r.clone())
                .with_database_snapshots(&r),
        )
    });
    ```

    `with_database_snapshots` takes `&Arc<SqliteRepository>` because `Repository`
    has no snapshot method — #3's consequence, and the reason the concrete `Arc`
    has to be in hand at this point (it already is: `search_service` one line
    below takes it).

    Everything else is already reachable from that line: the app's flush timer is
    a `SingleShot` re-armed per recorded burst (`controller.rs`), so
    `persistence_force_flush` runs the snapshot check on a tick it already makes.
    Two readings of "every ten minutes" follow from that and are worth confirming
    before wiring: an idle session takes no snapshot (deliberate — nothing new to
    protect, and no resident thread this round), and ten minutes is the *minimum*
    gap measured from the previous snapshot, not a wall-clock cadence. If an exact
    cadence is ever wanted the change is `TimerMode::Repeated` in
    `install_flush_hook`, app-side, with no storage or service edit.
13. **D12 had to resolve the path from inside `storage`, so three app-side lines
    are now stale.** `src/main.rs` is off-limits this round, so the placement
    rules (`%APPDATA%\Quire`, `--db`, `--portable`) live in
    `storage::data_location` and `SqliteRepository::open_with_report` asks it
    before opening. What that leaves behind:

    - `main.rs`'s `if let Some(parent) = path.parent() { create_dir_all(parent) }`
      runs on the *requested* path, so every default run still creates an empty
      `appdata/` beside the working directory it no longer uses. `effective_path`
      creates the directory it actually opens, so those three lines can go.
    - `LaunchArgs` has no `portable` field, so `--portable` is re-read from
      `std::env::args()` by `data_location::scan`. Adding the field is not enough
      to remove that: the flag has to reach storage through the path argument,
      i.e. `main` resolves first —
      `launch.db.unwrap_or_else(|| data_location::effective_path(&data_location::legacy_db()))`
      — and `open_with_report` then receives a finished path. Until someone wants
      that, the re-scan is load-bearing, and it is why an *unknown* flag is
      harmless today: `parse_launch_args` ignores `--portable` and storage acts
      on it.
    - Nothing tells the user the library moved. `eprintln!` is invisible in a
      `windows_subsystem = "windows"` build, so the notice needs the same route
      the recovery one already takes: an `OpenReport::migrated_from:
      Option<PathBuf>` beside `recovered_from`, and one more `set_db_notice` call
      next to `main.rs`'s existing `if let Some(from) = &recovered`. ADR-0020
      lists it as the unfinished part of the move.
