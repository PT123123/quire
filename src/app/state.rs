// Application state + mock content for M2/M4.
//
// The page tree lives in `workspace.rs` (pure, unit-tested); the block
// content of every page lives in `core::Document` since M4 (the editing
// truth, mutated only through commands). This module is the view-
// projection layer between the two and the Slint models (SidebarNode /
// BlockRow / CommandRow / SearchRow / MenuRow).

use crate::app::workspace::{SearchHit, Workspace, BENCH_ID_BASE, MAX_RECENTS};
use crate::core::persistence::{Change, Repository};
use crate::core::{
    Attachment, AttachmentId, Block, BlockId, BlockKind, ColorKind, Command, Document, History,
    OrderKey, PageId,
};
use crate::services::find_service::FindSession;
use crate::services::persistence::PersistenceService;
use crate::services::search_service::SearchService;
use crate::storage::search_index::SearchRequest;
use crate::storage::SqliteRepository;
use crate::{BlockRow, ColumnBox, ColumnItem, TableCell, CommandRow, MenuRow, SearchRow, SidebarNode, SlashRow, TextRun, TocEntry};
use slint::{Model, ModelRc, VecModel};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
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
    /// The database this session runs on (`None` = headless or memory-only).
    /// The settings dialog's storage row and "Back up now" ride it.
    pub repo: Option<Arc<crate::storage::SqliteRepository>>,
    /// FTS-backed search (M7). `None` falls back to the in-memory scan.
    pub search_service: Option<Arc<SearchService>>,
    /// Where attachments live (SPEC §三十七 批次 A). Beside the database, or
    /// in the temp folder for a headless session with no database at all.
    pub store: crate::services::attachment_store::AttachmentStore,
    /// Attachment rows by id, loaded at startup and appended on import.
    pub attachments: RefCell<BTreeMap<i64, Attachment>>,
    /// Decoded rasters by attachment id. Populated only for a row the ListView
    /// realizes, and capped by `MAX_ATTACHMENT_CACHE_BYTES` — Slint's own
    /// image cache is keyed by path and holds 5 MB, which is one photograph,
    /// so scrolling a photo page through it re-decodes on every frame.
    attachment_images: RefCell<BTreeMap<i64, CachedImage>>,
    attachment_cache_bytes: Cell<usize>,
    /// Highest the cache has ever been this session — what the bench harness
    /// prints, because a ceiling nobody reads is only a promise.
    attachment_cache_peak: Cell<usize>,
    attachment_tick: Cell<u64>,
    /// Next attachment id: one past the highest row this session loaded.
    next_attachment_id: Cell<i64>,
    /// In-flight async search with its generation; superseded queries drop
    /// their result instead of overwriting newer ones.
    pending_search: RefCell<Option<(u64, crate::services::search_service::PendingSearch)>>,
    /// Ctrl+F in-page find session (Track B's FindSession).
    find_session: RefCell<Option<FindSession>>,
    find_label: RefCell<String>,

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
    /// Startup notices (abort banner, backup restore, library move) — more
    /// than one can queue; the controller drains them as one line and shows
    /// it once in the shell.
    db_notice: RefCell<Vec<String>>,
    /// Currently open page (0 = none / empty workspace).
    pub open_page: Cell<i32>,
    /// Go Back / Go Forward stacks (SPEC §十六). Session-only: a restart
    /// starts with no history.
    nav: RefCell<NavHistory>,
    /// Page awaiting delete confirmation.
    pub pending_delete: Cell<Option<i32>>,
    /// Benchmark scroll bookkeeping (scene F): last viewport-y seen.
    pub last_scroll_y: Cell<f32>,
}

/// One decoded picture plus what it costs to keep it decoded.
#[derive(Clone)]
struct CachedImage {
    /// LRU stamp: the tick of the last projection that asked for this raster.
    used: u64,
    /// Decoded bytes (RGBA), which is what the budget below is spent against.
    bytes: usize,
    image: slint::Image,
}

/// Ceiling on the decoded rasters this session holds. A 1280x720 RGBA frame is
/// 3.7 MB, so this is roughly eight pictures — a screenful with room to scroll
/// back one page without re-decoding. Without a ceiling the map only ever
/// grows, and §二十二's low-RAM promise dies the first time someone pastes a
/// hundred screenshots.
const MAX_ATTACHMENT_CACHE_BYTES: usize = 32 * 1024 * 1024;

/// Go Back / Go Forward stacks (SPEC §十六), newest entry last. Kept as a
/// plain struct with no Slint or database in it so the stepping rules —
/// which are the whole feature — are unit-testable.
#[derive(Default)]
struct NavHistory {
    back: Vec<i32>,
    forward: Vec<i32>,
}

/// How far back Go Back reaches before it starts dropping entries.
const NAV_MAX: usize = 50;

impl NavHistory {
    /// A user-initiated move from one page to another. A new navigation
    /// drops the forward branch, the way a browser does.
    fn record(&mut self, from: i32, to: i32) {
        if from > 0 && from != to {
            self.back.push(from);
            if self.back.len() > NAV_MAX {
                self.back.remove(0);
            }
        }
        self.forward.clear();
    }

    /// Step one page back (or forward), skipping pages deleted since they
    /// were recorded, and push `current` onto the opposite stack. `live`
    /// answers "does this page still exist?".
    fn step(&mut self, forward: bool, current: i32, live: impl Fn(i32) -> bool) -> Option<i32> {
        let (from, to) = if forward {
            (&mut self.forward, &mut self.back)
        } else {
            (&mut self.back, &mut self.forward)
        };
        let target = loop {
            match from.pop() {
                Some(id) if live(id) => break id,
                Some(_) => continue,
                None => return None,
            }
        };
        if current > 0 && live(current) && to.last() != Some(&current) {
            to.push(current);
        }
        Some(target)
    }
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
    /// Scene D/F with media: how many of the bench page's rows are pictures
    /// (SPEC §三十七's gate for a page of images being scrolled).
    pub pictures: usize,
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
            let mut core_blocks = rows_to_blocks(PAGE_ATLAS, rows, &mut doc);
            bench_pictures(&mut core_blocks, args.pictures);
            doc.set_page_blocks(core_page_id(PAGE_ATLAS), core_blocks);
            let _ = title;
        }

        // with_database_snapshots powers the periodic snapshots (M8, D10):
        // they ride the flush tick with a 10-minute minimum interval
        let persistence = repo.clone().map(|r| {
            Arc::new(
                PersistenceService::with_default_clock(r.clone())
                    .with_database_snapshots(&r),
            )
        });

        // M8_FEEDBACK #9: the panic logger runs before any repository
        // exists, so its facts land in <data dir>/session.meta. This is the
        // first place that holds both sides: copy the entries into the
        // metadata table (one transaction), then consume them from the
        // session file — session.meta is a handoff note to exactly this
        // next session, so anything left would be re-reported forever. An
        // abort summary also becomes the startup notice bar's first line.
        let mut abort_notice: Option<String> = None;
        if let (Some(r), Some(logger)) = (&repo, crate::services::logging::current()) {
            let entries = logger.meta_entries();
            if !entries.is_empty() {
                let mut changes: Vec<Change> = entries
                    .iter()
                    .map(|(k, v)| Change::MetaSet {
                        key: k.clone(),
                        value: v.clone(),
                    })
                    .collect();
                changes.extend(entries.iter().map(|(k, _)| Change::MetaDelete {
                    key: k.clone(),
                }));
                let _ = r.apply(&changes);
                if let Some((_, summary)) = entries.iter().find(|(k, _)| {
                    k == crate::services::logging::KEY_SESSION_ABORTED
                }) {
                    let first_line = summary.lines().next().unwrap_or(summary);
                    abort_notice = Some(format!(
                        "the previous session ended unexpectedly: {first_line}"
                    ));
                }
            }
        }

        let repo_for_state = repo.clone();
        let search_service = repo.map(crate::services::search_service::SearchService::new_arc);

        // Attachments ride the same database. Their rows are read once here —
        // a small table of metadata, not pixels — so painting a realized
        // image row is a map lookup plus (first time only) one decode. A
        // library that predates v7 has no rows at all. A failed read costs
        // the session its pictures, not its documents.
        let store = crate::services::attachment_store::AttachmentStore::for_db(
            repo_for_state.as_ref().and_then(|r| r.path()),
        );
        let mut attachments: BTreeMap<i64, Attachment> = BTreeMap::new();
        let mut next_attachment_id: i64 = 1;
        let mut attachment_notice: Option<String> = None;
        if let Some(r) = &repo_for_state {
            match r.load_attachments() {
                Ok(rows) => {
                    for a in rows {
                        let key = a.id.as_u64() as i64;
                        next_attachment_id = next_attachment_id.max(key + 1);
                        attachments.insert(key, a);
                    }
                }
                Err(e) => {
                    attachment_notice =
                        Some(format!("attachments could not be loaded: {e}"));
                }
            }
        }

        // The media scene's fixtures (SPEC §三十七's "a page of pictures being
        // scrolled"): write the pool the bench page points at, on a fresh
        // library only. The measured pass of a bench run then loads pictures
        // instead of paying for them twice, and `--pictures` with nothing to
        // persist costs the shot tool no files.
        let media_scene = args.pictures > 0 && args.blocks > 0;
        if media_scene && persisted.is_none() && repo_for_state.is_some() {
            let (_, pool) = bench_picture_plan(args.blocks, args.pictures);
            for k in 1..=pool as i64 {
                if let Some(att) = store.create_fixture(AttachmentId(k as u64), 1280, 720) {
                    attachments.insert(k, att);
                    next_attachment_id = next_attachment_id.max(k + 1);
                }
            }
        }

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
            // rows before the blocks that point at them
            for a in attachments.values() {
                batch.push(Change::AttachmentAdded(a.clone()));
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
            // reopen the page the last session had open (the "current-page"
            // meta every open_page writes), unless it no longer exists
            restored_current
                .filter(|id| workspace.contains(*id))
                .unwrap_or(PAGE_GETTING_STARTED)
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
            ui: RefCell::new(None),
            db_notice: RefCell::new(
                abort_notice.into_iter().chain(attachment_notice).collect(),
            ),
            all_commands,
            doc: RefCell::new(doc),
            history: RefCell::new(History::default()),
            persistence,
            repo: repo_for_state,
            search_service,
            store,
            attachments: RefCell::new(attachments),
            attachment_images: RefCell::new(BTreeMap::new()),
            attachment_cache_bytes: Cell::new(0),
            attachment_cache_peak: Cell::new(0),
            attachment_tick: Cell::new(0),
            next_attachment_id: Cell::new(next_attachment_id),
            pending_search: RefCell::new(None),
            search_generation: Cell::new(0),
            find_session: RefCell::new(None),
            find_label: RefCell::new(String::new()),
            page_order: RefCell::new(page_order),
            flush_hook: RefCell::new(None),
            open_page: Cell::new(0),
            nav: RefCell::new(NavHistory::default()),
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
        let empty = self.workspace.borrow().page_count() == 0;
        if let Some(ui) = self.ui.borrow().clone() {
            ui.upgrade().unwrap().set_workspace_empty(empty);
        }
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

        push(
            &mut rows,
            &mut y,
            SidebarNode {
                id: WORKSPACE_HEADER_ID,
                label: "Workspace".into(),
                kind: "header".into(),
                depth: 0,
                expanded: false,
                has_children: false,
                selected: false,
                y: 0,
            },
        );
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

    /// Record that the user is moving from the page they had open to another
    /// one, so Go Back can retrace it (SPEC §十六).
    pub fn nav_record(&self, from: i32, to: i32) {
        self.nav.borrow_mut().record(from, to);
    }

    /// The page to navigate to, stepping back (or forward) through the
    /// session history; `None` when that direction is empty. `open_page` is
    /// still the page being left when this is called, which is what makes it
    /// the entry pushed onto the opposite stack.
    pub fn nav_step(&self, forward: bool) -> Option<i32> {
        let current = self.open_page.get();
        self.nav
            .borrow_mut()
            .step(forward, current, |id| {
                self.workspace.borrow().contains(id)
            })
    }

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
        // a fresh page starts at the top (the old viewport offset would
        // otherwise leak across pages)
        if let Some(ui) = self.ui.borrow().clone() {
            ui.upgrade()
                .unwrap()
                .set_editor_scroll_y(0.0);
        }
        self.rebuild_sidebar();
    }

    /// Rebuild the editor rows from the Document (page switch, undo/redo,
    /// structural edits). Typing never goes through here.
    pub fn reproject_blocks(&self) {
        let page = self.open_page.get();
        let mut rows = {
            let doc = self.doc.borrow();
            project_blocks(doc.page_blocks(core_page_id(page)))
        };
        // a Page or Link block shows the target page's live title, not stale
        // text; a reference whose page is gone reads as deleted
        let ws = self.workspace.borrow();
        for row in rows
            .iter_mut()
            .filter(|r| r.kind == BLOCK_PAGE || r.kind == BLOCK_LINK)
        {
            row.text = ws.title_of(row.page_ref).unwrap_or("(deleted page)").into();
        }
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
                // An attachment's text is the file name: on a picture row it
                // is invisible metadata, on a file row it is a label rather
                // than prose. Neither belongs in the word count.
                if matches!(
                    b.kind,
                    crate::core::BlockKind::Image | crate::core::BlockKind::File
                ) {
                    continue;
                }
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
                        hit_block_id: h.block.map(|b| b.0 as i32).unwrap_or(-1),
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
                            hit_block_id: h.block.map(|b| b.0 as i32).unwrap_or(-1),
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
        self.db_notice.borrow_mut().push(notice);
    }

    /// Drain everything queued as one line. Startup can stack several facts
    /// (the previous session aborted, a backup was restored, the library
    /// moved); they read better joined than overwriting each other.
    pub fn take_db_notice(&self) -> Option<String> {
        let queue = self.db_notice.borrow_mut();
        if queue.is_empty() {
            return None;
        }
        let mut line = queue.join("; ");
        if !line.ends_with('.') {
            line.push('.');
        }
        Some(line)
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

    /// Translate a drag landing from the editor row the pointer is over into
    /// the position `MoveBlockTo` counts in. The two agree while nothing is
    /// folded; a collapsed subtree takes its rows out from under the count.
    pub fn drop_index_for_row(&self, row: i32, below: bool) -> Option<i32> {
        let doc = self.doc.borrow();
        let blocks = doc.page_blocks(core_page_id(self.open_page.get()));
        let model = *visible_block_indices(blocks).get(usize::try_from(row).ok()?)?;
        Some((model + below as usize) as i32)
    }

    /// Read-only check whether a drag landing is valid (hover feedback must
    /// not mutate the document).
    pub fn can_move_block_to(&self, id: i32, index: i32) -> bool {
        let page = core_page_id(self.open_page.get());
        crate::core::command::can_move_block_to(
            &self.doc.borrow(),
            page,
            BlockId(id as u64),
            index,
        )
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

    /// Persisted window size (physical px), if a previous session saved one.
    pub fn window_size_setting(&self) -> Option<(f64, f64)> {
        let map = self.settings.borrow();
        let (Some(w), Some(h)) = (map.get("window.w"), map.get("window.h")) else {
            return None;
        };
        match (w.parse::<f64>(), h.parse::<f64>()) {
            (Ok(w), Ok(h)) if w >= 400.0 && h >= 300.0 => Some((w, h)),
            _ => None,
        }
    }

    /// Record the window size as settings changes (flushed with the batch).
    pub fn record_window_size(&self, w: f64, h: f64) {
        self.settings
            .borrow_mut()
            .insert("window.w".into(), format!("{w}"));
        self.settings
            .borrow_mut()
            .insert("window.h".into(), format!("{h}"));
        self.record(vec![
            Change::SettingSet { key: "window.w".into(), value: format!("{w}") },
            Change::SettingSet { key: "window.h".into(), value: format!("{h}") },
        ]);
    }

    /// Read a persisted settings flag (value "1"/"0").
    pub fn setting_flag(&self, key: &str) -> bool {
        self.settings.borrow().get(key).map(|v| v == "1").unwrap_or(false)
    }

    /// Persist a settings flag (one batch, flushed with the session).
    pub fn record_setting(&self, key: &str, value: &str) {
        self.settings
            .borrow_mut()
            .insert(key.into(), value.into());
        self.record(vec![Change::SettingSet {
            key: key.into(),
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
                disabled: false,
            })
            .collect();
        self.slash.set_vec(rows);
    }

    /// Fill the "+"-handle insert menu, filtered by `filter` (the block's
    /// whole line in insert mode). Includes the disabled later-milestone
    /// placeholders — see INSERT_ITEMS.
    pub fn open_slash_insert(&self, filter: &str) {
        let needle = filter.to_lowercase();
        let rows: Vec<SlashRow> = INSERT_ITEMS
            .iter()
            .filter(|(_, label, _)| needle.is_empty() || label.to_lowercase().contains(&needle))
            .map(|(id, label, hint)| SlashRow {
                id: *id,
                label: (*label).into(),
                hint: (*hint).into(),
                disabled: *id < 0,
            })
            .collect();
        self.slash.set_vec(rows);
    }

    pub fn slash_focus_count(&self) -> i32 {
        self.slash.row_count() as i32
    }

    /// Arrow-key navigation for the slash/insert popup: clamps like before,
    /// but skips the not-selectable placeholder rows.
    pub fn slash_next_focus(&self, current: i32, delta: i32) -> i32 {
        let count = self.slash.row_count() as i32;
        if count == 0 {
            return 0;
        }
        let disabled =
            |i: i32| self.slash.row_data(i as usize).map(|r| r.id < 0).unwrap_or(true);
        let mut next = (current + delta).clamp(0, count - 1);
        if delta != 0 {
            let step = delta.signum();
            let mut guard = 0;
            while disabled(next) && guard < count {
                let candidate = next + step;
                if candidate < 0 || candidate >= count {
                    break;
                }
                next = candidate;
                guard += 1;
            }
        }
        next
    }

    pub fn slash_selected_kind(&self, focus: i32) -> Option<BlockKind> {
        let row = self.slash.row_data(focus.max(0) as usize)?;
        if row.id < 0 {
            // placeholder row (later-milestone kind): nothing to apply
            return None;
        }
        Some(kind_from_int(row.id))
    }

    // ---- block menu ----

    /// Action-id ranges of the ⋮⋮ menu (see `fill_block_menu`).
    pub const MOVE_TO_BASE: i32 = 100_000;
    pub const COLOR_TEXT_BASE: i32 = 200_000;
    pub const COLOR_BG_BASE: i32 = 300_000;
    /// Plus the display width in percent, so the id carries the pick.
    pub const IMAGE_WIDTH_BASE: i32 = 500_000;


    /// Fill the handle menu for one block — Notion's ⋮⋮ set, minus the
    /// collab/AI items that are v1 out of scope, plus the move/copy
    /// affordances earlier milestones added. Paste appears only when the
    /// internal clipboard holds a block. Submenus swap the rows and keep
    /// the popup open; the controller routes action ids:
    ///   1..8    root actions + Turn into (7) + Back (8)
    ///   9..13   copy link / Move to / Text color / Background color opens,
    ///           and Image width (13, pictures only)
    ///   100+k   Turn-into target kinds
    ///   MOVE_TO_BASE+pid / COLOR_TEXT_BASE+slot / COLOR_BG_BASE+slot
    ///   IMAGE_WIDTH_BASE+percent
    pub fn fill_block_menu(&self, id: i32) {
        let mut rows = vec![
            row(7, "Turn into", "chevron-right", false, -1, false),
            row(3, "Duplicate", "copy", false, -1, false),
            row(9, "Copy link to block", "link", false, -1, false),
            row(10, "Move to", "arrow-right", false, -1, false),
            row(11, "Text color", "palette", false, -1, false),
            row(12, "Background color", "palette", false, -1, false),
            row(1, "Move up", "chevron-up", false, -1, false),
            row(2, "Move down", "chevron-down", false, -1, false),
            row(4, "Copy block", "copy", false, -1, false),
        ];
        if self.block_kind(id) == Some(BlockKind::Image) {
            rows.insert(1, row(13, "Image width", "chevron-right", false, -1, false));
        }
        if self.clipboard.borrow().is_some() {
            rows.push(row(5, "Paste below", "import", false, -1, false));
        }
        rows.push(row(6, "Delete", "trash", true, -1, false));
        self.block_menu.set_vec(rows);
    }

    /// Width submenu for a picture. Three stops, not Notion's four: the
    /// editor column is ~780 px, so a "fit content" tier below 25 % would
    /// resolve to a thumbnail nobody can read (SPEC §三十七).
    pub fn fill_block_menu_image_width(&self, id: i32) {
        let current = self
            .doc
            .borrow()
            .block(BlockId(id.max(0) as u64))
            .map(|b| b.img_percent as i32);
        let mut rows = vec![row(8, "Back", "chevron-left", false, -1, false)];
        for percent in [25, 50, 100] {
            let mut menu_row = row(
                AppState::IMAGE_WIDTH_BASE + percent,
                &format!("{percent}%"),
                "",
                false,
                -1,
                false,
            );
            menu_row.check = current == Some(percent);
            rows.push(menu_row);
        }
        self.block_menu.set_vec(rows);
    }

    /// Second-level "Turn into" menu for one block: the curated kinds minus
    /// the block's own. Action ids encode the target kind as 100 + kind int.
    pub fn fill_block_menu_turn_into(&self, id: i32) {
        let current = self.block_kind(id);
        let mut rows = vec![row(8, "Back", "chevron-left", false, -1, false)];
        for (kind, label, _) in TURN_INTO_ITEMS {
            if Some(*kind) != current {
                let icon = match kind {
                    BlockKind::Paragraph => "pencil",
                    BlockKind::Callout | BlockKind::Code => "page",
                    BlockKind::Toggle => "chevron-right",
                    BlockKind::Image => "image",
                    BlockKind::File => "page",
                    BlockKind::Table => "minimize",
                    _ => "minimize",
                };
                rows.push(row(
                    100 + kind_to_int(*kind),
                    label,
                    icon,
                    false,
                    -1,
                    false,
                ));
            }
        }
        self.block_menu.set_vec(rows);
    }

    /// "Move to" submenu: every page except the one the block lives on,
    /// depth-indented with em spaces. Targets are MOVE_TO_BASE + page id.
    pub fn fill_block_menu_move_to(&self) {
        let current = self.open_page.get();
        let mut rows = vec![row(8, "Back", "chevron-left", false, -1, false)];
        let ws = self.workspace.borrow();
        fn walk(
            ws: &Workspace,
            parent: Option<i32>,
            depth: usize,
            skip: i32,
            out: &mut Vec<MenuRow>,
        ) {
            for id in ws.children_of(parent) {
                if id != skip {
                    let indent = "\u{2003}".repeat(depth);
                    let title = ws.title_of(id).unwrap_or("Untitled");
                    out.push(row(
                        AppState::MOVE_TO_BASE + id,
                        format!("{indent}{title}"),
                        "page",
                        false,
                        -1,
                        false,
                    ));
                    walk(ws, Some(id), depth + 1, skip, out);
                }
            }
        }
        walk(&ws, None, 0, current, &mut rows);
        drop(ws);
        if rows.len() == 1 {
            // inert row (id -1 is swallowed by the controller's id guard)
            rows.push(row(-1, "No other page", "page", false, -1, false));
        }
        self.block_menu.set_vec(rows);
    }

    /// Color submenu: the palette as swatch rows. `background` selects the
    /// row-background palette; otherwise the text palette. Picks are
    /// COLOR_TEXT_BASE / COLOR_BG_BASE + palette slot; the current pick
    /// carries a check mark.
    pub fn fill_block_menu_colors(&self, id: i32, background: bool) {
        let current = self
            .doc
            .borrow()
            .block(BlockId(id.max(0) as u64))
            .map(|b| if background { b.background } else { b.color });
        let mut rows = vec![row(8, "Back", "chevron-left", false, -1, false)];
        for (i, kind) in ColorKind::ALL.iter().enumerate() {
            let base = if background { AppState::COLOR_BG_BASE } else { AppState::COLOR_TEXT_BASE };
            let label = match kind {
                ColorKind::Default => "Default",
                ColorKind::Gray => "Gray",
                ColorKind::Brown => "Brown",
                ColorKind::Orange => "Orange",
                ColorKind::Yellow => "Yellow",
                ColorKind::Green => "Green",
                ColorKind::Blue => "Blue",
                ColorKind::Purple => "Purple",
                ColorKind::Pink => "Pink",
                ColorKind::Red => "Red",
            };
            // the check renders as an icon (MenuRow.check): the software
            // renderer has no font fallback, so glyph marks are unreliable
            let mut menu_row = row(
                base + i as i32,
                label,
                "",
                false,
                i as i32,
                background,
            );
            menu_row.check = Some(*kind) == current;
            rows.push(menu_row);
        }
        self.block_menu.set_vec(rows);
    }

    /// The text that "Copy link to block" puts on the clipboard (the block
    /// anchor doubles as a link-mark URL; clicking one jumps in-app, see
    /// the controller's open-link wiring).
    pub fn block_link(&self, id: i32) -> String {
        // a Page/Link row anchors to its TARGET page, so a copied link opens
        // the page everywhere (the in-app resolver jumps to it directly)
        if matches!(
            self.block_kind_of(id),
            Some(BlockKind::Page) | Some(BlockKind::Link)
        ) {
            if let Some(r) = self.block_page_ref(id) {
                return format!("quire://page/{}", r);
            }
        }
        format!("quire://block/{}", id)
    }

    /// Drop a Page/Link block's reference — the Turn-into path leaving those
    /// kinds. A Page's child page survives in the tree, unowned from here on.
    pub fn clear_block_ref(&self, id: i32) {
        let change = Change::BlockRefSet {
            id: BlockId(id as u64),
            page: None,
        };
        self.doc.borrow_mut().apply(std::slice::from_ref(&change));
        self.record(vec![change]);
        self.reproject_blocks();
    }

    /// Cross-page move (⋮⋮ "Move to"): one undo step for the whole subtree.
    pub fn move_block_to_page(&self, id: i32, page: i32) -> bool {
        if page == self.open_page.get() {
            return false;
        }
        self.exec_on_open_page(Command::MoveBlockToPage {
            id: BlockId(id as u64),
            page: PageId(page as u32 as u64),
        })
        .is_some()
    }

    /// One side of the block color pair, from a submenu pick.
    pub fn set_block_color_slot(&self, id: i32, background: bool, slot: i32) {
        let Some(color) = ColorKind::from_slot(slot) else {
            return;
        };
        let bid = BlockId(id as u64);
        let (c, b) = {
            let doc = self.doc.borrow();
            match doc.block(bid) {
                Some(b) => (b.color, b.background),
                None => return,
            }
        };
        let _ = self.exec_on_open_page(Command::SetBlockColor {
            id: bid,
            color: if background { c } else { color },
            background: if background { color } else { b },
        });
    }

    /// Kind of one block (editing-flow decisions: markdown shortcuts,
    /// Turn-into filtering).
    pub fn block_kind(&self, id: i32) -> Option<BlockKind> {
        self.doc.borrow().block(BlockId(id as u64)).map(|b| b.kind)
    }

    /// Where one cell sits in the grid: its table's id, then its row and
    /// column. `None` for anything that is not a cell of a live grid.
    pub fn table_cell_place(&self, cell: i32) -> Option<(i32, usize, usize)> {
        let (grid, at) = self.grid_of_cell(cell)?;
        Some((grid.table, at / grid.cols, at % grid.cols))
    }

    /// The grid a cell belongs to, and the cell's row-major index in it.
    fn grid_of_cell(&self, cell: i32) -> Option<(LiveGrid, usize)> {
        let parent = {
            let d = self.doc.borrow();
            let b = d.block(BlockId(cell.max(0) as u64))?;
            if b.kind != BlockKind::TableCell {
                return None;
            }
            b.parent?
        };
        let grid = self.live_grid(parent.as_u64() as i32)?;
        let at = grid.cells.iter().position(|c| *c == BlockId(cell as u64))?;
        Some((grid, at))
    }

    /// A block as a grid, when it is a table with columns (a table with none
    /// has no shape to edit, and the commands refuse it the same way).
    fn live_grid(&self, table: i32) -> Option<LiveGrid> {
        LiveGrid::of(&self.doc.borrow(), BlockId(table.max(0) as u64))
    }

    /// Tab / Shift-Tab through a grid. Stepping past the last cell appends a
    /// row first — a table grows as far as the typing goes, which is how
    /// Notion's Tab reads (SPEC §三十七 批次 B) — and stepping off the front
    /// stays put. Returns the cell to focus.
    pub fn table_step(&self, cell: i32, delta: i32) -> Option<i32> {
        let (grid, at) = self.grid_of_cell(cell)?;
        let target = at as i64 + delta as i64;
        if target < 0 {
            return None;
        }
        if target as usize >= grid.cells.len() {
            let changes = self.exec_on_open_page(Command::TableAddRow {
                id: BlockId(grid.table as u64),
                row: grid.rows(),
            })?;
            return changes.iter().find_map(|ch| match ch {
                Change::BlockInserted(b) if b.kind == BlockKind::TableCell => {
                    Some(b.id.as_u64() as i32)
                }
                _ => None,
            });
        }
        grid.cells.get(target as usize).map(|c| c.as_u64() as i32)
    }

    /// Where the grid's toolbar lands: the focused cell's row and column when
    /// the focus is a cell of this table, else its last row and column.
    fn table_anchor(&self, grid: &LiveGrid, focus: i32) -> (usize, usize) {
        match self.table_cell_place(focus) {
            Some((owner, row, col)) if owner == grid.table => (row, col),
            _ => (grid.rows().saturating_sub(1), grid.cols.saturating_sub(1)),
        }
    }

    /// The four pills on the grid's toolbar. Adding goes *after* the anchor so
    /// the row under the caret keeps its place; deleting takes the anchor, and
    /// the plan refuses a table's final row or column.
    pub fn table_add_row(&self, table: i32, focus: i32) -> bool {
        let Some(grid) = self.live_grid(table) else { return false };
        let (row, _) = self.table_anchor(&grid, focus);
        self.exec_on_open_page(Command::TableAddRow {
            id: BlockId(table as u64),
            row: (row + 1).min(grid.rows()),
        })
        .is_some()
    }

    pub fn table_add_column(&self, table: i32, focus: i32) -> bool {
        let Some(grid) = self.live_grid(table) else { return false };
        let (_, col) = self.table_anchor(&grid, focus);
        self.exec_on_open_page(Command::TableAddColumn {
            id: BlockId(table as u64),
            col: (col + 1).min(grid.cols),
        })
        .is_some()
    }

    pub fn table_delete_row(&self, table: i32, focus: i32) -> bool {
        let Some(grid) = self.live_grid(table) else { return false };
        let (row, _) = self.table_anchor(&grid, focus);
        self.exec_on_open_page(Command::TableDeleteRow {
            id: BlockId(table as u64),
            row,
        })
        .is_some()
    }

    pub fn table_delete_column(&self, table: i32, focus: i32) -> bool {
        let Some(grid) = self.live_grid(table) else { return false };
        let (_, col) = self.table_anchor(&grid, focus);
        self.exec_on_open_page(Command::TableDeleteColumn {
            id: BlockId(table as u64),
            col,
        })
        .is_some()
    }

    /// The cell's row-major slot in its grid. The coordinate a delete can
    /// reuse to hand the caret back where it was.
    pub fn table_cell_index(&self, cell: i32) -> Option<usize> {
        Some(self.grid_of_cell(cell)?.1)
    }

    /// The grid's cell at row-major slot `at`, clamped into whatever the grid
    /// is now — where focus lands after a delete took the cell under it.
    pub fn table_cell_at(&self, table: i32, at: usize) -> Option<i32> {
        let grid = self.live_grid(table)?;
        let at = at.min(grid.cells.len().saturating_sub(1));
        grid.cells.get(at).map(|c| c.as_u64() as i32)
    }

    /// True when `id` is a table cell — the row the editor must not turn,
    /// split, merge or paste block structure into (SPEC §三十七 批次 B).
    pub fn is_table_cell(&self, id: i32) -> bool {
        self.block_kind(id) == Some(BlockKind::TableCell)
    }

    /// Patch one committed cell's text into the row that carries it. A cell
    /// has no row of its own — its text lives in its table's `table-cells` —
    /// so the targeted row sync the typing flush does for every other block
    /// finds nothing here, and the grid would keep showing the old word the
    /// moment the caret left.
    pub fn sync_cell_text(&self, cell: i32, text: &str) {
        let Some((grid, at)) = self.grid_of_cell(cell) else { return };
        let mut i = 0;
        while let Some(mut row) = self.blocks.row_data(i) {
            if row.id == grid.table {
                let mut cells: Vec<TableCell> = (0..row.table_cells.row_count())
                    .filter_map(|c| row.table_cells.row_data(c))
                    .collect();
                if let Some(c) = cells.get_mut(at) {
                    c.text = text.into();
                    row.table_cells = ModelRc::from(Rc::new(VecModel::from(cells)));
                    self.blocks.set_row_data(i, row);
                }
                return;
            }
            i += 1;
        }
    }

    // ---- columns layout (SPEC §三十七 批次 B) ----

    /// A layout as the editor sees it: its blocks in reading order, the very
    /// list the layout's own row carries as `column-items`. Tab and the text
    /// flush walk that list, so focus and the model can never disagree.
    fn live_layout(&self, layout: i32) -> Option<LiveLayout> {
        LiveLayout::of(&self.doc.borrow(), BlockId(layout.max(0) as u64))
    }

    /// The layout a block belongs to, and the block's slot in it.
    fn layout_of_item(&self, item: i32) -> Option<(LiveLayout, usize)> {
        let owner = {
            let d = self.doc.borrow();
            let mut cur = d.block(BlockId(item.max(0) as u64))?.parent?;
            loop {
                let b = d.block(cur)?;
                if b.kind == BlockKind::Columns {
                    break cur;
                }
                cur = b.parent?;
            }
        };
        let layout = self.live_layout(owner.as_u64() as i32)?;
        let at = layout.items.iter().position(|x| *x == BlockId(item as u64))?;
        Some((layout, at))
    }

    /// Tab / Shift-Tab through a layout. Either end stops rather than leaving
    /// the layout: a box is exited by clicking out, and growing a box on Tab
    /// would invent a shape the reader did not ask for.
    pub fn column_step(&self, item: i32, delta: i32) -> Option<i32> {
        let (layout, at) = self.layout_of_item(item)?;
        let target = at as i64 + delta as i64;
        if target < 0 {
            return None;
        }
        layout.items.get(target as usize).map(|i| i.as_u64() as i32)
    }

    pub fn column_add(&self, layout: i32) -> bool {
        self.exec_on_open_page(Command::ColumnsAddColumn { id: BlockId(layout as u64) })
            .is_some()
    }

    pub fn column_remove(&self, layout: i32) -> bool {
        self.exec_on_open_page(Command::ColumnsDeleteColumn { id: BlockId(layout as u64) })
            .is_some()
    }

    /// Give one of the layout's boxes its first line, and report the block the
    /// caret should land on. A box is the only thing on the page a click cannot
    /// put a caret in, so this is that click's whole job.
    pub fn column_fill(&self, column: i32) -> Option<i32> {
        let changes =
            self.exec_on_open_page(Command::ColumnsAddBlock { id: BlockId(column as u64) })?;
        changes.iter().find_map(|c| match c {
            Change::BlockInserted(b) if b.kind == BlockKind::Paragraph => {
                Some(b.id.as_u64() as i32)
            }
            _ => None,
        })
    }

    /// True when `id` is drawn by a layout's delegate instead of by a row of
    /// its own: it has no row for the targeted sync, the menus or the search
    /// focus to work on (SPEC §三十七 批次 B).
    pub fn is_column_item(&self, id: i32) -> bool {
        self.layout_of_item(id).is_some()
    }

    /// Patch one committed item's text into the layout row that carries it —
    /// the same job `sync_cell_text` does for a grid.
    pub fn sync_column_text(&self, item: i32, text: &str) {
        let Some((layout, at)) = self.layout_of_item(item) else { return };
        let mut i = 0;
        while let Some(mut row) = self.blocks.row_data(i) {
            if row.id == layout.layout {
                let mut items: Vec<ColumnItem> = (0..row.column_items.row_count())
                    .filter_map(|c| row.column_items.row_data(c))
                    .collect();
                if let Some(it) = items.get_mut(at) {
                    it.text = text.into();
                    row.column_items = ModelRc::from(Rc::new(VecModel::from(items)));
                    self.blocks.set_row_data(i, row);
                }
                return;
            }
            i += 1;
        }
    }

    pub fn copy_block(&self, id: i32) {
        if let Some(b) = self.doc.borrow().block(BlockId(id as u64)) {
            *self.clipboard.borrow_mut() = Some(b.clone());
        }
    }

    /// The empty page's front door: put one paragraph on a page that has no
    /// rows, and hand back its id so the caller can land the caret in it.
    /// Every other insert is anchored to an existing block, so until this
    /// existed a page with zero blocks could not be typed into at all.
    pub fn start_page(&self) -> Option<i32> {
        let changes = self.exec_on_open_page(Command::AppendBlock {
            kind: BlockKind::Paragraph,
            text: String::new(),
        })?;
        changes.iter().find_map(|ch| match ch {
            Change::BlockInserted(b) => Some(b.id.0 as i32),
            _ => None,
        })
    }

    pub fn paste_below(&self, id: i32) -> bool {
        let clip = self.clipboard.borrow().clone();
        let Some(c) = clip else { return false };
        // pasting a Page block would share the source's owned child page;
        // land the title as plain text instead. A Link block's target is
        // unowned, so the paste keeps the reference.
        if c.kind == BlockKind::Page {
            return self.exec_on_open_page(Command::InsertBlockAfter {
                id: BlockId(id as u64),
                kind: BlockKind::Paragraph,
                text: c.text,
            })
            .is_some();
        }
        if c.kind == BlockKind::Link {
            let Some(changes) = self.exec_on_open_page(Command::InsertBlockAfter {
                id: BlockId(id as u64),
                kind: BlockKind::Link,
                text: c.text,
            }) else {
                return false;
            };
            let Some(new_id) = changes.iter().find_map(|ch| match ch {
                Change::BlockInserted(b) => Some(b.id),
                _ => None,
            }) else {
                return false;
            };
            let change = Change::BlockRefSet {
                id: new_id,
                page: c.page_ref,
            };
            self.doc.borrow_mut().apply(std::slice::from_ref(&change));
            self.record(vec![change]);
            self.reproject_blocks();
            return true;
        }
        // An attachment block's pointer is shared, not owned: the paste shows
        // the same file from a second row. With no row to point at — an
        // attachment from a database this session never loaded — the name is
        // still worth more as text than as a broken reference.
        if matches!(c.kind, BlockKind::Image | BlockKind::File) {
            let row = c.attachment.and_then(|a| {
                self.attachments
                    .borrow()
                    .get(&(a.as_u64() as i64))
                    .cloned()
            });
            if let Some(att) = row {
                return self.insert_attachment(id, att, c.kind);
            }
            return self
                .exec_on_open_page(Command::InsertBlockAfter {
                    id: BlockId(id as u64),
                    kind: BlockKind::Paragraph,
                    text: c.text,
                })
                .is_some();
        }
        self.exec_on_open_page(Command::InsertBlockAfter {
            id: BlockId(id as u64),
            kind: c.kind,
            text: c.text,
        })
        .is_some()
    }

    // --- attachments (SPEC §三十七 批次 A) ---

    /// Id for the next attachment the session stores. Ids only have to be
    /// unique, not dense, so a cancelled file pick may burn one.
    pub fn claim_attachment_id(&self) -> AttachmentId {
        let id = self.next_attachment_id.get();
        self.next_attachment_id.set(id + 1);
        AttachmentId(id as u64)
    }

    /// The attachment lands as a new block below `after_id` — the "+" menu and
    /// block paste, which mean "a picture/file appears here". `kind` selects
    /// between the two attachment commands; nothing else in the app does.
    pub fn insert_attachment(&self, after_id: i32, attachment: Attachment, kind: BlockKind) -> bool {
        let id = BlockId(after_id.max(0) as u64);
        let cmd = match kind {
            BlockKind::Image => Command::InsertImage { id, attachment: attachment.clone() },
            BlockKind::File => Command::InsertFile { id, attachment: attachment.clone() },
            _ => return false,
        };
        self.run_attachment_command(cmd, attachment)
    }

    /// This block becomes the attachment — slash "/" and Turn into, which
    /// convert the block they were opened on. The id survives, so a row with
    /// children keeps them.
    pub fn set_block_attachment(&self, id: i32, attachment: Attachment, kind: BlockKind) -> bool {
        let bid = BlockId(id.max(0) as u64);
        let cmd = match kind {
            BlockKind::Image => Command::SetBlockImage { id: bid, attachment: attachment.clone() },
            BlockKind::File => Command::SetBlockFile { id: bid, attachment: attachment.clone() },
            _ => return false,
        };
        self.run_attachment_command(cmd, attachment)
    }

    /// A picture off the clipboard (SPEC §三十七 批次 A's last open item). The
    /// bytes are stored exactly like a picked file, and the block follows the
    /// caret: a block with nothing in it *becomes* the picture, a written one
    /// gets the picture below it, so a paste never leaves a stray empty line.
    /// `png` is what `platform::read_clipboard_image` already decoded and
    /// re-encoded; a false return means the page refused it, not the clipboard.
    pub fn paste_image(&self, id: i32, png: &[u8]) -> bool {
        let Ok(att) = self
            .store
            .import_bytes(self.claim_attachment_id(), "Pasted image", png)
        else {
            return false;
        };
        let empty = self
            .doc
            .borrow()
            .block(BlockId(id.max(0) as u64))
            .map(|b| b.text.is_empty())
            .unwrap_or(false);
        if empty {
            self.set_block_attachment(id, att, BlockKind::Image)
        } else {
            self.insert_attachment(id, att, BlockKind::Image)
        }
    }

    /// Both attachment edits put the `attachments` row into the database as
    /// part of the command's own batch, so persistence and undo stay
    /// single-tracked. The in-memory copy is the lookup `image_for` and
    /// `attachment_size` use; a rejected plan leaves a row nothing points at,
    /// which costs a file in the folder and nothing on screen.
    fn run_attachment_command(&self, cmd: Command, attachment: Attachment) -> bool {
        self.attachments
            .borrow_mut()
            .insert(attachment.id.as_u64() as i64, attachment);
        self.exec_on_open_page(cmd).is_some()
    }

    /// One attachment row as this session loaded it. `None` for an id from a
    /// database we never opened — a block copied in from another library,
    /// which has bytes on screen but nothing to hand the system.
    pub fn attachment_row(&self, id: i32) -> Option<Attachment> {
        if id <= 0 {
            return None;
        }
        self.attachments.borrow().get(&(id as i64)).cloned()
    }

    /// "1.4 MiB" for a file row's right-hand label; empty when there is no row
    /// to size. A callback rather than a model field so the string is not
    /// rebuilt for every row on every keystroke (§三十七, ADR-0029).
    pub fn attachment_size(&self, id: i32) -> String {
        let Some(att) = self.attachment_row(id) else {
            return String::new();
        };
        crate::services::attachment_store::format_size(att.bytes).into()
    }

    /// The display width of an image block (25 / 50 / 100, SPEC §三十七).
    pub fn set_image_width(&self, id: i32, percent: i32) -> bool {
        let percent = percent.clamp(1, 100) as u16;
        self.exec_on_open_page(Command::SetImageWidth {
            id: BlockId(id as u64),
            percent,
        })
        .is_some()
    }

    /// The raster behind an image row, decoded the first time it is asked for.
    /// This is a callback rather than a model field precisely so it is asked
    /// only for rows the ListView realizes: a page with five hundred pictures
    /// holds about ten in memory (§二十二). Slint caches by path, and so do we.
    pub fn image_for(&self, id: i32) -> slint::Image {
        let key = id as i64;
        if id <= 0 {
            return slint::Image::default();
        }
        self.attachment_tick.set(self.attachment_tick.get() + 1);
        let tick = self.attachment_tick.get();
        if let Some(hit) = self.attachment_images.borrow_mut().get_mut(&key) {
            hit.used = tick;
            return hit.image.clone();
        }
        let Some(att) = self.attachments.borrow().get(&key).cloned() else {
            return slint::Image::default();
        };
        let path = self.store.display_path(&att);
        let img = slint::Image::load_from_path(&path).unwrap_or_default();
        // A file that cannot be decoded caches as blank: the row must not
        // re-open it on every repaint, and a zero-size raster costs the budget
        // nothing. Replacing the file on disk needs a restart to show up.
        let size = img.size();
        let bytes = size.width.max(0) as usize * size.height.max(0) as usize * 4;
        self.cache_image(key, CachedImage { used: tick, bytes, image: img.clone() });
        img
    }

    /// Insert, then spend the budget: the least-recently-realized raster goes
    /// first, so a fast scroll through a photo page cannot stack up frames.
    fn cache_image(&self, key: i64, entry: CachedImage) {
        let weight = entry.bytes;
        let mut cache = self.attachment_images.borrow_mut();
        if let Some(old) = cache.insert(key, entry) {
            self.attachment_cache_bytes.set(self.attachment_cache_bytes.get() - old.bytes);
        }
        let mut total = self.attachment_cache_bytes.get() + weight;
        while total > MAX_ATTACHMENT_CACHE_BYTES {
            // A dozen entries live here at most, so scanning for the oldest
            // use beats keeping a second, ordered structure in step.
            let Some((&oldest, _)) = cache.iter().min_by_key(|(_, v)| v.used) else { break };
            total -= cache.remove(&oldest).unwrap().bytes;
        }
        self.attachment_cache_bytes.set(total);
        self.attachment_cache_peak
            .set(self.attachment_cache_peak.get().max(total));
    }

    /// Drop one raster from the decode cache and give its weight back. Only the
    /// reclaim needs it: the budget loop otherwise decides when a picture
    /// leaves, and here the file itself is gone, so a later `image_for` for a
    /// reused id must not find the old bytes still cached (SPEC §三十七,
    /// ADR-0037).
    fn evict_image(&self, key: i64) {
        let Some(old) = self.attachment_images.borrow_mut().remove(&key) else {
            return;
        };
        self.attachment_cache_bytes
            .set(self.attachment_cache_bytes.get() - old.bytes);
    }

    /// One line the bench harness reads from stderr: what the decode cache
    /// holds now and the high-water mark it reached, plus where the scroll got
    /// to — a picture cache that never grew is only a finding if the page
    /// really moved. A constructed ceiling without a reading is a promise.
    pub fn attachment_cache_report(&self) -> String {
        format!(
            "{{\"event\":\"attachment_cache\",\"bytes\":{},\"peak_bytes\":{},\"entries\":{},\"budget\":{},\"scroll_y\":{:.0}}}\n",
            self.attachment_cache_bytes.get(),
            self.attachment_cache_peak.get(),
            self.attachment_images.borrow().len(),
            MAX_ATTACHMENT_CACHE_BYTES,
            self.last_scroll_y.get(),
        )
    }

    /// height / width of the raster `image_for` returns, 0 when there is
    /// nothing to show. The row needs the ratio to size itself before it has
    /// the picture, and `slint::Image`'s size is not readable from .slint.
    pub fn image_aspect(&self, id: i32) -> f32 {
        let size = self.image_for(id).size();
        if size.width <= 0 {
            return 0.0;
        }
        size.height as f32 / size.width as f32
    }

    /// The directory the database lives in (settings storage row). `None`
    /// for a memory-only session.
    pub fn data_dir(&self) -> Option<String> {
        self.repo.as_ref().and_then(|r| {
            r.path()
                .and_then(|p| p.parent())
                .map(|p| p.display().to_string())
        })
    }

    /// Take a snapshot right now (settings "Back up now"). `Ok(message)` is
    /// user-visible confirmation; a memory-only session is a no-op that
    /// still reports success (there is nothing on disk to protect).
    pub fn backup_now(&self) -> Result<String, String> {
        match self.repo.as_ref().filter(|r| r.path().is_some()) {
            Some(r) => match r.snapshot() {
                Ok(()) => Ok("a fresh backup was created".into()),
                Err(e) => Err(e.to_string()),
            },
            None => Ok("running in memory — nothing to back up".into()),
        }
    }

    /// Reclaim the attachments nothing can reach any more (SPEC §三十七,
    /// ADR-0037): delete the `attachments` rows and, once the row is gone, the
    /// files beside them.
    ///
    /// "Reachable" is deliberately generous — every block of every page in the
    /// document, every id inside an outstanding undo *or* redo step, and the
    /// copied block in the internal clipboard. A reclaim that leaves an orphan
    /// behind costs disk; one that deletes a picture Ctrl+Z was about to bring
    /// back costs the user's bytes, so the scan errs to the former.
    ///
    /// It works from the loaded book, never from a directory listing: if the
    /// rows failed to load this session the book is empty and the sweep removes
    /// nothing, which is the only honest answer to "I cannot see the references".
    pub fn reclaim_attachments(&self) -> Result<String, String> {
        let Some(repo) = self.repo.as_ref().filter(|r| r.path().is_some()) else {
            return Ok("running in memory — nothing to reclaim".into());
        };
        // Queue first, sweep second. The debounced writer may still hold an
        // `AttachmentAdded` for a picture this session has already dropped from
        // the document; applying a DELETE out of order ahead of it would leave
        // the row *behind* pointing at files this call is about to remove.
        if let Some(p) = &self.persistence {
            if let Err(e) = p.force_flush() {
                return Err(format!("the queued edits could not be written ({e})"));
            }
        }

        let mut live: std::collections::BTreeSet<i64> = self
            .doc
            .borrow()
            .all_blocks()
            .filter_map(|b| b.attachment)
            .map(|a| a.as_u64() as i64)
            .collect();
        live.extend(self.history.borrow().referenced_attachments());
        if let Some(id) = self.clipboard.borrow().as_ref().and_then(|b| b.attachment) {
            live.insert(id.as_u64() as i64);
        }

        let doomed: Vec<Attachment> = self
            .attachments
            .borrow()
            .values()
            .filter(|a| !live.contains(&(a.id.as_u64() as i64)))
            .cloned()
            .collect();
        if doomed.is_empty() {
            return Ok("no unused attachments to remove".into());
        }

        let changes: Vec<Change> = doomed
            .iter()
            .map(|a| Change::AttachmentDeleted { id: a.id })
            .collect();
        // Rows before bytes: a failed write leaves the picture exactly where it
        // was, while a failed delete only leaves bytes no row claims.
        repo.apply(&changes).map_err(|e| e.to_string())?;

        let mut freed = 0i64;
        let mut stuck: Vec<String> = Vec::new();
        for att in &doomed {
            let key = att.id.as_u64() as i64;
            stuck.extend(self.store.remove(att));
            self.attachments.borrow_mut().remove(&key);
            self.evict_image(key);
            freed += att.bytes.max(0);
        }

        let removed = doomed.len();
        let mut notice = format!(
            "{removed} unused attachment{} removed ({})",
            if removed == 1 { "" } else { "s" },
            crate::services::attachment_store::format_size(freed),
        );
        if !stuck.is_empty() {
            notice.push_str(&format!(
                ", but {} file{} could not be deleted",
                stuck.len(),
                if stuck.len() == 1 { "" } else { "s" }
            ));
        }
        Ok(notice)
    }

    /// Plan+apply several commands as ONE undo step, refresh the rows.
    pub fn exec_all_on_open_page(&self, cmds: Vec<Command>) -> Option<Vec<Change>> {
        let page = core_page_id(self.open_page.get());
        let changes = crate::core::command::exec_all(
            &mut self.doc.borrow_mut(),
            &mut self.history.borrow_mut(),
            page,
            cmds,
        )?;
        self.record(changes.clone());
        self.reproject_blocks();
        Some(changes)
    }

    /// Rich paste (SPEC §二十七): land parsed markdown blocks into the
    /// document. An empty current row (the fresh "+" line, or a brand-new
    /// paragraph) is converted into the first parsed block in place;
    /// otherwise every parsed block inserts after it. Blocks go through the
    /// command system, and marks replay as one `ToggleMark` per span (the
    /// command plans against the pre-state, so a batch of them would
    /// overwrite each other) — an N-block paste is therefore several undo
    /// steps, accepted for v1 and noted in PLAN.
    pub fn paste_block_structure(
        &self,
        id: i32,
        parsed: &[crate::services::import_service::ParsedBlock],
    ) -> bool {
        if parsed.is_empty() {
            return false;
        }
        let page = core_page_id(self.open_page.get());
        let block_id = BlockId(id as u64);
        let current_empty = {
            let doc = self.doc.borrow();
            match doc.page_blocks(page).iter().find(|b| b.id == block_id) {
                Some(b) => b.kind == BlockKind::Paragraph && b.text.is_empty(),
                None => return false,
            }
        };
        let mut anchor = id;
        for (i, p) in parsed.iter().enumerate() {
            let target = if i == 0 && current_empty {
                let _ = self.exec_all_on_open_page(vec![
                    Command::ReplaceText {
                        id: block_id,
                        text: p.text.clone(),
                    },
                    Command::SetBlockType {
                        id: block_id,
                        kind: p.kind,
                    },
                ]);
                if p.checked {
                    let _ = self.exec_on_open_page(Command::ToggleTodoChecked {
                        id: block_id,
                    });
                }
                self.apply_marks(block_id, &p.marks);
                block_id
            } else {
                let Some(changes) = self.exec_on_open_page(Command::InsertBlockAfter {
                    id: BlockId(anchor as u64),
                    kind: p.kind,
                    text: p.text.clone(),
                }) else {
                    return false;
                };
                let Some(nid) = changes.iter().find_map(|c| match c {
                    Change::BlockInserted(b) => Some(b.id),
                    _ => None,
                }) else {
                    return false;
                };
                if p.checked {
                    let _ = self.exec_on_open_page(Command::ToggleTodoChecked { id: nid });
                }
                self.apply_marks(nid, &p.marks);
                anchor = nid.as_u64() as i32;
                nid
            };
            let _ = target;
        }
        true
    }

    /// Replay parsed inline-mark spans as sequential ToggleMark commands.
    fn apply_marks(&self, id: BlockId, marks: &[crate::core::types::Mark]) {
        for m in marks {
            let _ = self.exec_on_open_page(Command::ToggleMark {
                id,
                start: m.start,
                end: m.end,
                kind: m.kind,
                url: m.url.clone(),
            });
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
        let from = self.open_page.get();
        self.open_page(id);
        // creating a page navigates to it, so Go Back returns where the user
        // was (SPEC §十六)
        self.nav.borrow_mut().record(from, id);
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
        // Page blocks embedding this page show its live title
        self.reproject_blocks();
    }

    pub fn duplicate_page(&self, id: i32) -> Option<i32> {
        let new_id = self.workspace.borrow_mut().duplicate(id);
        if let Some(nid) = new_id {
            // copy the source page's blocks with fresh ids, remapping
            // parent pointers through the same map (nested lists survive).
            // The id range is RESERVED on the document — ids minted without
            // reserving collided with the next allocation (M8_FEEDBACK #2).
            let src = core_page_id(id);
            let dst = core_page_id(nid);
            let (copies, _id_map) = {
                let mut doc = self.doc.borrow_mut();
                let src_blocks = doc.page_blocks(src).to_vec();
                let start = doc.reserve_block_ids(src_blocks.len() as u64);
                let mut map = HashMap::new();
                let copies: Vec<Block> = src_blocks
                    .iter()
                    .enumerate()
                    .map(|(i, b)| {
                        let new_id = BlockId(start + i as u64);
                        map.insert(b.id, new_id);
                        let mut c = b.clone();
                        c.id = new_id;
                        c.page = dst;
                        c
                    })
                    .collect();
                // second pass: parents point at the copies now
                let copies: Vec<Block> = copies
                    .into_iter()
                    .map(|mut c| {
                        if let Some(pid) = c.parent {
                            c.parent = map.get(&pid).copied();
                        }
                        c
                    })
                    .collect();
                (copies, map)
            };
            let title = self.workspace.borrow().title_of(nid).unwrap().to_string();
            let blob = block_search_blob(&title, &project_blocks(&copies));

            // order: right after the original when a gap exists, else the
            // end of the sibling run. The workspace children vec must agree
            // with the key in BOTH branches — duplicate() attaches the copy
            // adjacent to the original, so the append fallback re-attaches
            // at the end; otherwise the session view (vec order) and the
            // restart projection (key order) would disagree.
            let (parent, order, appended) = {
                let ws = self.workspace.borrow();
                let parent = ws.get(nid).and_then(|p| p.parent);
                let kids = ws.children_of(parent);
                // the sibling AFTER the original — skipping the fresh copy,
                // which duplicate() parked right there without a key yet
                let next = kids
                    .iter()
                    .skip_while(|&&k| k != id)
                    .skip(1)
                    .find(|&&k| k != nid)
                    .and_then(|k| self.page_order.borrow().get(k).copied());
                let orig = self.page_order.borrow().get(&id).copied();
                match OrderKey::between(orig, next) {
                    Some(key) => (parent, key, false),
                    None => {
                        let last = kids
                            .iter()
                            .filter(|&&k| k != nid)
                            .last()
                            .and_then(|k| self.page_order.borrow().get(k).copied());
                        (
                            parent,
                            OrderKey::between(last, None).expect("order space exhausted"),
                            true,
                        )
                    }
                }
            };
            self.page_order.borrow_mut().insert(nid, order);
            if appended {
                self.workspace.borrow_mut().move_page(nid, parent, None);
            }

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

    /// The page a `Page`-kind block points at, if it is still there.
    pub fn block_page_ref(&self, id: i32) -> Option<i32> {
        let doc = self.doc.borrow();
        doc.block(BlockId(id as u64))
            .and_then(|b| b.page_ref)
            .map(|p| p.as_u64() as i32)
    }

    /// The kind of one block, for the controller's per-kind decisions.
    pub fn block_kind_of(&self, id: i32) -> Option<BlockKind> {
        let doc = self.doc.borrow();
        doc.block(BlockId(id as u64)).map(|b| b.kind)
    }

    /// Fill the slash popup with the page picker: every page in tree order,
    /// label = title, hint = breadcrumb. Typing filters by title.
    pub fn open_slash_pick(&self, filter: &str) {
        let needle = filter.to_lowercase();
        let ws = self.workspace.borrow();
        let rows: Vec<SlashRow> = ws
            .dfs_order()
            .into_iter()
            .filter_map(|id| {
                let title = ws.title_of(id)?;
                if !needle.is_empty() && !title.to_lowercase().contains(&needle) {
                    return None;
                }
                Some(SlashRow {
                    id,
                    label: title.into(),
                    hint: ws.breadcrumb(id).into(),
                    disabled: false,
                })
            })
            .collect();
        self.slash.set_vec(rows);
    }

    /// The page id behind the focused picker row.
    pub fn slash_selected_page(&self, focus: i32) -> Option<i32> {
        let row = self.slash.row_data(focus.max(0) as usize)?;
        if row.disabled {
            return None;
        }
        Some(row.id)
    }

    /// Convert the empty paragraph `id` (the "+" handle's fresh line) into a
    /// Link-to-page block pointing at `target`. The target is NOT owned:
    /// deleting the block leaves the page alone, so duplicates and pastes may
    /// share it freely. One recorded batch.
    pub fn create_page_link_block(&self, id: i32, target: i32) -> bool {
        let block_id = BlockId(id as u64);
        let page = self.open_page.get();
        {
            let doc = self.doc.borrow();
            let b = doc
                .page_blocks(core_page_id(page))
                .iter()
                .find(|b| b.id == block_id);
            let Some(b) = b else { return false };
            if b.kind != BlockKind::Paragraph || !b.text.is_empty() {
                return false;
            }
        }
        if !self.workspace.borrow().contains(target) {
            return false;
        }
        let changes = vec![
            Change::BlockRefSet {
                id: block_id,
                page: Some(PageId(target as u32 as u64)),
            },
            Change::BlockKindSet {
                id: block_id,
                kind: BlockKind::Link,
            },
        ];
        self.doc.borrow_mut().apply(&changes);
        self.record(changes);
        self.reproject_blocks();
        true
    }

    /// Turn the empty paragraph `after_id` (the "+" handle's fresh line, or a
    /// row the insert menu is applying to) into a Page block: one child page
    /// is created under the current page and the block points at it. One
    /// recorded batch. Page creation is not undoable (same as the sidebar
    /// flow), so undo restores the block kind but not the page.
    pub fn create_page_block(&self, after_id: i32) -> Option<i32> {
        let block_id = BlockId(after_id as u64);
        let parent_page = self.open_page.get();
        {
            let doc = self.doc.borrow();
            let b = doc
                .page_blocks(core_page_id(parent_page))
                .iter()
                .find(|b| b.id == block_id)?;
            if b.kind != BlockKind::Paragraph || !b.text.is_empty() {
                return None;
            }
        }
        let child = self.workspace.borrow_mut().create(Some(parent_page), "Untitled");
        let order = {
            let kids = self.workspace.borrow().children_of(Some(parent_page));
            let map = self.page_order.borrow();
            let prev = kids
                .len()
                .checked_sub(2)
                .and_then(|i| kids.get(i))
                .and_then(|pid| map.get(pid).copied());
            OrderKey::between(prev, None).expect("append order exhausted")
        };
        self.page_order.borrow_mut().insert(child, order);
        self.workspace
            .borrow_mut()
            .set_search_text(child, "Untitled".into());
        let child_id = PageId(child as u32 as u64);
        let changes = vec![
            Change::PageCreated(crate::core::Page {
                id: child_id,
                title: "Untitled".into(),
                parent: Some(PageId(parent_page as u32 as u64)),
                order,
                favorite: false,
                expanded: false,
            }),
            Change::BlockRefSet {
                id: block_id,
                page: Some(child_id),
            },
            Change::BlockKindSet {
                id: block_id,
                kind: BlockKind::Page,
            },
        ];
        self.doc.borrow_mut().apply(&changes);
        self.record(changes);
        self.rebuild_sidebar();
        self.reproject_blocks();
        Some(child)
    }

    /// Duplicate a Page block: the child page is deep-copied (sidebar
    /// semantics) and the fresh block points at the copy, so two blocks never
    /// share a target — deleting one would not orphan the other. A Link block
    /// does NOT take this path: its target is unowned, so the plain duplicate
    /// (which clones the ref) is safe.
    pub fn duplicate_page_block(&self, id: i32) -> Option<i32> {
        if self.block_kind_of(id) != Some(BlockKind::Page) {
            return None;
        }
        let page_ref = self.block_page_ref(id)?;
        let copy_page = self.duplicate_page(page_ref)?;
        let changes = self.exec_on_open_page(Command::DuplicateBlock {
            id: BlockId(id as u64),
        })?;
        let new_id = changes.iter().find_map(|c| match c {
            Change::BlockInserted(b) => Some(b.id),
            _ => None,
        })?;
        let change = Change::BlockRefSet {
            id: new_id,
            page: Some(PageId(copy_page as u32 as u64)),
        };
        self.doc.borrow_mut().apply(std::slice::from_ref(&change));
        self.record(vec![change]);
        self.reproject_blocks();
        Some(new_id.as_u64() as i32)
    }

    /// Move a page (and its subtree) under `new_parent` (root when `None`),
    /// appended as the parent's last child. One `PageMoved` change; the
    /// workspace refuses cycles (a parent cannot move into its own subtree).
    pub fn move_page(&self, id: i32, new_parent: Option<i32>) -> bool {
        let order = {
            let ws = self.workspace.borrow();
            let last = ws
                .children_of(new_parent)
                .last()
                .and_then(|k| self.page_order.borrow().get(k).copied());
            OrderKey::between(last, None).expect("append order exhausted")
        };
        if !self
            .workspace
            .borrow_mut()
            .move_page(id, new_parent, None)
        {
            return false;
        }
        self.page_order.borrow_mut().insert(id, order);
        self.record(vec![Change::PageMoved {
            id: PageId(id as u32 as u64),
            parent: new_parent.map(|v| PageId(v as u32 as u64)),
            order,
        }]);
        self.rebuild_sidebar();
        true
    }

    /// The drop target behind a sidebar row index, for the page-tree drag:
    /// `Some(None)` = the Workspace header (top level), `Some(Some(pid))` =
    /// drop into that page, `None` = not a target (other headers, recents,
    /// the "New page" row).
    pub fn page_drop_target(&self, index: i32) -> Option<Option<i32>> {
        let row = self.sidebar.row_data(index.max(0) as usize)?;
        match row.kind.as_str() {
            "page" => Some(Some(row.id)),
            "header" if row.id == WORKSPACE_HEADER_ID => Some(None),
            _ => None,
        }
    }

    /// Drag-hover validity for the page tree (called per frame of the
    /// gesture — read-only, like the block drag's hover check).
    pub fn page_drop_target_valid(&self, id: i32, index: i32) -> bool {
        let Some(target) = self.page_drop_target(index) else {
            return false;
        };
        let ws = self.workspace.borrow();
        ws.can_move_page(id, target)
    }

    /// Commit a page-tree drag: the dragged page moves under the row it was
    /// dropped on (the Workspace header moves it to the top level).
    pub fn page_dropped(&self, id: i32, index: i32) -> bool {
        let Some(target) = self.page_drop_target(index) else {
            return false;
        };
        self.move_page(id, target)
    }

    /// Swap a page with the sibling one slot up (-1) / down (+1): the two
    /// order keys trade places, recorded as two `PageMoved` changes.
    pub fn move_page_by(&self, id: i32, delta: i32) -> bool {
        let (parent, neighbor) = {
            let ws = self.workspace.borrow();
            let Some(parent) = ws.get(id).map(|p| p.parent) else {
                return false;
            };
            let kids = ws.children_of(parent);
            let Some(idx) = kids.iter().position(|&c| c == id) else {
                return false;
            };
            let nidx = idx as isize + delta as isize;
            if nidx < 0 || nidx as usize >= kids.len() {
                return false;
            }
            (parent, kids[nidx as usize])
        };
        if !self.workspace.borrow_mut().swap_with_neighbor(id, delta) {
            return false;
        }
        let mut map = self.page_order.borrow_mut();
        let a = map.get(&id).copied().unwrap_or(OrderKey::FIRST);
        let b = map.get(&neighbor).copied().unwrap_or(OrderKey::FIRST);
        map.insert(id, b);
        map.insert(neighbor, a);
        drop(map);
        self.record(vec![
            Change::PageMoved {
                id: PageId(id as u32 as u64),
                parent: parent.map(|v| PageId(v as u32 as u64)),
                order: b,
            },
            Change::PageMoved {
                id: PageId(neighbor as u32 as u64),
                parent: parent.map(|v| PageId(v as u32 as u64)),
                order: a,
            },
        ]);
        self.rebuild_sidebar();
        true
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
                // the blob scan has no block addressing; only FTS hits do
                hit_block_id: -1,
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
                swatch: -1,
                swatch_bg: false,
                check: false,
            },
            MenuRow {
                id: MENU_RENAME,
                label: "Rename".into(),
                icon: "pencil".into(),
                danger: false,
                swatch: -1,
                swatch_bg: false,
                check: false,
            },
            MenuRow {
                id: MENU_DUPLICATE,
                label: "Duplicate".into(),
                icon: "copy".into(),
                danger: false,
                swatch: -1,
                swatch_bg: false,
                check: false,
            },
            MenuRow {
                id: MENU_MOVE_UP,
                label: "Move up".into(),
                icon: "chevron-up".into(),
                danger: false,
                swatch: -1,
                swatch_bg: false,
                check: false,
            },
            MenuRow {
                id: MENU_MOVE_DOWN,
                label: "Move down".into(),
                icon: "chevron-down".into(),
                danger: false,
                swatch: -1,
                swatch_bg: false,
                check: false,
            },
            MenuRow {
                id: MENU_MOVE_TO,
                label: "Move to".into(),
                icon: "arrow-right".into(),
                danger: false,
                swatch: -1,
                swatch_bg: false,
                check: false,
            },
            MenuRow {
                id: MENU_FAVORITE,
                label: fav_label.into(),
                icon: "star".into(),
                danger: false,
                swatch: -1,
                swatch_bg: false,
                check: false,
            },
            MenuRow {
                id: MENU_DELETE,
                label: "Delete".into(),
                icon: "trash".into(),
                danger: true,
                swatch: -1,
                swatch_bg: false,
                check: false,
            },
        ];
        self.menu.set_vec(rows);
    }

    /// Second-level "Move to" menu for a page: every legal target — any page
    /// that is not the moved page and not inside its subtree (the walk skips
    /// the whole branch, so a cycle is impossible by construction) — plus
    /// "Top level" for moving to the root. Swaps the rows and keeps the
    /// popup open, like the block menu's mover.
    pub fn fill_page_menu_move_to(&self, id: i32) {
        let mut rows = vec![row(
            MENU_BACK,
            "Back",
            "chevron-left",
            false,
            -1,
            false,
        )];
        rows.push(row(
            PAGE_MOVE_TO_ROOT,
            "Top level",
            "export",
            false,
            -1,
            false,
        ));
        let ws = self.workspace.borrow();
        fn walk(
            ws: &Workspace,
            parent: Option<i32>,
            depth: usize,
            skip: i32,
            out: &mut Vec<MenuRow>,
        ) {
            for cid in ws.children_of(parent) {
                if cid == skip {
                    continue;
                }
                let indent = "\u{2003}".repeat(depth);
                let title = ws.title_of(cid).unwrap_or("Untitled");
                out.push(row(
                    PAGE_MOVE_TO_BASE + cid,
                    format!("{indent}{title}"),
                    "page",
                    false,
                    -1,
                    false,
                ));
                walk(ws, Some(cid), depth + 1, skip, out);
            }
        }
        walk(&ws, None, 0, id, &mut rows);
        drop(ws);
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
pub const MENU_MOVE_UP: i32 = 6;
pub const MENU_MOVE_DOWN: i32 = 7;
pub const MENU_MOVE_TO: i32 = 8;
pub const MENU_BACK: i32 = 9;
/// "Top level" target of the page-menu Move-to submenu (root, `None` parent).
pub const PAGE_MOVE_TO_ROOT: i32 = 499_999;
/// Page-menu Move-to targets encode the destination page above this base.
pub const PAGE_MOVE_TO_BASE: i32 = 500_000;
/// SidebarNode id of the Workspace section header — the drag-drop target
/// that moves a page to the top level. Page rows target themselves.
pub const WORKSPACE_HEADER_ID: i32 = -100;

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
/// renders labels. ids are BlockKind ints (see kind_from_int). Kinds with a
/// markdown line-shortcut ("# ", "- ", …) are deliberately absent — typing
/// the symbol converts, so the menu only lists the rest (ADR-0022).
const SLASH_ITEMS: &[(BlockKind, &str, &str)] = &[
    (BlockKind::Paragraph, "Text", "Plain paragraph"),
    (BlockKind::Toggle, "Toggle list", "Collapsible section"),
    (BlockKind::Image, "Image", "Embed a picture from a file"),
    (BlockKind::File, "File", "Attach a file of any type"),
    (BlockKind::Table, "Table", "Simple grid of cells"),
    (BlockKind::Columns, "Columns", "Two columns of blocks, side by side"),
    (BlockKind::Callout, "Callout", "Highlighted box with an emoji"),
    (BlockKind::Code, "Code", "Monospaced block — or type ```"),
    (BlockKind::Math, "Math", "LaTeX formula — or type $$"),
    (
        BlockKind::Toc,
        "Table of contents",
        "Links to this page's headings",
    ),
    (BlockKind::Divider, "Divider", "Visual separator — or type ---"),
];

/// "Turn into" targets: the same curation as the slash menu.
const TURN_INTO_ITEMS: &[(BlockKind, &str, &str)] = SLASH_ITEMS;

/// Insert-menu ("+" handle) descriptors: the full Notion-style list, unlike
/// the curated "/" menu (ADR-0022). Rows whose id is a BlockKind int are
/// insertable today; id < 0 marks the kinds still on the roadmap (SPEC
/// §三十七, §三十九) as disabled placeholders, so the menu shape matches
/// Notion and the roadmap stays visible. Keyboard navigation skips
/// placeholders and applying to one is a no-op.
const INSERT_ITEMS: &[(i32, &str, &str)] = &[
    (kind_to_int(BlockKind::Paragraph), "Text", "Plain paragraph"),
    (kind_to_int(BlockKind::Page), "Page", "Embed a child page"),
    (kind_to_int(BlockKind::Link), "Link to page", "Point at an existing page"),
    (kind_to_int(BlockKind::Image), "Image", "Embed a picture from a file"),
    (kind_to_int(BlockKind::File), "File", "Attach a file of any type"),
    (kind_to_int(BlockKind::Todo), "To-do list", "Track tasks with a checkbox"),
    (kind_to_int(BlockKind::Heading1), "Heading 1", "Big section heading"),
    (kind_to_int(BlockKind::Heading2), "Heading 2", "Medium section heading"),
    (kind_to_int(BlockKind::Heading3), "Heading 3", "Small section heading"),
    (kind_to_int(BlockKind::Table), "Table", "Simple grid of cells"),
    (kind_to_int(BlockKind::Columns), "Columns", "Side-by-side columns"),
    (kind_to_int(BlockKind::Bullet), "Bulleted list", "Simple bulleted list"),
    (kind_to_int(BlockKind::Numbered), "Numbered list", "Ordered list"),
    (kind_to_int(BlockKind::Toggle), "Toggle list", "Collapsible section"),
    (kind_to_int(BlockKind::Quote), "Quote", "Capture a quote"),
    (kind_to_int(BlockKind::Divider), "Divider", "Visual separator"),
    (kind_to_int(BlockKind::Callout), "Callout", "Highlighted box with an emoji"),
    (kind_to_int(BlockKind::Code), "Code", "Monospaced block"),
    (kind_to_int(BlockKind::Math), "Math", "LaTeX formula, rendered as Unicode"),
    (
        kind_to_int(BlockKind::Toc),
        "Table of contents",
        "Links to this page's headings",
    ),
    (-1, "Table view", "Database table · later"),
    (-1, "Board", "Board view · later"),
    (-1, "Gallery", "Gallery view · later"),
    (-1, "List view", "Database list · later"),
    (-1, "Calendar", "Calendar view · later"),
    (-1, "Timeline", "Timeline view · later"),
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
            disabled: false,
        })
        .collect()
}

/// MenuRow constructor for the ⋮⋮ menu fillers (`swatch < 0` = no swatch).
#[allow(clippy::too_many_arguments)]
fn row(id: i32, label: impl AsRef<str>, icon: &str, danger: bool, swatch: i32, swatch_bg: bool) -> MenuRow {
    MenuRow {
        id,
        label: label.as_ref().into(),
        icon: icon.into(),
        danger,
        swatch,
        swatch_bg,
        check: false,
    }
}

/// BlockKind int (UI menu ids) -> kind. Public: the controller resolves
/// Turn-into menu actions with it.
pub fn kind_from_int(kind: i32) -> BlockKind {
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
        10 => BlockKind::Callout,
        11 => BlockKind::Page,
        12 => BlockKind::Link,
        13 => BlockKind::Toggle,
        14 => BlockKind::Image,
        15 => BlockKind::File,
        16 => BlockKind::Table,
        17 => BlockKind::TableCell,
        18 => BlockKind::Columns,
        19 => BlockKind::Column,
        20 => BlockKind::Math,
        21 => BlockKind::Toc,
        _ => BlockKind::Paragraph,
    }
}

const fn kind_to_int(kind: BlockKind) -> i32 {
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
        BlockKind::Callout => 10,
        BlockKind::Page => 11,
        BlockKind::Link => 12,
        BlockKind::Toggle => 13,
        BlockKind::Image => 14,
        BlockKind::File => 15,
        BlockKind::Table => 16,
        BlockKind::TableCell => 17,
        BlockKind::Columns => 18,
        BlockKind::Column => 19,
        BlockKind::Math => 20,
        BlockKind::Toc => 21,
        BlockKind::Paragraph => 0,
    }
}

/// Nesting depth of one block (ancestors within the same page), bounded —
/// M4 renders a single indent level.
fn block_depth(blocks: &[Block], b: &Block) -> i32 {
    let mut depth = 0i32;
    let mut parent = b.parent;
    while let Some(pid) = parent {
        depth += 1;
        if depth >= 4 {
            break;
        }
        match blocks.iter().find(|x| x.id == pid) {
            Some(x) => parent = x.parent,
            None => break,
        }
    }
    depth
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
                color: ColorKind::Default,
                background: ColorKind::Default,
                page_ref: None,
                folded: false,
                attachment: None,
                img_percent: 100,
                columns: 0,
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
            // a formula run shows its glyphs, not its source: the document
            // keeps `\alpha`, the row shows α. Converted here rather than in
            // the delegate, so the cost is one pass per projection and not one
            // per binding evaluation.
            let math = marks
                .iter()
                .any(|m| m.kind == crate::core::MarkKind::Math && m.start <= s && m.end >= e);
            let run_text = &text[s..e];
            Some(TextRun {
                text: if math {
                    crate::core::math::to_unicode(run_text).into()
                } else {
                    run_text.into()
                },
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

/// True when `b` gets no editor row at all. Two things hide a subtree
/// (SPEC §三十七, ADR-0028): a folded block anywhere above it, and a container
/// whose delegate draws its children itself — a table's grid, a columns
/// layout's boxes — so they must not also cost a row each. The page's blocks
/// are pre-order, so an ancestor walk is enough (no set to maintain).
fn hidden_by_ancestor(blocks: &[Block], b: &Block) -> bool {
    if matches!(b.kind, BlockKind::TableCell | BlockKind::Column) {
        return true;
    }
    let mut parent = b.parent;
    let mut guard = 0;
    while let Some(pid) = parent {
        let Some(p) = blocks.iter().find(|x| x.id == pid) else {
            break;
        };
        if p.folded || matches!(p.kind, BlockKind::Table | BlockKind::Columns | BlockKind::Column)
        {
            return true;
        }
        parent = p.parent;
        guard += 1;
        if guard >= 64 {
            break;
        }
    }
    false
}

/// Positions (into the page's block list) of the blocks that get an editor
/// row: a folded block stays, its whole subtree does not, and so does a
/// table's grid. SPEC §三十七 is explicit that hiding a collapsed section
/// costs real rows rather than `visible: false` delegates, so every row-index
/// consumer shares this one list — see `drop_index_for_row`.
pub fn visible_block_indices(blocks: &[Block]) -> Vec<usize> {
    blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| !hidden_by_ancestor(blocks, b))
        .map(|(i, _)| i)
        .collect()
}

/// A table as the editor sees it: its cells in row-major order and the column
/// count. Cells are child blocks, and a page's block list is display order, so
/// filtering it yields the grid — the same reading `command::grid` uses.
struct LiveGrid {
    table: i32,
    cells: Vec<BlockId>,
    cols: usize,
}

/// A columns layout as the editor sees it: every block inside its boxes, in
/// reading order — the same list the row's `column-items` carries, built by
/// the same walk, so focus and the model cannot disagree about what is there.
struct LiveLayout {
    layout: i32,
    items: Vec<BlockId>,
}

impl LiveLayout {
    fn of(doc: &Document, id: BlockId) -> Option<Self> {
        let b = doc.block(id)?;
        if b.kind != BlockKind::Columns || b.columns == 0 {
            // a layout with no boxes has no shape to edit, and the commands
            // refuse it the same way a degenerate grid is refused
            return None;
        }
        let blocks = doc.page_blocks(b.page);
        let items = layout_slots(blocks, b)
            .into_iter()
            .map(|s| blocks[s.index].id)
            .collect();
        Some(Self { layout: id.as_u64() as i32, items })
    }
}

impl LiveGrid {
    fn of(doc: &Document, id: BlockId) -> Option<Self> {
        let b = doc.block(id)?;
        let cols = b.columns as usize;
        if b.kind != BlockKind::Table || cols == 0 {
            return None;
        }
        let cells = grid_blocks(doc.page_blocks(b.page), b)
            .into_iter()
            .map(|c| c.id)
            .collect();
        Some(Self { table: id.as_u64() as i32, cells, cols })
    }

    fn rows(&self) -> usize {
        self.cells.len() / self.cols
    }
}

/// A table's cells in row-major order — child blocks, and a page's block list
/// is display order, so filtering it yields the grid. The same reading
/// `command::grid` makes.
fn grid_blocks<'a>(blocks: &'a [Block], table: &Block) -> Vec<&'a Block> {
    let mut cells: Vec<&Block> = blocks
        .iter()
        .filter(|b| b.parent == Some(table.id) && b.kind == BlockKind::TableCell)
        .collect();
    // one table's cells live on one page, where the list is already display
    // order; the sort says so out loud
    cells.sort_by_key(|b| b.order);
    // whole rows only: the delegate chunks this list by `columns` and indexes
    // into it, so a ragged tail would be read out of range. Such a grid is not
    // editable either — `command::grid` refuses it — so the stray cells stay
    // in the document, unseen, rather than rendering as a broken row.
    let cols = table.columns.max(1) as usize;
    cells.truncate(cells.len() - cells.len() % cols);
    cells
}

/// The page's headings, as one `Toc` row's data. `shown` is the row list the
/// projection itself just built, so a heading hidden by a fold or by a
/// container's delegate stays out of the contents too — a link you cannot
/// scroll to is worse than no link. Reading order is the page's own.
fn toc_entries(blocks: &[Block], shown: &[usize]) -> Vec<TocEntry> {
    shown
        .iter()
        .filter_map(|&i| {
            let b = &blocks[i];
            let level = b.kind.heading_level()?;
            Some(TocEntry {
                block: b.id.0 as i32,
                // an empty heading still has a row to land on, so it still gets
                // an entry; a blank line is nothing to click
                label: if b.text.is_empty() {
                    "Untitled".into()
                } else {
                    b.text.clone().into()
                },
                level: level as i32,
            })
        })
        .collect()
}

/// The cells of one table, as the row's data.
fn table_cells(blocks: &[Block], table: &Block) -> Vec<TableCell> {
    grid_blocks(blocks, table)
        .into_iter()
        .map(|c| TableCell {
            id: c.id.0 as i32,
            text: c.text.clone().into(),
            runs: runs_to_model(c),
        })
        .collect()
}

/// One block of a columns layout, placed in the flat list the delegate draws.
struct ColumnSlot {
    /// which box, 0-based
    column: i32,
    /// how far inside that box (0 = a box's own child), for the indent
    depth: i32,
    /// position in the page's block list
    index: usize,
    /// has children, so the toggle chevron shows
    can_fold: bool,
}

/// A block's direct children, as positions in the page list. The list is kept
/// sorted by order key, so the sort is only saying so out loud.
fn child_indices(blocks: &[Block], parent: BlockId) -> Vec<usize> {
    let mut v: Vec<usize> = blocks
        .iter()
        .enumerate()
        .filter(|(_, b)| b.parent == Some(parent))
        .map(|(i, _)| i)
        .collect();
    v.sort_by_key(|i| blocks[*i].order);
    v
}

/// Emit `i` and everything below it, in reading order. A folded block keeps
/// its own slot and loses its subtree, exactly as the row list does.
fn column_slots(blocks: &[Block], i: usize, column: i32, depth: i32, out: &mut Vec<ColumnSlot>) {
    let b = &blocks[i];
    let kids = child_indices(blocks, b.id);
    out.push(ColumnSlot { column, depth, index: i, can_fold: !kids.is_empty() });
    // the depth cap is only a corrupt-data guard: nothing in the app nests
    // blocks eight deep inside one box
    if b.folded || depth >= 8 {
        return;
    }
    for k in kids {
        if blocks[k].kind == BlockKind::Column {
            continue; // a box is a container, never content
        }
        column_slots(blocks, k, column, depth + 1, out);
    }
}

/// A layout's content in reading order: every box's blocks, left to right.
/// The boxes are the layout's `Column` children — the same reading
/// `command::column_blocks` makes.
fn layout_slots(blocks: &[Block], layout: &Block) -> Vec<ColumnSlot> {
    let mut out = Vec::new();
    let mut box_n = 0i32;
    for c in child_indices(blocks, layout.id) {
        let b = &blocks[c];
        if b.kind != BlockKind::Column {
            // content hanging off the layout itself rather than a box has no
            // box to name, so it reads as the first one's
            column_slots(blocks, c, box_n, 0, &mut out);
            continue;
        }
        for k in child_indices(blocks, b.id) {
            if blocks[k].kind == BlockKind::Column {
                continue;
            }
            column_slots(blocks, k, box_n, 0, &mut out);
        }
        box_n += 1;
    }
    out
}

/// The blocks inside one layout, as the row's data, and its boxes as the
/// delegate sees them. Numbering restarts per box, which is what a reader
/// sees; the boxes carry where each group starts in the flat item list, since
/// Slint has no recursive component to work it out itself.
fn column_projection(
    blocks: &[Block],
    layout: &Block,
) -> (Vec<ColumnItem>, Vec<ColumnBox>) {
    // the layout's own boxes, left to right — the same reading
    // `command::column_blocks` makes
    let mut box_ids: Vec<BlockId> = Vec::new();
    for c in child_indices(blocks, layout.id) {
        if blocks[c].kind == BlockKind::Column {
            box_ids.push(blocks[c].id);
        }
    }
    let mut sizes = vec![0i32; box_ids.len()];
    let mut items = Vec::new();
    let mut numbers: Vec<(i32, i32)> = Vec::new();
    for s in layout_slots(blocks, layout) {
        let b = &blocks[s.index];
        let number = if b.kind == BlockKind::Numbered {
            match numbers.iter_mut().find(|(c, _)| *c == s.column) {
                Some((_, n)) => {
                    *n += 1;
                    *n
                }
                None => {
                    numbers.push((s.column, 1));
                    1
                }
            }
        } else {
            0
        };
        if let Some(n) = sizes.get_mut(s.column as usize) {
            *n += 1;
        }
        items.push(ColumnItem {
            id: b.id.0 as i32,
            column: s.column,
            depth: s.depth,
            kind: kind_to_int(b.kind),
            text: b.text.clone().into(),
            runs: runs_to_model(b),
            checked: b.checked,
            number,
            folded: b.folded,
            can_fold: s.can_fold,
            color: b.color.slot(),
            bg: b.background.slot(),
            attachment: b.attachment.map(|a| a.as_u64() as i32).unwrap_or(0),
        });
    }
    // `layout_slots` walks box by box, so the groups are already contiguous
    let mut first = 0i32;
    let boxes = box_ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let b = ColumnBox {
                id: id.0 as i32,
                column: i as i32,
                first,
                size: sizes[i],
            };
            first += sizes[i];
            b
        })
        .collect();
    (items, boxes)
}

/// Project a page's blocks into editor rows: numbered items renumbered by
/// position, the last row flagged as the tail spacer carrier. Folded
/// subtrees are left out entirely.
pub fn project_blocks(blocks: &[Block]) -> Vec<BlockRow> {
    let shown = visible_block_indices(blocks);
    let mut out: Vec<BlockRow> = shown
        .iter()
        .map(|&i| {
            let b = &blocks[i];
            // one walk per layout row, at most: the pair is built together
            let (column_items, column_boxes) = if b.kind == BlockKind::Columns {
                let (items, boxes) = column_projection(blocks, b);
                (
                    slint::ModelRc::from(Rc::new(VecModel::from(items))),
                    slint::ModelRc::from(Rc::new(VecModel::from(boxes))),
                )
            } else {
                (ModelRc::default(), ModelRc::default())
            };
            BlockRow {
                id: b.id.0 as i32,
                kind: kind_to_int(b.kind),
                text: b.text.clone().into(),
                checked: b.checked,
                number: 0,
                tail: false,
                runs: runs_to_model(b),
                depth: block_depth(blocks, b),
                color: b.color.slot(),
                bg: b.background.slot(),
                page_ref: b.page_ref.map(|p| p.as_u64() as i32).unwrap_or(-1),
                folded: b.folded,
                // 0 = none: the UI resolves an id through the attachment cache
                attachment: b.attachment.map(|a| a.as_u64() as i32).unwrap_or(0),
                img_percent: b.img_percent as i32,
                // any block can be a parent; EditorBlock only draws the
                // chevron for a Toggle, and a table's children are its grid
                // and a layout's its boxes, which no fold can reveal
                can_fold: blocks.iter().any(|x| x.parent == Some(b.id))
                    && !matches!(b.kind, BlockKind::Table | BlockKind::Columns),
                columns: b.columns as i32,
                // the guards are the whole cost of these fields: both walks
                // scan the page, so calling them for every row would make the
                // projection quadratic on a 10 000-block page
                table_cells: if b.kind == BlockKind::Table {
                    slint::ModelRc::from(Rc::new(VecModel::from(table_cells(blocks, b))))
                } else {
                    ModelRc::default()
                },
                // the same guard as above: a contents walk is a page scan, and
                // a page with one TOC block has exactly one row that wants it
                toc_entries: if b.kind == BlockKind::Toc {
                    slint::ModelRc::from(Rc::new(VecModel::from(toc_entries(blocks, &shown))))
                } else {
                    ModelRc::default()
                },
                column_items, column_boxes,
            }
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
pub const BLOCK_CALLOUT: i32 = 10;
pub const BLOCK_PAGE: i32 = 11;
pub const BLOCK_LINK: i32 = 12;
pub const BLOCK_TOGGLE: i32 = 13;
pub const BLOCK_IMAGE: i32 = 14;
pub const BLOCK_FILE: i32 = 15;
pub const BLOCK_TABLE: i32 = 16;
pub const BLOCK_TABLE_CELL: i32 = 17;
pub const BLOCK_COLUMNS: i32 = 18;
pub const BLOCK_COLUMN: i32 = 19;
pub const BLOCK_MATH: i32 = 20;
pub const BLOCK_TOC: i32 = 21;

fn block(kind: i32, text: &str) -> BlockRow {
    BlockRow {
        id: 0,
        kind,
        text: text.into(),
        checked: false,
        number: 0,
        tail: false,
        runs: ModelRc::from(Rc::new(VecModel::from(Vec::new()))),
        depth: 0,
        color: 0,
        bg: 0,
        page_ref: -1,
        folded: false,
        attachment: 0,
        img_percent: 100,
        can_fold: false,
        columns: 0,
        table_cells: ModelRc::from(Rc::new(VecModel::from(Vec::new()))),
        column_items: ModelRc::from(Rc::new(VecModel::from(Vec::new()))),
        column_boxes: ModelRc::from(Rc::new(VecModel::from(Vec::new()))),
        toc_entries: ModelRc::from(Rc::new(VecModel::from(Vec::new()))),
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
        block(BLOCK_TODO, "Slash menu — type \"/\" at the start of a line"),
        block(BLOCK_H2, "Try the interactions"),
        block(
            BLOCK_PARAGRAPH,
            "Everything below already works: select text and press Ctrl+B / Ctrl+I / Ctrl+E for bold, italic, and inline code; Ctrl+L links it; Tab indents a list item and Shift+Tab promotes it back.",
        ),
        block(BLOCK_BULLET, "Ctrl+P searches every page — titles and content"),
        block(BLOCK_BULLET, "Ctrl+K opens the command palette"),
        block(BLOCK_BULLET, "Right-click a page in the sidebar for its menu"),
        block(BLOCK_BULLET, "Click this page's big title to rename it in place"),
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
            "This page is empty. Start typing, or use the + handle beside any block to insert one — the slash menu offers every kind Quire knows.",
        ),
        block(BLOCK_DIVIDER, ""),
        block(
            BLOCK_PARAGRAPH,
            "Use the sidebar to create, rename, duplicate, and delete pages — everything you type is written to a local SQLite library and is there again after a restart.",
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

/// How many distinct pictures a bench page of `--pictures N` stores. Rows
/// reuse the pool *spread across the page* (image row k takes fixture
/// `k % pool`), so consecutive picture rows still carry different rasters —
/// which is what a photo page does, and what the decode cache is sized for —
/// while the seed pass stays short and the folder stays under ~40 MB.
const BENCH_FIXTURE_POOL: usize = 200;

/// `(stride, pool)` for a bench page: one picture row every `stride` blocks,
/// drawn from `pool` fixtures. Both the row builder and the seeder ask, so
/// neither can drift from the other.
fn bench_picture_plan(rows: usize, pictures: usize) -> (usize, usize) {
    if pictures == 0 || rows == 0 {
        return (0, 0);
    }
    let n = pictures.min(rows);
    ((rows / n).max(1), n.min(BENCH_FIXTURE_POOL))
}

/// Turn every `stride`-th row of the bench page into an image block.
fn bench_pictures(blocks: &mut [Block], pictures: usize) {
    let (stride, pool) = bench_picture_plan(blocks.len(), pictures);
    if stride == 0 {
        return;
    }
    for (i, b) in blocks.iter_mut().enumerate() {
        if i % stride != stride - 1 {
            continue;
        }
        b.kind = BlockKind::Image;
        b.text = String::new();
        b.attachment = Some(AttachmentId(((i / stride) % pool) as u64 + 1));
    }
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
/// Copy the open page's markdown onto the clipboard (FFI write, ADR-0025).
pub const CMD_COPY_MD: i32 = 11;
/// Retrace / re-advance the session's page navigation (SPEC §十六).
pub const CMD_NAV_BACK: i32 = 12;
pub const CMD_NAV_FORWARD: i32 = 13;
/// Jump-to-page commands are 10 000 + page id.
pub const CMD_PAGE_BASE: i32 = 10_000;

/// What a palette row means, resolved from its id in exactly one place.
///
/// The controller matches on this instead of on the raw id, and that match has
/// no wildcard arm: the palette dispatch used to be a `match id` over numeric
/// literals, and one constant whose import was missing turned its arm into a
/// catch-all *binding* — every command with id >= 9 silently ran `CopyMd`
/// (`aaa3763`). rustc warned, and nothing tested it. A row that maps to no
/// action is now a compile error at the dispatch and a failed assertion here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaletteAction {
    NewPage,
    SearchPages,
    ToggleSidebar,
    ToggleTheme,
    Settings,
    RenamePage,
    DuplicatePage,
    DeletePage,
    ExportMarkdown,
    ImportMarkdown,
    CopyMarkdown,
    NavigateBack,
    NavigateForward,
    OpenPage(i32),
    None,
}

pub fn palette_action(id: i32) -> PaletteAction {
    match id {
        CMD_NEW_PAGE => PaletteAction::NewPage,
        CMD_SEARCH => PaletteAction::SearchPages,
        CMD_TOGGLE_SIDEBAR => PaletteAction::ToggleSidebar,
        CMD_TOGGLE_THEME => PaletteAction::ToggleTheme,
        CMD_SETTINGS => PaletteAction::Settings,
        CMD_RENAME_PAGE => PaletteAction::RenamePage,
        CMD_DUPLICATE_PAGE => PaletteAction::DuplicatePage,
        CMD_DELETE_PAGE => PaletteAction::DeletePage,
        CMD_EXPORT_PAGE => PaletteAction::ExportMarkdown,
        CMD_IMPORT_MD => PaletteAction::ImportMarkdown,
        CMD_COPY_MD => PaletteAction::CopyMarkdown,
        CMD_NAV_BACK => PaletteAction::NavigateBack,
        CMD_NAV_FORWARD => PaletteAction::NavigateForward,
        page if page >= CMD_PAGE_BASE => PaletteAction::OpenPage(page - CMD_PAGE_BASE),
        _ => PaletteAction::None,
    }
}

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
        "Ctrl+\\",
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
    cmd(
        CMD_COPY_MD,
        "Copy Page as Markdown",
        "",
        "Page",
        "copy",
    );
    cmd(CMD_NAV_BACK, "Go Back", "Alt+Left", "Navigate", "arrow-left");
    cmd(
        CMD_NAV_FORWARD,
        "Go Forward",
        "Alt+Right",
        "Navigate",
        "arrow-right",
    );
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

#[cfg(test)]
mod tests {
    use super::{
        mock_commands, palette_action, NavHistory, CMD_NAV_BACK, CMD_NAV_FORWARD,
        CMD_PAGE_BASE, NAV_MAX, PaletteAction,
    };
    use crate::app::workspace::Workspace;

    /// A block in display order, page 1, no marks — the shape the projections
    /// below read.
    fn blk(
        id: u64,
        parent: Option<u64>,
        order: u64,
        kind: crate::core::BlockKind,
        folded: bool,
        text: &str,
    ) -> crate::core::Block {
        use crate::core::{Block, BlockId, ColorKind, OrderKey, PageId};
        Block {
            id: BlockId(id),
            page: PageId(1),
            parent: parent.map(|p| BlockId(p)),
            order: OrderKey(order),
            kind,
            text: text.into(),
            checked: false,
            marks: Vec::new(),
            color: ColorKind::Default,
            background: ColorKind::Default,
            page_ref: None,
            folded,
            attachment: None,
            img_percent: 100,
            columns: 0,
        }
    }

    /// A page's blocks in display order: a folded toggle with two children
    /// (one of them a grandchild of the other) and a trailing paragraph.
    fn fold_scene() -> Vec<crate::core::Block> {
        use crate::core::BlockKind;
        vec![
            blk(1, None, 10, BlockKind::Toggle, true, "section"),
            blk(2, Some(1), 11, BlockKind::Paragraph, false, "a"),
            blk(3, Some(2), 12, BlockKind::Bullet, false, "a/1"),
            blk(4, None, 13, BlockKind::Toggle, false, "open section"),
            blk(5, Some(4), 14, BlockKind::Paragraph, false, "b"),
            blk(6, None, 15, BlockKind::Paragraph, false, "tail"),
        ]
    }

    /// What a `Toc` row's list currently says, as (block, label, indent).
    fn toc_of(row: &crate::BlockRow) -> Vec<(i32, String, i32)> {
        use slint::Model as _;
        (0..row.toc_entries.row_count())
            .map(|i| {
                let e = row.toc_entries.row_data(i).unwrap();
                (e.block, e.label.to_string(), e.level)
            })
            .collect()
    }

    #[test]
    fn a_folded_subtree_produces_no_rows_at_all() {
        let blocks = fold_scene();
        let rows = super::project_blocks(&blocks);
        // SPEC §三十七: the collapsed section costs real rows, not hidden
        // delegates — block 2 and its own child 3 drop out with their parent.
        let ids: Vec<i32> = rows.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![1, 4, 5, 6]);
        assert_eq!(super::visible_block_indices(&blocks), vec![0, 3, 4, 5]);
    }

    #[test]
    fn unfolding_returns_the_subtree_in_its_source_order() {
        let mut blocks = fold_scene();
        blocks[0].folded = false;
        let ids: Vec<i32> = super::project_blocks(&blocks).iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn the_fold_flag_reports_children_not_kind() {
        let blocks = fold_scene();
        let rows = super::project_blocks(&blocks);
        assert!(rows.iter().find(|r| r.id == 1).unwrap().can_fold);
        assert!(rows.iter().find(|r| r.id == 4).unwrap().can_fold);
        // a section with nothing in it reports false: no chevron to click.
        // The stored flag stays whatever it was — it just hides nothing.
        let lone = vec![{
            let mut b = blocks[0].clone();
            b.id = crate::core::BlockId(9);
            b.parent = None;
            b
        }];
        let rows = super::project_blocks(&lone);
        assert!(!rows[0].can_fold, "toggle with no children");
        assert_eq!(rows[0].kind, super::BLOCK_TOGGLE);
        assert!(rows[0].folded, "the flag rides along with the block");
    }

    #[test]
    fn hidden_rows_do_not_rename_the_numbered_list() {
        use crate::core::BlockKind;
        let mut blocks = fold_scene();
        blocks[0].kind = BlockKind::Numbered;
        blocks[1].kind = BlockKind::Numbered;
        blocks[2].kind = BlockKind::Numbered;
        blocks[5].kind = BlockKind::Numbered;
        let rows = super::project_blocks(&blocks);
        // 2 and 3 are inside the fold: the visible list is 1 then 2, not 4
        let nums: Vec<i32> = rows.iter().filter(|r| r.kind == 5).map(|r| r.number).collect();
        assert_eq!(nums, vec![1, 2]);
    }

    /// Pages 1..=9 are alive; anything else was deleted.
    fn live(id: i32) -> bool {
        (1..=9).contains(&id)
    }

    #[test]
    fn a_contents_row_lists_the_headings_the_page_can_show() {
        use crate::core::BlockKind;
        let blocks = vec![
            blk(1, None, 10, BlockKind::Toc, false, ""),
            blk(2, None, 11, BlockKind::Heading1, false, "Top"),
            blk(3, None, 12, BlockKind::Toggle, true, "Hidden section"),
            blk(4, Some(3), 13, BlockKind::Heading2, false, "Inside the fold"),
            blk(5, None, 14, BlockKind::Heading2, false, "Second"),
            blk(6, None, 15, BlockKind::Heading3, false, ""),
            blk(7, None, 16, BlockKind::Paragraph, false, "prose"),
        ];
        let rows = super::project_blocks(&blocks);
        let toc = rows
            .iter()
            .find(|r| r.kind == super::BLOCK_TOC)
            .expect("a toc row");
        // 4 is behind its folded parent: the list is built from the rows the
        // projection itself made, so a heading nobody can reach is not here.
        // An untitled heading still gets a line, the same word a tab shows.
        assert_eq!(
            toc_of(toc),
            vec![
                (2, "Top".into(), 1),
                (5, "Second".into(), 2),
                (6, "Untitled".into(), 3),
            ]
        );
        // …and the walk runs for the one row that asks for it
        assert!(
            rows.iter()
                .filter(|r| r.kind != super::BLOCK_TOC)
                .all(|r| toc_of(r).is_empty()),
            "a paragraph carries a contents list"
        );
    }

    #[test]
    fn renaming_a_heading_renames_its_line_because_nothing_was_copied() {
        use crate::core::BlockKind;
        let mut blocks = vec![
            blk(1, None, 10, BlockKind::Toc, false, ""),
            blk(2, None, 11, BlockKind::Heading2, false, "Before"),
        ];
        assert_eq!(toc_of(&super::project_blocks(&blocks)[0])[0].1, "Before");
        blocks[1].text = "After".into();
        assert_eq!(toc_of(&super::project_blocks(&blocks)[0])[0].1, "After");
        // the block's own row keeps whatever text it was made from, like a
        // divider does — painted by no one, and the list never read it
        blocks[0].text = "stale".into();
        assert_eq!(toc_of(&super::project_blocks(&blocks)[0])[0].1, "After");
    }

    /// What one contents block adds to a projection, on the shape the RAM gate
    /// measures: 10 000 rows, a heading every tenth, release profile.
    #[test]
    #[ignore = "prints a timing; run with --release"]
    fn cost_of_one_contents_block_on_a_ten_thousand_row_page() {
        use crate::core::BlockKind;
        use std::time::Instant;
        let rounds = 50u32;
        let build = |toc: bool| -> Vec<crate::core::Block> {
            (0..10_000u64)
                .map(|i| {
                    let kind = if toc && i == 0 {
                        BlockKind::Toc
                    } else if i % 10 == 5 {
                        BlockKind::Heading2
                    } else {
                        BlockKind::Paragraph
                    };
                    blk(i + 2, None, i, kind, false, "a line of the bench page")
                })
                .collect()
        };
        let time = |blocks: &[crate::core::Block]| {
            let t = Instant::now();
            for _ in 0..rounds {
                std::hint::black_box(super::project_blocks(blocks).len());
            }
            t.elapsed().as_secs_f64() * 1e3 / rounds as f64
        };
        let (without, with) = (time(&build(false)), time(&build(true)));
        println!(
            "projection: no contents block {without:.3} ms, one on 10 000 rows {with:.3} ms \
             (+{:.3} ms, 1 000 headings)",
            with - without
        );
    }

    /// SPEC §十六 names these two as palette commands, and the id a row
    /// carries is exactly what the dispatch matches on — a drifted row is a
    /// silently dead command, which is how the id >= 9 shadowing bug hid.
    #[test]
    fn the_palette_carries_the_navigation_commands() {
        let cmds = mock_commands(&Workspace::sample());
        for (id, name) in [(CMD_NAV_BACK, "Go Back"), (CMD_NAV_FORWARD, "Go Forward")] {
            let row = cmds.iter().find(|c| c.id == id).expect("row present");
            assert_eq!(row.name.as_str(), name);
            assert_eq!(row.section.as_str(), "Navigate");
        }
    }

    /// The whole registry, walked: every row must resolve to an action of its
    /// own. This is the durable repair for `aaa3763`, where a missing import
    /// turned one `match` arm into a catch-all binding and every row with id
    /// >= 9 ran `Copy Page as Markdown` — green tests, because nothing walked
    /// the registry until now.
    #[test]
    fn every_palette_row_resolves_to_its_own_action() {
        let cmds = mock_commands(&Workspace::sample());
        assert!(!cmds.is_empty());
        let mut actions: Vec<PaletteAction> = Vec::new();
        for row in &cmds {
            let action = palette_action(row.id);
            assert_ne!(
                action,
                PaletteAction::None,
                "row {:?} (id {}) maps to no action",
                row.name.as_str(),
                row.id
            );
            if !matches!(action, PaletteAction::OpenPage(_)) {
                assert!(
                    !actions.contains(&action),
                    "two rows share the action {action:?} — one of them is dead"
                );
                actions.push(action);
            }
        }
        // the ids themselves are unique too, or the palette cannot select one
        let mut ids: Vec<i32> = cmds.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), before, "two palette rows carry the same id");
    }

    #[test]
    fn a_jump_row_carries_its_page_id_and_an_unknown_id_does_nothing() {
        assert_eq!(
            palette_action(CMD_PAGE_BASE + 42),
            PaletteAction::OpenPage(42)
        );
        // nothing between the command block and the page block, and nothing
        // the palette could invent, may reach a handler
        for id in [0, 14, 9_999, -1] {
            assert_eq!(palette_action(id), PaletteAction::None, "id {id}");
        }
    }

    #[test]
    fn back_retraces_and_forward_rewinds() {
        let mut nav = NavHistory::default();
        nav.record(1, 2);
        nav.record(2, 3);
        assert_eq!(nav.step(false, 3, live), Some(2));
        assert_eq!(nav.step(false, 2, live), Some(1));
        // nothing left behind the first page
        assert_eq!(nav.step(false, 1, live), None);
        assert_eq!(nav.step(true, 1, live), Some(2));
    }

    #[test]
    fn a_new_navigation_drops_the_forward_branch() {
        let mut nav = NavHistory::default();
        nav.record(1, 2);
        assert_eq!(nav.step(false, 2, live), Some(1));
        nav.record(1, 9);
        assert_eq!(nav.step(true, 9, live), None);
    }

    #[test]
    fn deleted_pages_are_skipped_not_opened() {
        let mut nav = NavHistory::default();
        nav.record(1, 2);
        // 99 sat in the history and has since been deleted
        nav.record(99, 3);
        assert_eq!(nav.step(false, 3, live), Some(1));
        // the page that was left stays reachable in the direction it came from
        assert_eq!(nav.step(true, 1, live), Some(3));
    }

    #[test]
    fn the_first_open_records_nothing_and_the_stack_stays_bounded() {
        let mut nav = NavHistory::default();
        nav.record(0, 1);
        assert_eq!(nav.step(false, 1, live), None);

        for i in 0..(NAV_MAX + 20) {
            nav.record((i % 9 + 1) as i32, (i % 9 + 2) as i32);
        }
        assert_eq!(nav.back.len(), NAV_MAX);
        for _ in 0..NAV_MAX {
            assert!(nav.step(false, 0, live).is_some());
        }
        assert_eq!(nav.step(false, 0, live), None);
    }

    /// §二十二's promise in one number: a page of photographs must not cost
    /// the session more decoded rasters than the budget allows.
    #[test]
    fn the_picture_cache_spends_its_budget_and_drops_the_stalest_first() {
        use super::{AppState, HandleArgs, MAX_ATTACHMENT_CACHE_BYTES};
        use crate::core::AttachmentId;

        let state = AppState::new(
            &HandleArgs { blocks: 0, auto_exit_secs: 0.0, bench_pages: 0, pictures: 0 },
            None,
        );
        // No database, so the store is the temp fallback; the high id range
        // keeps this fixture away from anything else writing there.
        let base: i64 = 900_000;

        // One pixel over MAX_EDGE, so the store hands the editor a 1280x720
        // raster = 3.7 MB. Eleven of them want 40 MB against a 32 MB budget.
        const COUNT: i64 = 11;
        let show = |state: &AppState, n: i64| {
            if !state.attachments.borrow().contains_key(&n) {
                let id = (base + n) as u64;
                let att = state.store.create_fixture(AttachmentId(id), 1281, 720).unwrap();
                assert_eq!(att.thumb, format!("{id}.cache.png"), "no downsample to measure");
                state.attachments.borrow_mut().insert(n, att);
            }
            assert!(
                state.image_for(n as i32).size().width > 0,
                "the fixture must really decode, or the cache is being \
                 measured on blanks"
            );
        };
        for n in 1..=COUNT {
            show(&state, n);
        }
        let cached = state.attachment_images.borrow().len();
        assert!(cached < COUNT as usize, "nothing was ever dropped: {cached} rasters");
        let spent = state.attachment_cache_bytes.get();
        assert!(spent <= MAX_ATTACHMENT_CACHE_BYTES, "budget blown by {spent}");
        // control: the survivors are full-size rasters, so the eviction above
        // emptied a real cache rather than a map of zero-weight blanks
        assert!(spent > MAX_ATTACHMENT_CACHE_BYTES / 2, "only {spent} spent");
        for n in (COUNT - 2)..=COUNT {
            assert!(
                state.attachment_images.borrow().contains_key(&n),
                "the newest raster {n} should still be decoded"
            );
        }

        // Re-showing the oldest survivor makes it the freshest thing here, so
        // the next round of evictions has to take 5, 6 and 7 and leave 4 —
        // first-in-first-out would drop 4, and scroll-back would re-decode.
        show(&state, 4);
        for n in 12..15 {
            show(&state, n);
        }
        assert!(state.attachment_images.borrow().contains_key(&4), "4 was just on screen");
        assert!(!state.attachment_images.borrow().contains_key(&5), "5 is the stalest");

        for n in 1..15 {
            let id = (base + n) as u64;
            std::fs::remove_file(state.store.dir().join(format!("{id}.png"))).ok();
            std::fs::remove_file(state.store.dir().join(format!("{id}.cache.png"))).ok();
        }
    }

    /// The plan is shared by the row builder and the seeder so neither can
    /// drift: `stride` is how far apart the picture rows sit, `pool` how many
    /// fixtures back them.
    #[test]
    fn a_pictures_plan_spaces_the_rows_and_caps_the_pool() {
        use super::{bench_picture_plan, BENCH_FIXTURE_POOL};
        assert_eq!(bench_picture_plan(0, 500), (0, 0), "a page with no rows");
        assert_eq!(bench_picture_plan(1000, 0), (0, 0), "--pictures 0 is off");
        // the two scenes the matrix runs
        assert_eq!(bench_picture_plan(10_000, 500), (20, BENCH_FIXTURE_POOL));
        assert_eq!(bench_picture_plan(10_000, 5_000), (2, BENCH_FIXTURE_POOL));
        // more pictures asked for than rows available cannot invent rows
        assert_eq!(bench_picture_plan(100, 1_000), (1, 100));
    }

    #[test]
    fn a_pictures_page_turns_every_stride_row_into_an_image() {
        use super::{bench_pictures, BENCH_FIXTURE_POOL, BLOCK_IMAGE, BLOCK_PARAGRAPH};
        use crate::core::{Block, BlockId, BlockKind, ColorKind, OrderKey, PageId};
        let mut blocks: Vec<Block> = (0..1000u64)
            .map(|i| Block {
                id: BlockId(i + 1),
                page: PageId(1),
                parent: None,
                order: OrderKey(0),
                kind: BlockKind::Paragraph,
                text: format!("row {i}"),
                checked: false,
                marks: Vec::new(),
                color: ColorKind::Default,
                background: ColorKind::Default,
                page_ref: None,
                folded: false,
                attachment: None,
                img_percent: 100,
                columns: 0,
            })
            .collect();
        bench_pictures(&mut blocks, 250);

        let picture_rows: Vec<(usize, u64)> = blocks
            .iter()
            .enumerate()
            .filter_map(|(i, b)| b.attachment.map(|a| (i, a.as_u64())))
            .collect();
        assert_eq!(picture_rows.len(), 250, "one row every four");
        assert!(
            blocks
                .iter()
                .all(|b| (b.kind == BlockKind::Image) == (b.attachment.is_some())),
            "an image row is the only row that carries an attachment"
        );
        assert_eq!(picture_rows[0], (3, 1), "the stride, not an off-by-one");
        // consecutive picture rows draw different fixtures — that is what a
        // photo page does and what the decode cache is sized for...
        assert_eq!(picture_rows[1], (7, 2));
        // ...and the pool wraps only after it is exhausted.
        assert_eq!(picture_rows[BENCH_FIXTURE_POOL].0, 3 + 4 * BENCH_FIXTURE_POOL);
        assert_eq!(picture_rows[BENCH_FIXTURE_POOL].1, 1, "the pool wrapped");
        assert_eq!(super::kind_to_int(blocks[0].kind), BLOCK_PARAGRAPH);
        assert_eq!(super::kind_to_int(blocks[3].kind), BLOCK_IMAGE);
        assert_eq!(blocks[3].text, "", "a picture row has no text to lay out");
    }

    /// The bench scene has to survive from its seed pass to its measured pass
    /// the way a real session survives a restart: the first `AppState::new`
    /// writes the pool and its rows, the second loads them. If the second one
    /// regenerated anything, the measured pass would be timing a write.
    #[test]
    fn the_pictures_scene_seeds_its_pool_once_and_then_only_loads_it() {
        use super::{AppState, HandleArgs, BLOCK_IMAGE};
        use crate::testing::ScratchDir;
        use slint::Model;

        let dir = ScratchDir::new("pictures");
        let db = dir.path().join("library.db");
        let args = HandleArgs { blocks: 60, auto_exit_secs: 0.0, bench_pages: 0, pictures: 20 };

        let first = AppState::new(&args, Some(std::sync::Arc::new(
            crate::storage::SqliteRepository::open(&db).unwrap(),
        )));
        let seeded: Vec<String> = {
            let book = first.attachments.borrow();
            assert_eq!(book.len(), 20, "one fixture per picture row");
            (1..=20i64)
                .map(|k| book.get(&k).expect("fixture id").file.clone())
                .collect()
        };
        assert!(
            seeded.iter().all(|f| dir.path().join("attachments").join(f).is_file()),
            "the bytes are on disk, not just in the map"
        );
        let rows: Vec<i32> = (0..first.blocks.row_count())
            .filter_map(|i| first.blocks.row_data(i).map(|r| r.kind))
            .collect();
        assert_eq!(rows.iter().filter(|k| **k == BLOCK_IMAGE).count(), 20);

        // One fixture goes away between the passes. A second `AppState::new`
        // that re-seeded would silently write it back and the assertion below
        // would pass for the wrong reason, so the load path is what is being
        // checked here, not the count.
        std::fs::remove_file(dir.path().join("attachments").join(&seeded[0])).unwrap();

        let second = AppState::new(&args, Some(std::sync::Arc::new(
            crate::storage::SqliteRepository::open(&db).unwrap(),
        )));
        assert_eq!(second.attachments.borrow().len(), 20, "the rows loaded back");
        assert!(
            !dir.path().join("attachments").join(&seeded[0]).exists(),
            "the measured pass must not re-write the pool"
        );
    }

    /// A 3x2 grid between two paragraphs, in display order. Cell "A1" is bold.
    fn grid_scene() -> Vec<crate::core::Block> {
        use crate::core::{Block, BlockId, BlockKind, ColorKind, Mark, MarkKind, OrderKey, PageId};
        let mk = |id: u64, parent: Option<u64>, kind: BlockKind, text: &str| Block {
            id: BlockId(id),
            page: PageId(1),
            parent: parent.map(BlockId),
            order: OrderKey(0),
            kind,
            text: text.into(),
            checked: false,
            marks: Vec::new(),
            color: ColorKind::Default,
            background: ColorKind::Default,
            page_ref: None,
            folded: false,
            attachment: None,
            img_percent: 100,
            columns: if kind == BlockKind::Table { 3 } else { 0 },
        };
        let cell = |id: u64, text: &str| mk(id, Some(8), BlockKind::TableCell, text);
        let mut blocks = vec![
            mk(1, None, BlockKind::Paragraph, "before"),
            mk(8, None, BlockKind::Table, ""),
            cell(2, "A0"),
            Block {
                marks: vec![Mark { start: 0, end: 2, kind: MarkKind::Bold, url: String::new() }],
                ..cell(3, "A1")
            },
            cell(4, "A2"),
            cell(5, "B0"),
            cell(6, "B1"),
            cell(7, "B2"),
            mk(9, None, BlockKind::Paragraph, "after"),
        ];
        // the vec *is* display order — that is what a page's block list is
        for (i, b) in blocks.iter_mut().enumerate() {
            b.order = OrderKey((i as u64 + 1) * 10);
        }
        blocks
    }

    fn cell_texts(row: &crate::BlockRow) -> Vec<String> {
        use slint::Model;
        (0..row.table_cells.row_count())
            .map(|i| row.table_cells.row_data(i).unwrap().text.to_string())
            .collect()
    }

    #[test]
    fn a_grid_costs_one_row_and_carries_its_cells() {
        use slint::Model;
        let blocks = grid_scene();
        let rows = super::project_blocks(&blocks);
        // ADR-0028: the cells paint inside the grid delegate, so they must not
        // also cost a row each — SPEC §三十七 counts hidden as really hidden.
        let ids: Vec<i32> = rows.iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![1, 8, 9]);
        let table = &rows[1];
        assert_eq!((table.kind, table.columns), (super::BLOCK_TABLE, 3));
        assert!(!table.can_fold, "a grid has no chevron: no fold reveals its cells");
        assert_eq!(cell_texts(table), ["A0", "A1", "A2", "B0", "B1", "B2"], "row-major");
        // §十 marks ride into the grid: the delegate reads runs like a line's
        let marked = table.table_cells.row_data(1).unwrap();
        assert_eq!(marked.runs.row_count(), 1);
        let run = marked.runs.row_data(0).unwrap();
        assert_eq!((run.text.as_str(), run.bold), ("A1", true));
        assert_eq!(
            table.table_cells.row_data(0).unwrap().runs.row_count(),
            0,
            "an unmarked cell keeps the wrapping Text"
        );
        assert!(rows.last().unwrap().tail);
    }

    #[test]
    fn a_ragged_grid_projects_whole_rows() {
        use slint::Model;
        let mut blocks = grid_scene();
        // stray cells (a v8 database touched by hand) must not become a
        // half-row: the delegate chunks by `columns` and indexes into the list
        blocks.retain(|b| b.id != crate::core::BlockId(6) && b.id != crate::core::BlockId(7));
        let rows = super::project_blocks(&blocks);
        assert_eq!(rows[1].table_cells.row_count(), 3, "four cells is one row of three");
        assert_eq!(cell_texts(&rows[1]), ["A0", "A1", "A2"]);
    }

    #[test]
    fn every_row_index_consumer_shares_the_one_visible_list() {
        let blocks = grid_scene();
        // a cell has no row, so no row can carry kind 17 and no row index can
        // land inside a grid — drop_index_for_row reads the same list
        let rows = super::project_blocks(&blocks);
        assert!(rows.iter().all(|r| r.kind != super::BLOCK_TABLE_CELL));
        assert_eq!(super::visible_block_indices(&blocks), vec![0, 1, 8]);
    }

    /// PNG bytes the way the clipboard hands them over: already encoded, never
    /// a file on disk.
    fn png_bytes(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(w, h, image::Rgba([9, 99, 199, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    #[test]
    fn a_pasted_picture_becomes_an_empty_block_and_lands_below_a_written_one() {
        use super::{AppState, HandleArgs, BLOCK_IMAGE, BLOCK_PARAGRAPH};
        use crate::app::state::Change;
        use crate::core::{BlockId, Command};
        use crate::testing::ScratchDir;
        use slint::Model;

        let dir = ScratchDir::new("paste");
        // A real library in the scratch folder, so the attachment bytes this
        // test stores are the folder's to lose. With no database the store
        // falls back to a shared %TEMP% directory and the paste would litter
        // it.
        let repo = std::sync::Arc::new(
            crate::storage::SqliteRepository::open(&dir.path().join("library.db")).unwrap(),
        );
        let state =
            AppState::new(&HandleArgs { blocks: 0, auto_exit_secs: 0.0, bench_pages: 0, pictures: 0 }, Some(repo));

        state.create_page(None);
        let empty = state.start_page().expect("an empty page takes a paragraph");
        let png = png_bytes(24, 18);

        assert!(state.paste_image(empty, &png), "the paste must land");
        let row = state.blocks.row_data(0).unwrap();
        assert_eq!(row.kind, BLOCK_IMAGE, "the empty block *is* the picture");
        assert_eq!(row.id, empty, "and not a new row");
        let att_id = state
            .doc
            .borrow()
            .block(BlockId(empty as u64))
            .and_then(|b| b.attachment)
            .expect("the block carries the attachment");
        let (stored, name, mime, size) = {
            let book = state.attachments.borrow();
            let att = book.get(&(att_id.as_u64() as i64)).expect("the attachment is known");
            (att.file.clone(), att.name.clone(), att.mime.clone(), att.bytes)
        };
        assert_eq!(name, "Pasted image");
        assert_eq!((size, mime.as_str()), (png.len() as i64, "image/png"));
        let on_disk = dir.path().join("attachments").join(&stored);
        assert!(on_disk.exists(), "the bytes went beside the library, not into it");

        // a block with words in it keeps them and gets the picture below
        let changes = state
            .exec_on_open_page(Command::InsertBlockAfter {
                id: BlockId(empty as u64),
                kind: crate::core::BlockKind::Paragraph,
                text: "written first".into(),
            })
            .expect("a paragraph below the picture");
        let written = changes
            .iter()
            .find_map(|c| match c {
                Change::BlockInserted(b) => Some(b.id.0 as i32),
                _ => None,
            })
            .unwrap();
        assert!(state.paste_image(written, &png));
        assert_eq!(state.blocks.row_count(), 3);
        assert_eq!(state.blocks.row_data(1).unwrap().kind, BLOCK_PARAGRAPH);
        assert_eq!(state.blocks.row_data(1).unwrap().text, "written first");
        assert_eq!(state.blocks.row_data(2).unwrap().kind, BLOCK_IMAGE);

        // one paste is one undo step, and it drops the reference only
        state.undo_open_page();
        assert_eq!(state.blocks.row_count(), 2);
        assert_eq!(state.blocks.row_data(0).unwrap().kind, BLOCK_IMAGE);
        assert!(on_disk.exists(), "undo never deletes bytes another block may still point at");
    }

    // --- reclaim (SPEC §三十七, ADR-0037) ---

    // The signatures here spell out their paths: the helpers below are used by
    // several tests and a `use` in a function body does not reach a signature.
    fn scratch_repo(
        dir: &crate::testing::ScratchDir,
    ) -> std::sync::Arc<crate::storage::SqliteRepository> {
        std::sync::Arc::new(
            crate::storage::SqliteRepository::open(&dir.path().join("library.db")).unwrap(),
        )
    }

    fn plain_args() -> super::HandleArgs {
        super::HandleArgs { blocks: 0, auto_exit_secs: 0.0, bench_pages: 0, pictures: 0 }
    }

    /// A fresh session on a real database: one page with `n` pictures on it,
    /// oldest first, each with its row in the database and its bytes in the
    /// scratch folder. Per picture the caller gets back
    /// `(attachment id, file name, block id)`.
    fn session_with_pictures(
        dir: &crate::testing::ScratchDir,
        n: usize,
    ) -> (
        std::rc::Rc<super::AppState>,
        std::sync::Arc<crate::storage::SqliteRepository>,
        i32,
        Vec<(i64, String, i32)>,
    ) {
        use crate::core::BlockKind;

        let repo = scratch_repo(dir);
        let state = super::AppState::new(&plain_args(), Some(repo.clone()));
        let page = state.create_page(None);
        let mut anchor = state.start_page().expect("the page takes its first block");
        let mut pics = Vec::new();
        for _ in 0..n {
            let att = state
                .store
                .import_bytes(state.claim_attachment_id(), "sample", &png_bytes(24, 18))
                .unwrap();
            assert!(state.insert_attachment(anchor, att.clone(), BlockKind::Image));
            let block = {
                let doc = state.doc.borrow();
                let found = doc
                    .all_blocks()
                    .find(|b| b.attachment == Some(att.id))
                    .expect("the row carries the pointer")
                    .id;
                found.0 as i32
            };
            anchor = block;
            pics.push((att.id.as_u64() as i64, att.file.clone(), block));
        }
        (state, repo, page, pics)
    }

    #[test]
    fn a_reclaim_leaves_alone_everything_undo_can_still_bring_back() {
        use super::BLOCK_IMAGE;
        use crate::core::{BlockId, Command};
        use crate::testing::ScratchDir;
        use slint::Model;

        let dir = ScratchDir::new("reclaim-undo");
        let attach_dir = dir.path().join("attachments");
        let (state, _repo, _page, pics) = session_with_pictures(&dir, 2);

        // drop the second picture's row: reference gone, row and bytes stay
        state
            .exec_on_open_page(Command::DeleteBlock { id: BlockId(pics[1].2 as u64) })
            .expect("a picture block deletes");
        assert_eq!(
            state.reclaim_attachments().unwrap(),
            "no unused attachments to remove",
            "the undo step holds the only other pointer to it"
        );
        assert!(attach_dir.join(&pics[1].1).is_file(), "nothing was deleted");

        // the protection is not theoretical: Ctrl+Z puts the row back and the
        // picture still has bytes to show
        state.undo_open_page();
        let kinds: Vec<i32> = (0..state.blocks.row_count())
            .filter_map(|i| state.blocks.row_data(i).map(|r| r.kind))
            .collect();
        assert_eq!(kinds.iter().filter(|k| **k == BLOCK_IMAGE).count(), 2);
        assert_eq!(state.image_for(pics[1].0 as i32).size().width, 24);
    }

    #[test]
    fn a_restarted_session_reclaims_the_picture_its_predecessor_deleted() {
        use super::AppState;
        use crate::core::{BlockId, Command};
        use crate::testing::ScratchDir;

        let dir = ScratchDir::new("reclaim-restart");
        let attach_dir = dir.path().join("attachments");
        let (state, repo, _page, pics) = session_with_pictures(&dir, 2);
        state
            .exec_on_open_page(Command::DeleteBlock { id: BlockId(pics[1].2 as u64) })
            .expect("a picture block deletes");
        state.persistence_force_flush();
        drop(state);
        // control: the orphan is really in the database, or the sweep below
        // would be proving nothing
        assert_eq!(repo.load_attachments().unwrap().len(), 2);
        drop(repo);

        let repo = scratch_repo(&dir);
        let state = AppState::new(&plain_args(), Some(repo.clone()));
        assert_eq!(state.attachments.borrow().len(), 2, "a new session sees both rows");
        // the dead row is on no screen, but a decode of it costs the cache
        // weight, and the reclaim has to hand that back
        assert!(state.image_for(pics[1].0 as i32).size().width > 0);
        assert!(state.attachment_cache_bytes.get() > 0);

        let notice = state.reclaim_attachments().unwrap();
        assert!(notice.starts_with("1 unused attachment removed ("), "{notice}");
        assert!(state.attachments.borrow().contains_key(&pics[0].0), "the live row stays");
        assert!(!state.attachments.borrow().contains_key(&pics[1].0));
        assert!(attach_dir.join(&pics[0].1).is_file(), "and nothing else went");
        assert!(!attach_dir.join(&pics[1].1).exists(), "its bytes went with its row");
        assert_eq!(repo.load_attachments().unwrap().len(), 1, "the row left the database too");
        assert_eq!(state.attachment_cache_bytes.get(), 0, "the decode cache paid back");
        assert_eq!(
            state.image_for(pics[1].0 as i32).size().width,
            0,
            "a reclaimed id paints blank, not the bytes it used to name"
        );
    }

    /// The clipboard is the third pointer a block list cannot show. This needs
    /// a session that did not paste the picture itself — otherwise the undo
    /// stack is protecting it and the clipboard is never tested.
    #[test]
    fn a_copied_picture_survives_losing_its_page_to_a_reclaim() {
        use super::AppState;
        use crate::testing::ScratchDir;
        use slint::Model;

        let dir = ScratchDir::new("reclaim-clipboard");
        let (state, repo, page, pics) = session_with_pictures(&dir, 1);
        state.persistence_force_flush();
        assert_eq!(repo.load_attachments().unwrap().len(), 1, "control: the row is in the db");
        drop(state);
        drop(repo);

        let repo = scratch_repo(&dir);
        let state = AppState::new(&plain_args(), Some(repo));
        state.open_page(page);
        let rows: Vec<i32> = (0..state.blocks.row_count())
            .filter_map(|i| state.blocks.row_data(i).map(|r| r.id))
            .collect();
        assert!(rows.contains(&pics[0].2), "control: the page loaded with its picture row");
        state.copy_block(pics[0].2);
        state.create_page(None);
        state.delete_page(page);
        assert_eq!(
            state.reclaim_attachments().unwrap(),
            "no unused attachments to remove",
            "pasting the copied row is still one keystroke away"
        );
        let target = state.start_page().expect("the new page takes a block");
        assert!(state.paste_below(target), "the copied row lands");
        assert_eq!(state.image_for(pics[0].0 as i32).size().width, 24, "and its bytes were never touched");
    }

    #[test]
    fn a_deleted_pages_picture_is_reclaimable_without_a_restart() {
        use super::AppState;
        use crate::testing::ScratchDir;

        let dir = ScratchDir::new("reclaim-page");
        let attach_dir = dir.path().join("attachments");
        let (state, repo, page, pics) = session_with_pictures(&dir, 1);
        state.persistence_force_flush();
        drop(state);
        drop(repo);

        let repo = scratch_repo(&dir);
        let state = AppState::new(&plain_args(), Some(repo.clone()));
        state.create_page(None); // deleting a page needs one left to be open
        // the bool is "was the open page among those removed", not "did it work"
        state.delete_page(page);
        let notice = state.reclaim_attachments().unwrap();
        assert!(notice.starts_with("1 unused attachment removed ("), "{notice}");
        assert!(!attach_dir.join(&pics[0].1).exists());
        assert_eq!(repo.load_attachments().unwrap().len(), 0, "the row went with the page");
    }

    /// The half-orphan: a row whose bytes were deleted by hand. A missing file
    /// is not a failure for the sweep — clearing the row is the whole point.
    #[test]
    fn a_row_whose_bytes_are_already_gone_is_still_removed() {
        use super::AppState;
        use crate::testing::ScratchDir;

        let dir = ScratchDir::new("reclaim-half-orphan");
        let attach_dir = dir.path().join("attachments");
        let (state, repo, page, pics) = session_with_pictures(&dir, 1);
        state.persistence_force_flush();
        std::fs::remove_file(attach_dir.join(&pics[0].1)).unwrap();
        drop(state);
        drop(repo);

        let repo = scratch_repo(&dir);
        let state = AppState::new(&plain_args(), Some(repo.clone()));
        state.create_page(None);
        state.delete_page(page);
        let notice = state.reclaim_attachments().unwrap();
        assert!(notice.starts_with("1 unused attachment removed ("), "{notice}");
        assert!(
            !notice.contains("could not be deleted"),
            "a file that was already gone is not a stuck file: {notice}"
        );
        assert_eq!(repo.load_attachments().unwrap().len(), 0);
    }

    #[test]
    fn a_session_without_a_file_has_nothing_to_reclaim() {
        use super::AppState;

        let state = AppState::new(&plain_args(), None);
        assert_eq!(
            state.reclaim_attachments().unwrap(),
            "running in memory — nothing to reclaim"
        );
        let repo = std::sync::Arc::new(crate::storage::SqliteRepository::in_memory().unwrap());
        let state = AppState::new(&plain_args(), Some(repo));
        assert_eq!(
            state.reclaim_attachments().unwrap(),
            "running in memory — nothing to reclaim"
        );
    }

    /// Reclaim runs on the UI thread, so its cost is how long a click freezes
    /// the window. Prints two arms in one sitting, so they share a disk cache:
    /// sweeping a library of 1 000 orphans, and the same call afterwards with
    /// nothing left to do (the floor — queue flush over an empty book).
    #[test]
    #[ignore = "prints a timing measurement"]
    fn a_reclaim_of_a_thousand_orphans_is_timed() {
        use crate::core::persistence::{Change, Repository};
        use crate::testing::ScratchDir;
        use std::time::Instant;

        const N: usize = 1000;
        let dir = ScratchDir::new("reclaim-timing");
        let repo = scratch_repo(&dir);
        let state = super::AppState::new(&plain_args(), Some(repo.clone()));
        // the shape of a library nobody ever reclaimed: a row and bytes for
        // every one of them, and no block pointing at any
        let setup = Instant::now();
        for _ in 0..N {
            let att = state
                .store
                .import_bytes(state.claim_attachment_id(), "orphan", &png_bytes(24, 18))
                .unwrap();
            repo.apply(&[Change::AttachmentAdded(att.clone())]).unwrap();
            state
                .attachments
                .borrow_mut()
                .insert(att.id.as_u64() as i64, att);
        }
        let setup_ms = setup.elapsed().as_secs_f64() * 1000.0;

        // control: the sweep can only be fast if there really was work here
        let attach_dir = dir.path().join("attachments");
        let on_disk = std::fs::read_dir(&attach_dir).unwrap().count();
        assert_eq!(on_disk, N, "the folder has one file per orphan");
        assert_eq!(
            repo.load_attachments().unwrap().len(),
            N,
            "and the table has one row per file"
        );

        let swept = Instant::now();
        let report = state.reclaim_attachments().unwrap();
        let sweep_ms = swept.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(
            std::fs::read_dir(&attach_dir).unwrap().count(),
            0,
            "and the sweep took them all"
        );
        assert_eq!(repo.load_attachments().unwrap().len(), 0);

        let second = Instant::now();
        assert_eq!(
            state.reclaim_attachments().unwrap(),
            "no unused attachments to remove"
        );
        let scan_ms = second.elapsed().as_secs_f64() * 1000.0;

        println!(
            r#"{{"scene":"reclaim-timing","orphans":{N},"setup_ms":{setup_ms:.1},"sweep_ms":{sweep_ms:.1},"scan_ms":{scan_ms:.1},"report":"{report}"}}"#
        );
    }
}
