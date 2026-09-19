// End-to-end checks for the debounced persistence pipeline against a
// real SQLite file: typing-rate bursts coalesce, the quiet window gates
// writes, Ctrl+S / shutdown flushes land before the next "session" loads
// the file, and the periodic snapshot (M8 D10) reaches the `.bak<N>` family.

use std::path::Path;
use std::sync::Arc;

use quire::core::persistence::{Change, Repository};
use quire::core::types::{Block, BlockId, BlockKind, OrderKey, Page, PageId};
use quire::services::persistence::{
    FakeClock, PersistenceService, DEFAULT_DEBOUNCE_MS, DEFAULT_SNAPSHOT_INTERVAL_MS,
};
use quire::storage::{backup, SqliteRepository};

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
        color: quire::core::ColorKind::Default,
        background: quire::core::ColorKind::Default,
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
        color: quire::core::ColorKind::Default,
        background: quire::core::ColorKind::Default,
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

/// M8 D10: the periodic snapshot runs through the flush path the app already
/// ticks, against the real file, so a mid-session corruption costs the edits
/// made since the last tick instead of everything since startup.
#[test]
fn a_quiet_period_after_a_write_reaches_the_snapshot_family() {
    let path = temp_db("periodic-snapshot");
    // a rerun must start from no database and no family, not last run's
    let mut stale = vec![path.clone()];
    stale.extend((1..=backup::KEEP).map(|index| backup::slot(&path, index)));
    for file in &stale {
        let _ = std::fs::remove_file(file);
        for suffix in ["-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", file.display()));
        }
    }
    let repo = seeded(&path); // this open wrote the startup snapshot
    let clock = Arc::new(FakeClock::new());
    let svc = PersistenceService::new(repo.clone(), clock.clone(), DEFAULT_DEBOUNCE_MS)
        .with_database_snapshots(&repo);

    // the edit lands, is written by the debounce, and the period then elapses
    svc.record(vec![Change::PageTitleSet {
        id: PageId(1),
        title: "backed up mid-session".into(),
    }]);
    clock.set(DEFAULT_DEBOUNCE_MS);
    assert!(svc.flush_if_due().unwrap());
    assert_eq!(
        1,
        snapshots_of(&path),
        "the period has not passed: `.bak1` is still the startup copy"
    );
    assert_eq!(None, title_at(&backup::slot(&path, 1)));

    clock.set(DEFAULT_SNAPSHOT_INTERVAL_MS + 1);
    assert!(!svc.flush_if_due().unwrap(), "nothing new to write");
    assert_eq!(
        Some("backed up mid-session".to_string()),
        title_at(&backup::slot(&path, 1)),
        "the tick that closed the period took the snapshot"
    );
    assert_eq!(2, snapshots_of(&path), "and pushed the startup copy down");

    // a further period with no write takes none: `pending` gates it
    clock.set(DEFAULT_SNAPSHOT_INTERVAL_MS * 2);
    assert!(!svc.flush_if_due().unwrap());
    assert_eq!(
        Some("backed up mid-session".to_string()),
        title_at(&backup::slot(&path, 1))
    );
    drop(svc);
    drop(repo);
    let _ = std::fs::remove_dir_all(path.parent().unwrap());
}

/// How many generations the family at `path` currently holds.
fn snapshots_of(path: &Path) -> usize {
    (1..=backup::KEEP)
        .filter(|index| backup::slot(path, *index).exists())
        .count()
}

/// Read one page title out of a database file without opening it for write.
fn title_at(path: &Path) -> Option<String> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    conn.query_row("SELECT title FROM pages WHERE id = 1", [], |r| {
        r.get::<_, String>(0)
    })
    .ok()
}
