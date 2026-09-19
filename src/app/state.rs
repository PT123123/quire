// Application state + mock content for M2/M4.
//
// The page tree lives in `workspace.rs` (pure, unit-tested); the block
// content of every page lives in `core::Document` since M4 (the editing
// truth, mutated only through commands). This module is the view-
// projection layer between the two and the Slint models (SidebarNode /
// BlockRow / CommandRow / SearchRow / MenuRow).

use crate::app::workspace::{SearchHit, Workspace, BENCH_ID_BASE, MAX_RECENTS};
use crate::core::persistence::{Change, Repository};
use crate::core::{Block, BlockId, BlockKind, Command, Document, History, OrderKey, PageId};
use crate::services::find_service::FindSession;
use crate::services::persistence::PersistenceService;
use crate::services::search_service::SearchService;
use crate::storage::search_index::SearchRequest;
use crate::storage::SqliteRepository;
use crate::{BlockRow, CommandRow, MenuRow, SearchRow, SidebarNode, SlashRow, TextRun};
use slint::{Model, ModelRc, VecModel};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

pub struct AppState {
    pub workspace: RefCell<Workspace>,
    pub sidebar: Rc<VecModel<SidebarNode>>,
    pub blocks: Rc<VecModel<BlockRow>>,
    pub commands: Rc<VecModel<CommandRow>>,
    pub search: Rc<VecModel<SearchRow>>,
    pub menu: Rc<VecModel<MenuRow>>,
    pub slash: Rc<VecModel<SlashRow>>,
    pub block_menu: Rc<VecModel<MenuRow>>,
    /// Full command list before query filtering.
    pub all_commands: Vec<CommandRow>,
    /// The editing truth for every page's blocks (M4).
    pub doc: RefCell<Document>,
    /// Per-page undo/redo stacks.
    pub history: RefCell<History>,
    /// Debounced persistence pipeline (M3). `None` = headless/test mode.
    pub persistence: Option<Arc<PersistenceService>>,
    /// FTS-backed search (M7). `None` falls back to the in-memory scan.
    pub search_service: Option<Arc<SearchService>>,
    /// In-flight async search with its generation; superseded queries drop
    /// their result instead of overwriting newer ones.
    pending_search: RefCell<Option<(u64, crate::services::search_service::PendingSearch)>>,
    /// Ctrl+F in-page find session (Track B's FindSession).
    find_session: RefCell<Option<FindSession>>,
    find_label: RefCell<String>,
    /// Page to open instead of the default landing page (persisted
    /// "current-page" meta).
    restore_current: Cell<Option<i32>>,
    search_generation: Cell<u64>,
    /// Persisted sibling order of every page (drives PageCreated/Moved).
    page_order: RefCell<HashMap<i32, OrderKey>>,
    /// Installed by the controller: restarts the flush timer on record().
    flush_hook: RefCell<Option<Rc<dyn Fn()>>>,
    /// Cross-block clipboard (menu-driven copy/paste of one block).
    clipboard: RefCell<Option<Block>>,
    /// Persisted settings (theme etc.), loaded from storage at startup.
    settings: RefCell<HashMap<String, String>>,
    /// UI weak handle, installed by the controller at wire time — lets the
    /// state push display-only projections (page stats) without a callback.
    ui: RefCell<Option<slint::Weak<crate::UIState<'static>>>>,
    /// Persisted recent-page ids, restored before first open.
    recents_restored: Cell<Vec<i32>>,
    /// Startup notice (e.g. database restored from backup); consumed by the
    /// controller and shown once in the shell.
    db_notice: RefCell<Option<String>>,
    /// Currently open page (0 = none / empty workspace).
    pub open_page: Cell<i32>,
    /// Page awaiting delete confirmation.
    pub pending_delete: Cell<Option<i32>>,
    /// Benchmark scroll bookkeeping (scene F): last viewport-y seen.
    pub last_scroll_y: Cell<f32>,
}

/// Block ids start above this so they never collide with anything derived
/// from mock page ids.
const BLOCK_ID_BASE: u64 = 1_000_000_000;

pub struct HandleArgs {
    /// Number of mock blocks for benchmarks (0 = default sample document).
    pub blocks: usize,
    /// Quit the event loop after N seconds (0 = never).
    pub auto_exit_secs: f64,
    /// Scene G: number of extra flat pages to switch between.
    pub bench_pages: usize,
}

/// Build the mock/bench session (fresh database or no persistence).
fn build_mock_session(args: &HandleArgs) -> (Workspace, Document, HashMap<i32, OrderKey>) {
    let mut workspace = if args.bench_pages > 0 {
        Workspace::with_bench_pages(args.bench_pages)
    } else {
        Workspace::sample()
    };
    let mut doc = Document::new(BLOCK_ID_BASE);
    for id in workspace.dfs_order() {
        let title = workspace.title_of(id).unwrap().to_string();
        let rows = if args.blocks > 0 && id == PAGE_ATLAS {
            mock_blocks_bench(args.blocks)
        } else if id >= BENCH_ID_BASE {
            mock_blocks_bench_page(&title)
        } else {
            mock_blocks_for_page(id, &title)
        };
        let blob = block_search_blob(&title, &rows);
        workspace.set_search_text(id, blob);
        let core_blocks = rows_to_blocks(id, rows, &mut doc);
        doc.set_page_blocks(core_page_id(id), core_blocks);
    }
    let page_order = assign_page_orders(&workspace);
    (workspace, doc, page_order)
}

/// Chain sibling order keys per parent group (deterministic seed order).
fn assign_page_orders(workspace: &Workspace) -> HashMap<i32, OrderKey> {
    fn walk(ws: &Workspace, parent: Option<i32>, map: &mut HashMap<i32, OrderKey>) {
        let mut prev: Option<OrderKey> = None;
        for id in ws.children_of(parent) {
            let key = OrderKey::between(prev, None).expect("append order exhausted");
            map.insert(id, key);
            prev = Some(key);
            walk(ws, Some(id), map);
        }
    }
    let mut map = HashMap::new();
    walk(workspace, None, &mut map);
    map
}

fn workspace_from_persisted(
    state: &crate::core::PersistedState,
) -> (Workspace, HashMap<i32, OrderKey>) {
    let ws = Workspace::from_persisted(&state.pages);
    let mut map = HashMap::new();
    for p in &state.pages {
        map.insert(p.id.0 as i32, p.order);
    }
    (ws, map)
}

impl AppState {
    pub fn new(args: &HandleArgs, repo_in: Option<Arc<SqliteRepository>>) -> Rc<Self> {
        // Try to load persisted state. A failed load disables persistence
        // for the session rather than risking a seed-flush over live data.
        let mut repo = repo_in;
        let loaded = match repo.take() {
            Some(r) => match r.load() {
                Ok(state) => {
                    repo = Some(r);
                    Some(state)
                }
                Err(e) => {
                    eprintln!("quire: load failed ({e}); running without persistence");
                    None
                }
            },
            None => None,
        };
        let persisted = loaded.filter(|s| !s.pages.is_empty());
        let mut restored_settings: HashMap<String, String> = HashMap::new();
        let mut restored_recents: Vec<i32> = Vec::new();
        let mut restored_current: Option<i32> = None;
        if let Some(state0) = &persisted {
            restored_settings = state0
                .settings
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            restored_recents = state0
                .meta
                .get("recents")
                .map(|v| v.split(',').filter_map(|x| x.parse::<i32>().ok()).collect())
                .unwrap_or_default();
            restored_current = state0
                .meta
                .get("current-page")
                .and_then(|v| v.parse::<i32>().ok());
        }

        let (workspace, mut doc, page_order, seed) = match &persisted {
            Some(state) => {
                let (ws, orders) = workspace_from_persisted(state);
                let mut d = Document::new(BLOCK_ID_BASE);
                let mut by_page: HashMap<PageId, Vec<Block>> = HashMap::new();
                for b in &state.blocks {
                    by_page.entry(b.page).or_default().push(b.clone());
                }
                for (pid, blocks) in by_page {
                    d.set_page_blocks(pid, blocks);
                }
                (ws, d, orders, Vec::<Vec<Change>>::new())
            }
            None => {
                let (ws, d, orders) = build_mock_session(args);
                (ws, d, orders, Vec::<Vec<Change>>::new())
            }
        };

        // bench content overrides the loaded page (in memory only; never
        // recorded, so scene D stays deterministic across runs)
        if args.blocks > 0 {
            let title = workspace.title_of(PAGE_ATLAS).unwrap_or("").to_string();
            let rows = mock_blocks_bench(args.blocks);
            let core_blocks = rows_to_blocks(PAGE_ATLAS, rows, &mut doc);
            doc.set_page_blocks(core_page_id(PAGE_ATLAS), core_blocks);
            let _ = title;
        }

        let persistence = repo
            .clone()
            .map(|r| Arc::new(PersistenceService::with_default_clock(r)));
        let search_service = repo.map(crate::services::search_service::SearchService::new_arc);

        // fresh database: record the whole session once so a restart
        // reproduces exactly this state
        if let (Some(p), true) = (&persistence, persisted.is_none()) {
            let mut batch = Vec::new();
            for (id, title, parent, favorite, expanded) in workspace.page_seed_rows() {
                batch.push(Change::PageCreated(crate::core::Page {
                    id: PageId(id as u32 as u64),
                    title,
                    parent: parent.map(|v| PageId(v as u32 as u64)),
                    order: *page_order.get(&id).unwrap_or(&OrderKey::FIRST),
                    favorite,
                    expanded,
                }));
            }
            for id in workspace.dfs_order() {
                for b in doc.page_blocks(core_page_id(id)) {
                    batch.push(Change::BlockInserted(b.clone()));
                }
            }
            p.record_all(vec![batch]);
            // best effort: a failed first write surfaces on the next flush
            let _ = p.force_flush();
        }
        let _ = seed;

        let open = if args.blocks > 0 {
            PAGE_ATLAS
        } else {
            PAGE_GETTING_STARTED
        };
        let all_commands = mock_commands(&workspace);
        let blocks = Rc::new(VecModel::from(Vec::new()));
        let commands = Rc::new(VecModel::from(all_commands.clone()));
        let state = AppState {
            workspace: RefCell::new(workspace),
            sidebar: Rc::new(VecModel::from(Vec::new())),
            blocks,
            commands,
            search: Rc::new(VecModel::from(Vec::new())),
            menu: Rc::new(VecModel::from(Vec::new())),
            slash: Rc::new(VecModel::from(slash_items(""))),
            block_menu: Rc::new(VecModel::from(Vec::new())),
            clipboard: RefCell::new(None),
            settings: RefCell::new(restored_settings),
            recents_restored: Cell::new(restored_recents),
            restore_current: Cell::new(restored_current),
            ui: RefCell::new(None),
            db_notice: RefCell::new(None),
            all_commands,
            doc: RefCell::new(doc),
            history: RefCell::new(History::default()),
            persistence,
            search_service,
            pending_search: RefCell::new(None),
            search_generation: Cell::new(0),
            find_session: RefCell::new(None),
            find_label: RefCell::new(String::new()),
            page_order: RefCell::new(page_order),
            flush_hook: RefCell::new(None),
            open_page: Cell::new(0),
            pending_delete: Cell::new(None),
            last_scroll_y: Cell::new(0.0),
        };
        // restore persisted recents before the first open marks its page
        let state = Rc::new(state);
        {
            let recents = state.recents_restored.take();
            if !recents.is_empty() {
                state.workspace.borrow_mut().set_recents(recents);
            }
        }
        state.open_page(open);
        state
    }

    // ---- models ----

    pub fn sidebar_model(&self) -> ModelRc<SidebarNode> {
        ModelRc::from(self.sidebar.clone())
    }
    pub fn blocks_model(&self) -> ModelRc<BlockRow> {
        ModelRc::from(self.blocks.clone())
    }
    pub fn commands_model(&self) -> ModelRc<CommandRow> {
        ModelRc::from(self.commands.clone())
    }
    pub fn search_model(&self) -> ModelRc<SearchRow> {
        ModelRc::from(self.search.clone())
    }
    pub fn menu_model(&self) -> ModelRc<MenuRow> {
        ModelRc::from(self.menu.clone())
    }
    pub fn slash_model(&self) -> ModelRc<SlashRow> {
        ModelRc::from(self.slash.clone())
    }
    pub fn block_menu_model(&self) -> ModelRc<MenuRow> {
        ModelRc::from(self.block_menu.clone())
    }

    // ---- projections ----

    /// Rebuild the flat sidebar model: Favorites, Recent, Workspace tree,
    /// and the trailing "New page" action row. `row.y` is the cumulative
    /// pixel offset inside the tree area (used to anchor the context menu).
    pub fn rebuild_sidebar(&self) {
        self.sidebar.set_vec(self.build_sidebar_rows());
    }

    pub fn build_sidebar_rows(&self) -> Vec<SidebarNode> {
        let ws = self.workspace.borrow();
        let open = self.open_page.get();
        let mut rows: Vec<SidebarNode> = Vec::new();
        let mut y = 0;

        let push = |rows: &mut Vec<SidebarNode>, y: &mut i32, node: SidebarNode| {
            let mut n = node;
            n.y = *y;
            *y += if n.kind == "header" { 26 } else { 28 };
            rows.push(n);
        };

        let favorites = ws.favorites();
        if !favorites.is_empty() {
            push(&mut rows, &mut y, header("Favorites"));
            for (id, title) in favorites {
                push(&mut rows, &mut y, leaf_row(id, &title, "favorite", false));
            }
        }
        let recents = ws.recents();
        if !recents.is_empty() {
            push(&mut rows, &mut y, header("Recent"));
            for (id, title) in recents.iter().take(MAX_RECENTS) {
                push(&mut rows, &mut y, leaf_row(*id, title, "recent", false));
            }
        }

        push(&mut rows, &mut y, header("Workspace"));
        for r in ws.tree_rows() {
            rows.push(SidebarNode {
                id: r.id,
                label: r.label.into(),
                kind: "page".into(),
                depth: r.depth,
                expanded: r.expanded,
                has_children: r.has_children,
                selected: r.id == open,
                y: y,
            });
            y += 28;
        }
        // trailing "new page" action row
        rows.push(SidebarNode {
            id: ROW_NEW_PAGE,
            label: "New page".into(),
            kind: "new-page".into(),
            depth: 0,
            expanded: false,
            has_children: false,
            selected: false,
            y: y,
        });
        rows
    }

    pub fn sidebar_row_y(&self, id: i32) -> i32 {
        self.sidebar
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.y)
            .unwrap_or(0)
    }

    // ---- operations (called by the controller) ----

    pub fn open_page(&self, id: i32) {
        {
            let mut ws = self.workspace.borrow_mut();
            if !ws.contains(id) {
                return;
            }
            ws.mark_opened(id);
            ws.expand_ancestors(id);
        }
        // persist the recent list + last-opened page
        self.record(vec![Change::MetaSet {
            key: "current-page".into(),
            value: id.to_string(),
        }]);
        let recents = self.workspace.borrow().recents_ids();
        self.record(vec![Change::MetaSet {
            key: "recents".into(),
            value: recents
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(","),
        }]);
        self.open_page.set(id);
        self.reproject_blocks();
        self.rebuild_sidebar();
    }

    /// Rebuild the editor rows from the Document (page switch, undo/redo,
    /// structural edits). Typing never goes through here.
    pub fn reproject_blocks(&self) {
        let page = self.open_page.get();
        let rows = {
            let doc = self.doc.borrow();
            project_blocks(doc.page_blocks(core_page_id(page)))
        };
        self.blocks.set_vec(rows);
        self.update_page_stats();
    }

    /// Controller installs the UI weak handle at wire time.
    pub fn set_ui(&self, ui: slint::Weak<crate::UIState<'static>>) {
        *self.ui.borrow_mut() = Some(ui);
    }

    /// Word/char counts for the editor footer (display-only push).
    pub fn update_page_stats(&self) {
        let page = core_page_id(self.open_page.get());
        let (words, chars) = {
            let doc = self.doc.borrow();
            let mut words = 0;
            let mut chars = 0;
            for b in doc.page_blocks(page) {
                let t = b.text.trim();
                if !t.is_empty() {
                    words += t.split_whitespace().count();
                }
                chars += b.text.chars().count();
            }
            (words, chars)
        };
        if let Some(ui) = self.ui.borrow().clone() {
            let g = ui.upgrade().unwrap();
            g.set_page_stats(format!("{words} words · {chars} chars").into());
        }
    }

    /// Controller installs the UI weak handle at wire time.

    /// Poll the in-flight search; called by the controller on a short
    /// timer while a query is pending. Returns rows when a result landed.
    fn next_search_generation(&self) -> u64 {
        let g = self.search_generation.get() + 1;
        self.search_generation.set(g);
        g
    }

    pub fn poll_search(&self) -> Option<Vec<SearchRow>> {
        let (gen, pending) = self.pending_search.borrow_mut().take()?;
        if gen != self.search_generation.get() {
            return Some(Vec::new()); // superseded: show nothing
        }
        match pending.poll() {
            Some(Ok(hits)) => {
                let rows: Vec<SearchRow> = hits
                    .iter()
                    .map(|h| SearchRow {
                        page_id: h.page.0 as i32,
                        title: h.title.clone().into(),
                        snippet: if h.snippet.is_empty() {
                            self.workspace.borrow().breadcrumb(h.page.0 as i32).into()
                        } else {
                            h.snippet.clone().into()
                        },
                    })
                    .collect();
                Some(rows)
            }
            Some(Err(_)) => Some(Vec::new()),
            None => {
                // still running: put it back
                *self.pending_search.borrow_mut() = Some((gen, pending));
                None
            }
        }
    }

    pub fn search_in_flight(&self) -> bool {
        self.pending_search.borrow().is_some()
    }

    /// Blocking search for tools/headless scenes (the GUI path is async).
    pub fn set_search_rows_sync(&self, query: &str) {
        if let Some(svc) = &self.search_service {
            if !query.trim().is_empty() {
                let rows: Vec<SearchRow> = match svc.query(query) {
                    Ok(hits) => hits
                        .iter()
                        .map(|h| SearchRow {
                            page_id: h.page.0 as i32,
                            title: h.title.clone().into(),
                            snippet: if h.snippet.is_empty() {
                                self.workspace.borrow().breadcrumb(h.page.0 as i32).into()
                            } else {
                                h.snippet.clone().into()
                            },
                        })
                        .collect(),
                    Err(_) => Vec::new(),
                };
                self.search.set_vec(rows);
                return;
            }
        }
        self.set_search_query(query);
    }

    pub fn set_db_notice(&self, notice: String) {
        *self.db_notice.borrow_mut() = Some(notice);
    }

    pub fn take_db_notice(&self) -> Option<String> {
        self.db_notice.borrow_mut().take()
    }

    pub fn page_order_of(&self, id: i32) -> OrderKey {
        *self
            .page_order
            .borrow()
            .get(&id)
            .unwrap_or(&OrderKey::FIRST)
    }

    /// Queue a change batch for the debounced flush and arm the timer.
    pub fn record(&self, changes: Vec<Change>) {
        if changes.is_empty() {
            return;
        }
        if let Some(p) = &self.persistence {
            p.record(changes);
        }
        if let Some(hook) = &*self.flush_hook.borrow() {
            hook();
        }
    }

    /// Controller installs the flush-timer trigger at wiring time.
    pub fn install_flush_hook(&self, hook: Box<dyn Fn()>) {
        *self.flush_hook.borrow_mut() = Some(Rc::new(hook));
    }

    /// Write everything queued so far (Ctrl+S, app exit).
    pub fn persistence_force_flush(&self) {
        if let Some(p) = &self.persistence {
            // errors surface through take_last_error on the next call;
            // nothing actionable at the exit path
            let _ = p.force_flush();
        }
    }

    /// Run a command without reprojecting (typing): the caller keeps the
    /// delegate alive and syncs the single row itself.
    pub fn exec_editor(&self, cmd: Command) -> Option<Vec<Change>> {
        let page = core_page_id(self.open_page.get());
        let changes = crate::core::command::exec(
            &mut self.doc.borrow_mut(),
            &mut self.history.borrow_mut(),
            page,
            cmd,
        )?;
        self.record(changes.clone());
        Some(changes)
    }

    /// Run a command and refresh the editor rows (structural edits).
    pub fn exec_on_open_page(&self, cmd: Command) -> Option<Vec<Change>> {
        let changes = self.exec_editor(cmd)?;
        self.reproject_blocks();
        Some(changes)
    }

    /// Set the theme and persist it (settings table; key spelling matches
    /// services/settings_store.rs `Settings::KEY_THEME`).
    pub fn set_dark(&self, dark: bool) {
        let value = if dark { "dark" } else { "light" };
        self.settings
            .borrow_mut()
            .insert("theme".into(), value.into());
        self.record(vec![Change::SettingSet {
            key: "theme".into(),
            value: value.into(),
        }]);
    }

    pub fn dark_setting(&self) -> bool {
        self.settings
            .borrow()
            .get("theme")
            .map(|v| v == "dark")
            .unwrap_or(false)
    }

    // ---- in-page find (Ctrl+F; data layer = Track B's FindSession) ----

    /// (Re)build the session for `term` over the open page's blocks.
    pub fn find_start(&self, term: &str) {
        let page = core_page_id(self.open_page.get());
        let blocks = self.doc.borrow().page_blocks(page).to_vec();
        let session = FindSession::new(term, &blocks);
        let label = if session.is_empty() {
            "no matches".to_string()
        } else {
            format!("0 / {}", session.total())
        };
        *self.find_label.borrow_mut() = label;
        *self.find_session.borrow_mut() = Some(session);
    }

    /// Step to the next/previous hit. Returns (block id as i32, start, end)
    /// for the UI to select, plus refreshes the position label.
    pub fn find_step(&self, next: bool) -> Option<(i32, usize, usize)> {
        let mut slot = self.find_session.borrow_mut();
        let session = slot.as_mut()?;
        let hit = if next { session.next() } else { session.prev() }?;
        let position = session.position()?;
        let total = session.total();
        drop(slot);
        *self.find_label.borrow_mut() = format!("{}/{}", position + 1, total);
        Some((hit.block.0 as i32, hit.start, hit.end))
    }

    pub fn find_label(&self) -> String {
        self.find_label.borrow().clone()
    }

    pub fn find_close(&self) {
        *self.find_session.borrow_mut() = None;
        *self.find_label.borrow_mut() = String::new();
    }

    // ---- slash menu (descriptors owned by Rust, per SPEC §十五) ----

    /// Filter the block-kind descriptors by the text after "/".
    pub fn open_slash(&self, filter: &str) {
        let needle = filter.to_lowercase();
        let rows: Vec<SlashRow> = SLASH_ITEMS
            .iter()
            .filter(|(_, label, _)| needle.is_empty() || label.to_lowercase().contains(&needle))
            .map(|(kind, label, hint)| SlashRow {
                id: kind_to_int(*kind),
                label: (*label).into(),
                hint: (*hint).into(),
            })
            .collect();
        self.slash.set_vec(rows);
    }

    pub fn slash_focus_count(&self) -> i32 {
        self.slash.row_count() as i32
    }

    pub fn slash_selected_kind(&self, focus: i32) -> Option<BlockKind> {
        let row = self.slash.row_data(focus.max(0) as usize)?;
        Some(kind_from_int(row.id))
    }

    // ---- block menu ----

    /// Fill the handle menu for one block. Paste appears only when the
    /// internal clipboard holds a block.
    pub fn fill_block_menu(&self) {
        let mut rows = vec![
            MenuRow {
                id: 1,
                label: "Move up".into(),
                icon: "chevron-up".into(),
                danger: false,
            },
            MenuRow {
                id: 2,
                label: "Move down".into(),
                icon: "chevron-down".into(),
                danger: false,
            },
            MenuRow {
                id: 3,
                label: "Duplicate".into(),
                icon: "copy".into(),
                danger: false,
            },
            MenuRow {
                id: 4,
                label: "Copy block".into(),
                icon: "copy".into(),
                danger: false,
            },
        ];
        if self.clipboard.borrow().is_some() {
            rows.push(MenuRow {
                id: 5,
                label: "Paste below".into(),
                icon: "import".into(),
                danger: false,
            });
        }
        rows.push(MenuRow {
            id: 6,
            label: "Delete".into(),
            icon: "trash".into(),
            danger: true,
        });
        self.block_menu.set_vec(rows);
    }

    pub fn copy_block(&self, id: i32) {
        if let Some(b) = self.doc.borrow().block(BlockId(id as u64)) {
            *self.clipboard.borrow_mut() = Some(b.clone());
        }
    }

    pub fn paste_below(&self, id: i32) -> bool {
        let clip = self.clipboard.borrow().clone();
        match clip {
            Some(c) => self
                .exec_on_open_page(Command::InsertBlockAfter {
                    id: BlockId(id as u64),
                    kind: c.kind,
                    text: c.text,
                })
                .is_some(),
            None => false,
        }
    }

    pub fn undo_open_page(&self) -> Option<Vec<Change>> {
        let page = core_page_id(self.open_page.get());
        let applied = crate::core::undo(
            &mut self.doc.borrow_mut(),
            &mut self.history.borrow_mut(),
            page,
        )?;
        self.record(applied.clone());
        self.reproject_blocks();
        Some(applied)
    }

    pub fn redo_open_page(&self) -> Option<Vec<Change>> {
        let page = core_page_id(self.open_page.get());
        let applied = crate::core::redo(
            &mut self.doc.borrow_mut(),
            &mut self.history.borrow_mut(),
            page,
        )?;
        self.record(applied.clone());
        self.reproject_blocks();
        Some(applied)
    }

    /// Open a page and return its title + breadcrumb for the top bar.
    pub fn open_page_info(&self, id: i32) -> (String, String) {
        let ws = self.workspace.borrow();
        let title = ws.title_of(id).unwrap_or("").to_string();
        let crumb = ws.breadcrumb(id);
        (title, crumb)
    }

    pub fn create_page(&self, parent: Option<i32>) -> i32 {
        let id = self.workspace.borrow_mut().create(parent, "Untitled");
        // the new page is the last sibling: order = previous last + 1
        let order = {
            let kids = self.workspace.borrow().children_of(parent);
            let map = self.page_order.borrow();
            let prev = kids
                .len()
                .checked_sub(2)
                .and_then(|i| kids.get(i))
                .and_then(|pid| map.get(pid).copied());
            OrderKey::between(prev, None).expect("append order exhausted")
        };
        self.page_order.borrow_mut().insert(id, order);
        self.record(vec![Change::PageCreated(crate::core::Page {
            id: PageId(id as u32 as u64),
            title: "Untitled".into(),
            parent: parent.map(|v| PageId(v as u32 as u64)),
            order,
            favorite: false,
            expanded: false,
        })]);
        self.open_page(id);
        id
    }

    pub fn rename_page(&self, id: i32, title: &str) {
        let title = title.trim();
        if title.is_empty() {
            self.rebuild_sidebar();
            return;
        }
        self.workspace.borrow_mut().rename(id, title);
        // searchable blob: refresh the title prefix in place
        let pid = core_page_id(id);
        let blob = {
            let doc = self.doc.borrow();
            let mut b = String::from(title);
            for block in doc.page_blocks(pid) {
                if !block.text.is_empty() {
                    b.push('\n');
                    b.push_str(&block.text);
                }
            }
            b
        };
        self.workspace.borrow_mut().set_search_text(id, blob);
        self.record(vec![Change::PageTitleSet {
            id: PageId(id as u32 as u64),
            title: title.to_string(),
        }]);
        self.rebuild_sidebar();
    }

    pub fn duplicate_page(&self, id: i32) -> Option<i32> {
        let new_id = self.workspace.borrow_mut().duplicate(id);
        if let Some(nid) = new_id {
            // copy the source page's blocks with fresh ids
            let src = core_page_id(id);
            let dst = core_page_id(nid);
            let copies: Vec<Block> = {
                let doc = self.doc.borrow();
                let start = doc.next_id_value();
                doc.page_blocks(src)
                    .iter()
                    .enumerate()
                    .map(|(i, b)| {
                        let mut c = b.clone();
                        c.id = BlockId(start + i as u64);
                        c.page = dst;
                        c
                    })
                    .collect()
            };
            let title = self.workspace.borrow().title_of(nid).unwrap().to_string();
            let blob = block_search_blob(&title, &project_blocks(&copies));

            // order: right after the original when a gap exists, else the
            // end of the sibling run (known drift: the copy may sort last
            // after a restart; the tree session view keeps it adjacent)
            let (parent, order) = {
                let ws = self.workspace.borrow();
                let parent = ws.get(nid).and_then(|p| p.parent);
                let kids = ws.children_of(parent);
                let next = kids
                    .iter()
                    .skip_while(|&&k| k != id)
                    .nth(1)
                    .and_then(|k| self.page_order.borrow().get(k).copied());
                let orig = self.page_order.borrow().get(&id).copied();
                let key = OrderKey::between(orig, next).or_else(|| {
                    let last = kids
                        .last()
                        .and_then(|k| self.page_order.borrow().get(k).copied());
                    OrderKey::between(last, None)
                });
                (parent, key.expect("order space exhausted"))
            };
            self.page_order.borrow_mut().insert(nid, order);

            let mut batch = vec![Change::PageCreated(crate::core::Page {
                id: PageId(nid as u32 as u64),
                title,
                parent: parent.map(|v| PageId(v as u32 as u64)),
                order,
                favorite: false,
                expanded: false,
            })];
            for b in &copies {
                batch.push(Change::BlockInserted(b.clone()));
            }
            self.doc.borrow_mut().set_page_blocks(dst, copies);
            self.workspace.borrow_mut().set_search_text(nid, blob);
            self.record(batch);
            self.rebuild_sidebar();
        }
        new_id
    }

    pub fn delete_page(&self, id: i32) -> bool {
        let removed = self.workspace.borrow_mut().delete(id);
        let had_open = removed.contains(&self.open_page.get());
        {
            let mut doc = self.doc.borrow_mut();
            for r in &removed {
                doc.drop_page(core_page_id(*r));
            }
        }
        self.record(vec![Change::PageDeleted {
            id: PageId(id as u32 as u64),
        }]);
        if had_open {
            // stale until the controller opens a fallback page
            self.open_page.set(0);
        }
        self.rebuild_sidebar();
        had_open
    }

    pub fn toggle_favorite(&self, id: i32) {
        self.workspace.borrow_mut().toggle_favorite(id);
        let favorite = self
            .workspace
            .borrow()
            .get(id)
            .map(|p| p.favorite)
            .unwrap_or(false);
        self.record(vec![Change::PageFavoriteSet {
            id: PageId(id as u32 as u64),
            favorite,
        }]);
        self.rebuild_sidebar();
    }

    pub fn toggle_expanded(&self, id: i32) {
        self.workspace.borrow_mut().toggle_expanded(id);
        let expanded = self
            .workspace
            .borrow()
            .get(id)
            .map(|p| p.expanded)
            .unwrap_or(false);
        self.record(vec![Change::PageExpandedSet {
            id: PageId(id as u32 as u64),
            expanded,
        }]);
        self.rebuild_sidebar();
    }

    /// Prepare the delete-confirmation dialog for `id`; the controller reads
    /// the text and shows the popup.
    pub fn delete_dialog_text(&self, id: i32) -> (String, String) {
        let ws = self.workspace.borrow();
        let title = ws.title_of(id).unwrap_or("").to_string();
        let n = ws.subtree_size(id);
        let message = if n > 1 {
            format!(
                "“{title}” and its {} sub-pages will be deleted. This cannot be undone.",
                n - 1
            )
        } else {
            format!("“{title}” will be deleted. This cannot be undone.")
        };
        self.pending_delete.set(Some(id));
        (title, message)
    }

    // ---- command palette ----

    pub fn set_query(&self, query: &str) {
        let filtered = if query.is_empty() {
            self.all_commands.clone()
        } else {
            self.all_commands
                .iter()
                .filter(|c| fuzzy_subsequence(query, &c.name))
                .cloned()
                .collect()
        };
        self.commands.set_vec(filtered);
    }

    // ---- search panel ----

    pub fn set_search_query(&self, query: &str) {
        if let Some(svc) = &self.search_service {
            if !query.trim().is_empty() {
                // hand the query to the worker thread; the controller polls
                // on a timer and the generation counter drops stale results
                let gen = self.next_search_generation();
                self.pending_search
                    .borrow_mut()
                    .replace((gen, svc.search_async(SearchRequest::new(query))));
                self.search.set_vec(Vec::new());
                return;
            }
            self.pending_search.borrow_mut().take();
        }
        let hits: Vec<SearchHit> = self.workspace.borrow().search(query);
        let rows: Vec<SearchRow> = hits
            .into_iter()
            .map(|h| SearchRow {
                page_id: h.id,
                title: h.title.into(),
                snippet: if h.snippet.is_empty() {
                    h.breadcrumb.into()
                } else {
                    h.snippet.into()
                },
            })
            .collect();
        self.search.set_vec(rows);
    }

    // ---- context menu ----

    /// Context-menu items for a page row (also used by the TopBar ⋯ menu).
    pub fn fill_menu(&self, id: i32) {
        let ws = self.workspace.borrow();
        let fav_label = if ws.get(id).map(|p| p.favorite).unwrap_or(false) {
            "Remove from favorites"
        } else {
            "Add to favorites"
        };
        let rows = vec![
            MenuRow {
                id: MENU_NEW_SUBPAGE,
                label: "New subpage".into(),
                icon: "plus".into(),
                danger: false,
            },
            MenuRow {
                id: MENU_RENAME,
                label: "Rename".into(),
                icon: "pencil".into(),
                danger: false,
            },
            MenuRow {
                id: MENU_DUPLICATE,
                label: "Duplicate".into(),
                icon: "copy".into(),
                danger: false,
            },
            MenuRow {
                id: MENU_FAVORITE,
                label: fav_label.into(),
                icon: "star".into(),
                danger: false,
            },
            MenuRow {
                id: MENU_DELETE,
                label: "Delete".into(),
                icon: "trash".into(),
                danger: true,
            },
        ];
        self.menu.set_vec(rows);
    }

    /// Benchmark scene F: the controller reads/writes the editor viewport-y
    /// property and uses this cell to detect "hit the bottom" (position
    /// stopped changing) so the scroll can wrap.
    pub fn last_scroll_y(&self) -> f32 {
        self.last_scroll_y.get()
    }
    pub fn set_last_scroll_y(&self, v: f32) {
        self.last_scroll_y.set(v);
    }
}

// Sample page ids (stable, used by scenes/tests).
pub const PAGE_WEEKLY_REVIEW: i32 = 100;
pub const PAGE_GETTING_STARTED: i32 = 102;
pub const PAGE_ATLAS: i32 = 105;
pub const PAGE_CHINESE: i32 = 112;
pub const PAGE_SCRATCHPAD: i32 = 113;

pub const ROW_NEW_PAGE: i32 = -2;

// Context-menu action ids.
pub const MENU_NEW_SUBPAGE: i32 = 1;
pub const MENU_RENAME: i32 = 2;
pub const MENU_DUPLICATE: i32 = 3;
pub const MENU_FAVORITE: i32 = 4;
pub const MENU_DELETE: i32 = 5;

fn header(label: &str) -> SidebarNode {
    SidebarNode {
        id: -1,
        label: label.into(),
        kind: "header".into(),
        depth: 0,
        expanded: false,
        has_children: false,
        selected: false,
        y: 0,
    }
}

fn leaf_row(id: i32, label: &str, kind: &str, selected: bool) -> SidebarNode {
    SidebarNode {
        id,
        label: label.into(),
        kind: kind.into(),
        depth: 0,
        expanded: false,
        has_children: false,
        selected,
        y: 0,
    }
}

// ---- block content ----

// ---- core <-> projection bridge ----

/// M2 mock page ids (i32) map into the u64 core id space unchanged.
pub fn core_page_id(id: i32) -> PageId {
    PageId(id as u32 as u64)
}

/// Slash-menu descriptors: Rust owns the list (SPEC §十五), the UI only
/// renders labels. ids are BlockKind ints (see kind_from_int).
const SLASH_ITEMS: &[(BlockKind, &str, &str)] = &[
    (BlockKind::Paragraph, "Text", "Plain paragraph"),
    (BlockKind::Heading1, "Heading 1", "Large section heading"),
    (BlockKind::Heading2, "Heading 2", "Medium section heading"),
    (BlockKind::Heading3, "Heading 3", "Small section heading"),
    (BlockKind::Bullet, "Bullet list", "Simple bulleted list"),
    (BlockKind::Numbered, "Numbered list", "List with numbering"),
    (BlockKind::Todo, "To-do list", "Track tasks with checkboxes"),
    (BlockKind::Quote, "Quote", "Capture a quotation"),
    (BlockKind::Code, "Code", "Monospaced code block"),
    (BlockKind::Divider, "Divider", "Visual separator"),
];

fn slash_items(filter: &str) -> Vec<SlashRow> {
    let needle = filter.to_lowercase();
    SLASH_ITEMS
        .iter()
        .filter(|(_, label, _)| needle.is_empty() || label.to_lowercase().contains(&needle))
        .map(|(kind, label, hint)| SlashRow {
            id: kind_to_int(*kind),
            label: (*label).into(),
            hint: (*hint).into(),
        })
        .collect()
}

fn kind_from_int(kind: i32) -> BlockKind {
    match kind {
        1 => BlockKind::Heading1,
        2 => BlockKind::Heading2,
        3 => BlockKind::Heading3,
        4 => BlockKind::Bullet,
        5 => BlockKind::Numbered,
        6 => BlockKind::Todo,
        7 => BlockKind::Quote,
        8 => BlockKind::Code,
        9 => BlockKind::Divider,
        _ => BlockKind::Paragraph,
    }
}

fn kind_to_int(kind: BlockKind) -> i32 {
    match kind {
        BlockKind::Heading1 => 1,
        BlockKind::Heading2 => 2,
        BlockKind::Heading3 => 3,
        BlockKind::Bullet => 4,
        BlockKind::Numbered => 5,
        BlockKind::Todo => 6,
        BlockKind::Quote => 7,
        BlockKind::Code => 8,
        BlockKind::Divider => 9,
        BlockKind::Paragraph => 0,
    }
}

fn runs_to_model(b: &Block) -> slint::ModelRc<TextRun> {
    slint::ModelRc::from(Rc::new(slint::VecModel::from(build_runs(
        &b.text, &b.marks,
    ))))
}

/// Convert mock template rows into core Blocks with fresh ids and order
/// keys (order = the row order).
fn rows_to_blocks(page: i32, rows: Vec<BlockRow>, doc: &mut Document) -> Vec<Block> {
    let pid = core_page_id(page);
    let mut prev: Option<OrderKey> = None;
    rows.into_iter()
        .map(|row| {
            let order = OrderKey::between(prev, None).expect("order space exhausted");
            prev = Some(order);
            Block {
                id: doc.alloc_block_id(),
                page: pid,
                parent: None,
                order,
                kind: kind_from_int(row.kind),
                text: row.text.to_string(),
                checked: row.checked,
                marks: Vec::new(),
            }
        })
        .collect()
}

/// Split text into per-mark runs. Runs are single-line rendered (Slint Text
/// has no inline rich formatting — documented limitation, see
/// docs/EDITOR_ARCHITECTURE.md).
fn build_runs(text: &str, marks: &[crate::core::Mark]) -> Vec<TextRun> {
    if marks.is_empty() || text.is_empty() {
        return Vec::new();
    }
    let len = text.len();
    let mut bounds: Vec<usize> = vec![0, len];
    for m in marks {
        for v in [m.start.min(len), m.end.min(len)] {
            if text.is_char_boundary(v) {
                bounds.push(v);
            }
        }
    }
    bounds.sort_unstable();
    bounds.dedup();
    bounds
        .windows(2)
        .filter_map(|w| {
            let (s, e) = (w[0], w[1]);
            if s == e {
                return None;
            }
            let link_mark = marks
                .iter()
                .find(|m| m.kind == crate::core::MarkKind::Link && m.start <= s && m.end >= e);
            Some(TextRun {
                text: text[s..e].into(),
                bold: marks
                    .iter()
                    .any(|m| m.kind == crate::core::MarkKind::Bold && m.start <= s && m.end >= e),
                italic: marks
                    .iter()
                    .any(|m| m.kind == crate::core::MarkKind::Italic && m.start <= s && m.end >= e),
                strike: marks
                    .iter()
                    .any(|m| m.kind == crate::core::MarkKind::Strike && m.start <= s && m.end >= e),
                code: marks
                    .iter()
                    .any(|m| m.kind == crate::core::MarkKind::Code && m.start <= s && m.end >= e),
                link: link_mark.is_some(),
                url: link_mark.map(|m| m.url.clone()).unwrap_or_default().into(),
            })
        })
        .collect()
}

/// Project a page's blocks into editor rows: numbered items renumbered by
/// position, the last row flagged as the tail spacer carrier.
pub fn project_blocks(blocks: &[Block]) -> Vec<BlockRow> {
    let mut out: Vec<BlockRow> = blocks
        .iter()
        .map(|b| BlockRow {
            id: b.id.0 as i32,
            kind: kind_to_int(b.kind),
            text: b.text.clone().into(),
            checked: b.checked,
            number: 0,
            tail: false,
            runs: runs_to_model(b),
        })
        .collect();
    let mut n = 0;
    for r in &mut out {
        if r.kind == BLOCK_NUMBERED {
            n += 1;
            r.number = n;
        }
    }
    if let Some(last) = out.last_mut() {
        last.tail = true;
    }
    out
}

pub const BLOCK_PARAGRAPH: i32 = 0;
pub const BLOCK_H1: i32 = 1;
pub const BLOCK_H2: i32 = 2;
pub const BLOCK_H3: i32 = 3;
pub const BLOCK_BULLET: i32 = 4;
pub const BLOCK_NUMBERED: i32 = 5;
pub const BLOCK_TODO: i32 = 6;
pub const BLOCK_QUOTE: i32 = 7;
pub const BLOCK_CODE: i32 = 8;
pub const BLOCK_DIVIDER: i32 = 9;

fn block(kind: i32, text: &str) -> BlockRow {
    BlockRow {
        id: 0,
        kind,
        text: text.into(),
        checked: false,
        number: 0,
        tail: false,
        runs: ModelRc::from(Rc::new(VecModel::from(Vec::new()))),
    }
}

/// Flag the last row so its delegate renders the page's bottom spacer
/// (a ListView allows a single `for`, so chrome must live inside a row).
fn with_tail(mut b: Vec<BlockRow>) -> Vec<BlockRow> {
    if let Some(last) = b.last_mut() {
        last.tail = true;
    }
    b
}

fn number_renumber(b: &mut [BlockRow]) {
    let mut n = 0;
    for r in b.iter_mut() {
        if r.kind == BLOCK_NUMBERED {
            n += 1;
            r.number = n;
        }
    }
}

fn mock_blocks_for_page(id: i32, title: &str) -> Vec<BlockRow> {
    match id {
        PAGE_GETTING_STARTED => mock_blocks_sample(),
        PAGE_CHINESE => mock_blocks_chinese(),
        PAGE_SCRATCHPAD => Vec::new(),
        _ => mock_blocks_generic(title),
    }
}

fn mock_blocks_sample() -> Vec<BlockRow> {
    let mut b = vec![
        block(
            BLOCK_PARAGRAPH,
            "A quiet home for thinking. This workspace collects notes, plans, and references for the Atlas project — and doubles as the visual test document for Quire itself.",
        ),
        block(BLOCK_DIVIDER, ""),
        block(BLOCK_H2, "Why a local-first editor"),
        block(
            BLOCK_PARAGRAPH,
            "Cloud apps are great until the laptop fan sounds like a jet engine. Quire keeps documents in a local SQLite database, renders with the GPU, and stays out of the way.",
        ),
        block(BLOCK_QUOTE, "Simplicity is the ultimate sophistication — but performance is the ultimate courtesy."),
        block(BLOCK_H3, "Principles"),
        block(BLOCK_BULLET, "One process, one document model, no hidden servers"),
        block(BLOCK_BULLET, "Only the focused block owns a real text cursor"),
        block(BLOCK_BULLET, "Nothing animates unless the user asks for it"),
        block(BLOCK_NUMBERED, "Write instantly, even on a five-year-old laptop"),
        block(BLOCK_NUMBERED, "Scroll a 10 000-block page without hitching"),
        block(BLOCK_NUMBERED, "Close the lid, reopen, and everything is there"),
        block(BLOCK_TODO, "Block editor MVP"),
        block(BLOCK_TODO, "Slash menu (type \"/\" anywhere)"),
        block(BLOCK_H2, "运行与中文"),
        block(
            BLOCK_PARAGRAPH,
            "中文段落用于验证字体回退与行高：排版本应稳定，不出现字符裁剪；标点悬挂与换行位置符合预期。",
        ),
        block(BLOCK_CODE, "cargo run --release  # 140 ms to first paint, hopefully"),
        block(
            BLOCK_PARAGRAPH,
            "Start typing, or press Ctrl+K to open the command palette.",
        ),
    ];
    for (i, row) in b.iter_mut().enumerate() {
        row.id = i as i32;
    }
    number_renumber(&mut b);
    with_tail(b)
}

fn mock_blocks_chinese() -> Vec<BlockRow> {
    let mut b = vec![
        block(BLOCK_H2, "写作与中文测试"),
        block(
            BLOCK_PARAGRAPH,
            "中文段落用于验证字体回退与行高：排版本应稳定，不出现字符裁剪；标点悬挂与换行位置符合预期。",
        ),
        block(
            BLOCK_PARAGRAPH,
            "在长段落中混排 English words 与数字（如 2026 年 9 月）时，基线应保持一致，中西文之间留有恰当的间隙。",
        ),
        block(BLOCK_TODO, "检查行高在 125% 缩放下是否稳定"),
        block(BLOCK_TODO, "检查标点挤压与引号方向"),
        block(BLOCK_QUOTE, "好的排版是看不见的 —— 读者只注意到内容本身。"),
        block(BLOCK_CODE, "cargo run --release --features skia"),
    ];
    for (i, row) in b.iter_mut().enumerate() {
        row.id = i as i32;
    }
    with_tail(b)
}

/// Placeholder content for regular pages. No H1: the editor header already
/// renders the page title.
fn mock_blocks_generic(_title: &str) -> Vec<BlockRow> {
    let mut b = vec![
        block(
            BLOCK_PARAGRAPH,
            "This is a mock page for the navigation milestone: the tree, search, and menus are live; the text is placeholder until the block editor arrives.",
        ),
        block(BLOCK_DIVIDER, ""),
        block(
            BLOCK_PARAGRAPH,
            "Use the sidebar to create, rename, duplicate, and delete pages — changes live in memory for now; SQLite persistence lands with the next milestone.",
        ),
        block(BLOCK_BULLET, "Ctrl+P searches every page, titles and content"),
        block(BLOCK_BULLET, "Ctrl+K opens the command palette"),
        block(BLOCK_BULLET, "Right-click a page for its context menu"),
    ];
    for (i, row) in b.iter_mut().enumerate() {
        row.id = i as i32;
    }
    with_tail(b)
}

/// Small page body for scene G's bench pages.
fn mock_blocks_bench_page(title: &str) -> Vec<BlockRow> {
    let mut b = vec![
        block(
            BLOCK_PARAGRAPH,
            "Benchmark fixture page. Switching between these pages exercises model swap + delegate rebuild.",
        ),
        block(BLOCK_H2, title),
        block(BLOCK_PARAGRAPH, "The quick brown fox jumps over the lazy dog."),
        block(BLOCK_BULLET, "Pack my box with five dozen liquor jugs."),
        block(BLOCK_TODO, "Sphinx of black quartz, judge my vow."),
    ];
    for (i, row) in b.iter_mut().enumerate() {
        row.id = i as i32;
    }
    with_tail(b)
}

fn mock_blocks_bench(count: usize) -> Vec<BlockRow> {
    let lorem = [
        "The quick brown fox jumps over the lazy dog.",
        "Pack my box with five dozen liquor jugs, then verify rendering.",
        "中文基准段落：用于长文档下的内存与滚动性能测试。",
        "Sphinx of black quartz, judge my vow — line after line after line.",
    ];
    with_tail(
        (0..count)
            .map(|i| {
                let kind = match i % 20 {
                    0 => BLOCK_H2,
                    5 | 6 => BLOCK_BULLET,
                    9 => BLOCK_TODO,
                    13 => BLOCK_QUOTE,
                    _ => BLOCK_PARAGRAPH,
                };
                let mut row = block(kind, lorem[i % lorem.len()]);
                row.id = i as i32;
                row
            })
            .collect(),
    )
}

/// Title + block texts, the blob the search scans.
fn block_search_blob(title: &str, blocks: &[BlockRow]) -> String {
    let mut blob = String::from(title);
    for b in blocks {
        if !b.text.is_empty() {
            blob.push('\n');
            blob.push_str(&b.text);
        }
    }
    blob
}

// ---- command palette ----

const CMD_NEW_PAGE: i32 = 1;
const CMD_SEARCH: i32 = 2;
const CMD_TOGGLE_SIDEBAR: i32 = 3;
const CMD_TOGGLE_THEME: i32 = 4;
const CMD_SETTINGS: i32 = 5;
pub const CMD_RENAME_PAGE: i32 = 6;
pub const CMD_DUPLICATE_PAGE: i32 = 7;
pub const CMD_DELETE_PAGE: i32 = 8;
pub const CMD_EXPORT_PAGE: i32 = 9;
pub const CMD_IMPORT_MD: i32 = 10;
/// Jump-to-page commands are 10 000 + page id.
pub const CMD_PAGE_BASE: i32 = 10_000;

fn mock_commands(ws: &Workspace) -> Vec<CommandRow> {
    let mut v = Vec::new();
    let mut cmd = |id: i32, name: &str, hint: &str, section: &str, icon: &str| {
        v.push(CommandRow {
            id,
            name: name.into(),
            hint: hint.into(),
            section: section.into(),
            icon: icon.into(),
        })
    };
    cmd(CMD_NEW_PAGE, "New Page", "Ctrl+N", "Editor", "plus");
    cmd(CMD_SEARCH, "Search Pages…", "Ctrl+P", "Navigate", "search");
    cmd(
        CMD_TOGGLE_SIDEBAR,
        "Toggle Sidebar",
        "Ctrl+B",
        "Interface",
        "panel-left",
    );
    cmd(
        CMD_TOGGLE_THEME,
        "Toggle Dark Mode",
        "Ctrl+Shift+L",
        "Interface",
        "moon",
    );
    cmd(CMD_SETTINGS, "Settings", "", "Navigate", "settings");
    cmd(CMD_RENAME_PAGE, "Rename Page", "F2", "Page", "pencil");
    cmd(CMD_DUPLICATE_PAGE, "Duplicate Page", "", "Page", "copy");
    cmd(CMD_DELETE_PAGE, "Delete Page", "", "Page", "trash");
    cmd(
        CMD_EXPORT_PAGE,
        "Export Page as Markdown…",
        "",
        "Page",
        "export",
    );
    cmd(CMD_IMPORT_MD, "Import Markdown…", "", "Page", "import");
    for id in ws.dfs_order() {
        if id >= BENCH_ID_BASE {
            continue;
        }
        if let Some(title) = ws.title_of(id) {
            cmd(CMD_PAGE_BASE + id, title, "", "Jump to page", "page");
        }
    }
    v
}

/// Cheap subsequence fuzzy match, case-insensitive, ASCII-only scoring.
fn fuzzy_subsequence(query: &str, target: &str) -> bool {
    let target = target.to_lowercase();
    let mut it = target.chars();
    query
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .all(|q| it.any(|t| t == q))
}
