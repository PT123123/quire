// Application state + mock content for M1/M2.
//
// The real Document Model (M3+) replaces the `mock_*` constructors; the
// shapes below (SidebarNode / BlockRow / CommandRow) are UI-facing
// projections generated from `ui/Types.slint`, not the storage format.

use crate::{BlockRow, CommandRow, SidebarNode};
use slint::{ModelRc, VecModel};
use std::rc::Rc;

pub struct AppState {
    pub sidebar: Rc<VecModel<SidebarNode>>,
    pub blocks: Rc<VecModel<BlockRow>>,
    pub commands: Rc<VecModel<CommandRow>>,
    /// Full command list before query filtering.
    pub all_commands: Vec<CommandRow>,
    pub page_titles: Vec<&'static str>,
}

pub struct HandleArgs {
    /// Number of mock blocks for benchmarks (0 = default sample document).
    pub blocks: usize,
    /// Quit the event loop after N seconds (0 = never).
    pub auto_exit_secs: f64,
}

impl AppState {
    pub fn new(args: &HandleArgs) -> Rc<Self> {
        let sidebar = Rc::new(VecModel::from(mock_sidebar()));
        let blocks = if args.blocks > 0 {
            Rc::new(VecModel::from(mock_blocks_bench(args.blocks)))
        } else {
            Rc::new(VecModel::from(mock_blocks_sample()))
        };
        let all_commands = mock_commands();
        let commands = Rc::new(VecModel::from(all_commands.clone()));
        Rc::new(AppState {
            sidebar,
            blocks,
            commands,
            all_commands,
            page_titles: mock_page_titles(),
        })
    }

    pub fn sidebar_model(&self) -> ModelRc<SidebarNode> {
        ModelRc::from(self.sidebar.clone())
    }
    pub fn blocks_model(&self) -> ModelRc<BlockRow> {
        ModelRc::from(self.blocks.clone())
    }
    pub fn commands_model(&self) -> ModelRc<CommandRow> {
        ModelRc::from(self.commands.clone())
    }

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

    /// Replace sidebar rows (used when a page node toggles expansion or is selected).
    pub fn set_sidebar_rows(&self, rows: Vec<SidebarNode>) {
        self.sidebar.set_vec(rows);
    }

    pub fn load_page(&self, title: &str) {
        self.blocks.set_vec(mock_blocks_for_page(title));
    }
}

fn node(id: i32, label: &str, kind: &str, depth: i32, expanded: bool, has_children: bool) -> SidebarNode {
    SidebarNode {
        id,
        label: label.into(),
        kind: kind.into(),
        depth,
        expanded,
        has_children,
        selected: false,
    }
}

/// Flat, pre-expanded row list; `depth` drives indentation. Children of a
/// collapsed node are simply not present in the model (M2 keeps it this way;
/// real virtualization arrives with M7's tree work).
pub fn mock_sidebar() -> Vec<SidebarNode> {
    use sidebar_kind::*;
    let mut rows = vec![node(10, "FAVORITES", SECTION_HEADER, 0, false, false)];
    rows.push(node(11, "Weekly Review", FAVORITE, 0, false, false));
    rows.push(node(12, "Design Ideas", FAVORITE, 0, false, false));
    rows.push(node(20, "RECENT", SECTION_HEADER, 0, false, false));
    rows.push(node(21, "Project Atlas", RECENT, 0, false, false));
    rows.push(node(22, "Meeting Notes", RECENT, 0, false, false));
    rows.push(node(30, "WORKSPACE", SECTION_HEADER, 0, false, false));
    rows.push(node(31, "Getting Started", PAGE, 0, true, true));
    rows.push(node(32, "Keyboard Shortcuts", PAGE, 1, false, false));
    rows.push(node(33, "Import from Markdown", PAGE, 1, false, false));
    rows.push(node(34, "Project Atlas", PAGE, 0, true, true));
    rows.push(node(35, "Research Notes", PAGE, 1, true, true));
    rows.push(node(36, "Sources", PAGE, 2, false, false));
    rows.push(node(37, "Meeting Notes", PAGE, 1, false, false));
    rows.push(node(38, "Architecture", PAGE, 1, false, false));
    rows.push(node(39, "Reading List", PAGE, 0, false, true));
    rows.push(node(40, "写作与中文测试", PAGE, 0, false, false));
    rows.push(node(41, "Scratchpad", PAGE, 0, false, false));
    if let Some(atlas) = rows.iter_mut().find(|r| r.id == 34) {
        atlas.selected = true;
    }
    rows
}

mod sidebar_kind {
    pub const SECTION_HEADER: &str = "header";
    pub const FAVORITE: &str = "favorite";
    pub const RECENT: &str = "recent";
    pub const PAGE: &str = "page";
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

fn mock_blocks_sample() -> Vec<BlockRow> {
    let mut b = vec![block(
        BLOCK_PARAGRAPH,
        "A quiet home for thinking. This workspace collects notes, plans, and references for the Atlas project — and doubles as the visual test document for Quire itself.",
    )];
    b.push(block(BLOCK_DIVIDER, ""));
    b.push(block(BLOCK_H2, "Why a local-first editor"));
    b.push(block(
        BLOCK_PARAGRAPH,
        "Cloud apps are great until the laptop fan sounds like a jet engine. Quire keeps documents in a local SQLite database, renders with the GPU, and stays out of the way.",
    ));
    b.push(block(BLOCK_QUOTE, "Simplicity is the ultimate sophistication — but performance is the ultimate courtesy."));
    b.push(block(BLOCK_H3, "Principles"));
    b.push(block(BLOCK_BULLET, "One process, one document model, no hidden servers"));
    b.push(block(BLOCK_BULLET, "Only the focused block owns a real text cursor"));
    b.push(block(BLOCK_BULLET, "Nothing animates unless the user asks for it"));
    b.push(block(BLOCK_NUMBERED, "Write instantly, even on a five-year-old laptop"));
    b.push(block(BLOCK_NUMBERED, "Scroll a 10 000-block page without hitching"));
    b.push(block(BLOCK_NUMBERED, "Close the lid, reopen, and everything is there"));
    b.push(block(BLOCK_TODO, "Block editor MVP"));
    b.push(block(BLOCK_TODO, "Slash menu (type \"/\" anywhere)"));
    b.push(block(BLOCK_H2, "运行与中文"));
    b.push(block(
        BLOCK_PARAGRAPH,
        "中文段落用于验证字体回退与行高：排版本应稳定，不出现字符裁剪；标点悬挂与换行位置符合预期。",
    ));
    b.push(block(BLOCK_CODE, "cargo run --release  # 140 ms to first paint, hopefully"));
    b.push(block(
        BLOCK_PARAGRAPH,
        "Start typing, or press Ctrl+K to open the command palette.",
    ));
    let mut numbered = 0;
    for (i, row) in b.iter_mut().enumerate() {
        row.id = i as i32;
        if row.kind == BLOCK_NUMBERED {
            numbered += 1;
            row.number = numbered;
        }
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

pub fn mock_blocks_for_page(title: &str) -> Vec<BlockRow> {
    let mut b = Vec::new();
    b.push(block(BLOCK_H1, title));
    b.push(block(
        BLOCK_PARAGRAPH,
        &format!("This is the “{title}” page. Real editing arrives with the block editor milestone; the shell, the fonts, and the colors are already load-bearing."),
    ));
    b.push(block(BLOCK_DIVIDER, ""));
    b.push(block(
        BLOCK_PARAGRAPH,
        "Start typing, or press Ctrl+K to open the command palette.",
    ));
    with_tail(b)
}

fn mock_commands() -> Vec<CommandRow> {
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
    cmd(1, "New Page", "Ctrl+N", "Editor", "plus");
    cmd(2, "Toggle Sidebar", "Ctrl+B", "Interface", "panel-left");
    cmd(3, "Toggle Dark Mode", "Ctrl+Shift+L", "Interface", "moon");
    cmd(4, "Go to Settings", "", "Navigate", "settings");
    cmd(5, "Export as Markdown…", "Ctrl+E", "File", "export");
    cmd(6, "Import from Markdown…", "", "File", "import");
    for (i, t) in mock_page_titles().iter().enumerate() {
        cmd(50 + i as i32, t, "", "Jump to page", "page");
    }
    v
}

fn mock_page_titles() -> Vec<&'static str> {
    vec![
        "Getting Started",
        "Keyboard Shortcuts",
        "Import from Markdown",
        "Project Atlas",
        "Research Notes",
        "Meeting Notes",
        "Architecture",
        "Reading List",
        "写作与中文测试",
        "Scratchpad",
    ]
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
