// End-to-end checks for the debounced persistence pipeline against a
// real SQLite file: typing-rate bursts coalesce, the quiet window gates
// writes, and Ctrl+S / shutdown flushes land before the next "session"
// loads the file.

use std::path::Path;
use std::sync::Arc;

use quire::core::persistence::{Change, Repository};
use quire::core::types::{Block, BlockId, BlockKind, OrderKey, Page, PageId};
use quire::services::persistence::{FakeClock, PersistenceService};
use quire::storage::SqliteRepository;

fn temp_db(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "quire-persist-{}-{}",
        std::process::id(),
        name
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("quire.db")
}

fn seeded(path: &Path) -> Arc<SqliteRepository> {
    let repo = Arc::new(SqliteRepository::open(path).unwrap());
    repo.apply(&[
        Change::PageCreated(Page {
            id: PageId(1),
            title: "Editor scratch".into(),
            parent: None,
            order: OrderKey::FIRST,
            favorite: false,
            expanded: true,
        }),
        Change::BlockInserted(Block {
            id: BlockId(1),
            page: PageId(1),
            parent: None,
            order: OrderKey::FIRST,
            kind: BlockKind::Paragraph,
            text: String::new(),
            checked: false,
                marks: Vec::new(),
        }),
    ])
    .unwrap();
    repo
}

#[test]
fn burst_then_quiet_writes_exactly_once() {
    let path = temp_db("burst");
    let repo = seeded(&path);
    let clock = Arc::new(FakeClock::new());
    let svc = PersistenceService::new(repo.clone(), clock.clone(), 300);

    // the M4 editor will send one batch per keystroke-equivalent command;
    // simulate 50 rapid text updates
    for i in 0..50 {
        clock.set(i * 5); // 5 ms apart: never quiet long enough
        svc.record(vec![Change::BlockTextSet {
            id: BlockId(1),
            text: format!("typed {} chars", i + 1),
        }]);
        assert_eq!(svc.flush_if_due().unwrap(), false);
    }
    let loaded = repo.load().unwrap();
    assert_eq!(loaded.blocks[0].text, "", "nothing may hit the db mid-burst");

    clock.set(1000); // user stops typing
    assert_eq!(svc.flush_if_due().unwrap(), true);
    let loaded = repo.load().unwrap();
    assert_eq!(loaded.blocks[0].text, "typed 50 chars");

    // quiet afterwards: no spinning writes without new changes
    clock.set(5000);
    assert_eq!(svc.flush_if_due().unwrap(), false);
}

#[test]
fn shutdown_flush_then_next_session_loads_it() {
    let path = temp_db("session");
    {
        let repo = seeded(&path);
        let clock = Arc::new(FakeClock::new());
        let svc = PersistenceService::new(repo.clone(), clock.clone(), 300);
        svc.record(vec![Change::BlockInserted(Block {
            id: BlockId(2),
            page: PageId(1),
            parent: Some(BlockId(1)),
            order: OrderKey::FIRST,
            kind: BlockKind::Todo,
            text: "未保存的中文草稿".into(),
            checked: true,
                marks: Vec::new(),
        })]);
        clock.set(10); // nowhere near due — this is the Ctrl+S/quit path
        svc.force_flush().unwrap();
        assert!(!svc.has_pending());
    }
    let reopened = SqliteRepository::open(&path).unwrap();
    let state = reopened.load().unwrap();
    let todo = state.blocks.iter().find(|b| b.id == BlockId(2)).unwrap();
    assert_eq!(todo.text, "未保存的中文草稿");
    assert_eq!(todo.kind, BlockKind::Todo);
    assert!(todo.checked);
    assert_eq!(todo.parent, Some(BlockId(1)));
}

#[test]
fn deadline_guides_the_timer_and_multiple_bursts_stay_ordered() {
    let path = temp_db("deadline");
    let repo = seeded(&path);
    let clock = Arc::new(FakeClock::new());
    clock.set(1000);
    let svc = PersistenceService::new(repo.clone(), clock.clone(), 300);
    assert_eq!(svc.next_deadline_ms(), None);

    svc.record(vec![Change::PageTitleSet {
        id: PageId(1),
        title: "A".into(),
    }]);
    assert_eq!(svc.next_deadline_ms(), Some(1300));
    clock.set(1299);
    assert_eq!(svc.flush_if_due().unwrap(), false);
    clock.set(1300);
    assert_eq!(svc.flush_if_due().unwrap(), true);

    // two later bursts: last write per id must win, order inside a batch kept
    svc.record(vec![Change::PageTitleSet {
        id: PageId(1),
        title: "B".into(),
    }]);
    svc.record(vec![Change::PageTitleSet {
        id: PageId(1),
        title: "C".into(),
    }]);
    clock.set(1700);
    assert_eq!(svc.flush_if_due().unwrap(), true);
    let state = repo.load().unwrap();
    assert_eq!(state.pages[0].title, "C");
}
