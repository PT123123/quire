// Integration tests: exercise the workspace model and the state projection
// through the public crate API, the way a future `core/` consumer would.

use quire::app::state::{core_page_id, AppState, HandleArgs, PAGE_GETTING_STARTED};
use quire::app::workspace::Workspace;
use slint::Model;

#[test]
fn tree_operations_survive_a_full_lifecycle() {
    let mut ws = Workspace::sample();
    let page = ws.create(Some(102), "Child");
    ws.rename(page, "Renamed Child");
    assert_eq!(ws.title_of(page), Some("Renamed Child"));

    // Getting Started + its 2 sample children + the new child
    let copy = ws.duplicate(102).expect("duplicate root");
    assert_eq!(ws.subtree_size(copy), 4);

    let removed = ws.delete(copy);
    assert_eq!(removed.len(), 4);
    assert!(!ws.contains(copy));
    assert!(ws.contains(page)); // original untouched
}

#[test]
fn recents_cap_and_order() {
    let mut ws = Workspace::sample();
    ws.mark_opened(112);
    assert_eq!(ws.recents()[0].0, 112);
    // only real pages land in recents; opening 6 more pushes 112 out
    for id in [103, 104, 106, 107, 108, 109] {
        ws.mark_opened(id);
    }
    assert_eq!(ws.recents().len(), quire::app::workspace::MAX_RECENTS);
    assert_eq!(ws.recents()[0].0, 109);
    assert!(ws.recents().iter().all(|(id, _)| *id != 112));
}

#[test]
fn search_finds_unopened_page_content() {
    // content blobs are filled at construction, so search works before a
    // page is ever opened
    let args = HandleArgs { blocks: 0, auto_exit_secs: 0.0, bench_pages: 0 };
    let state = AppState::new(&args, None);
    let hits = state.workspace.borrow().search("字体回退");
    assert!(
        hits.iter().any(|h| h.id == 112 && h.snippet.contains("字体回退")),
        "chinese fixture page should match by content"
    );
}

#[test]
fn sidebar_projection_shape() {
    let args = HandleArgs { blocks: 0, auto_exit_secs: 0.0, bench_pages: 0 };
    let state = AppState::new(&args, None);
    let rows = state.build_sidebar_rows();
    // sections in order, y offsets strictly increasing, no duplicate ids
    let kinds: Vec<&str> = rows.iter().map(|r| r.kind.as_str()).collect();
    assert_eq!(kinds[0], "header");
    assert!(rows.iter().any(|r| r.kind == "favorite"));
    assert!(rows.iter().any(|r| r.kind == "recent"));
    assert!(rows.iter().any(|r| r.kind == "new-page"));
    for w in rows.windows(2) {
        assert!(w[0].y < w[1].y, "y offsets must increase");
    }
    // the open page (Getting Started) is selected in the tree
    assert!(
        rows.iter()
            .any(|r| r.kind == "page" && r.selected && r.id == 102)
    );
}

#[test]
fn create_then_open_page_lands_empty() {
    let args = HandleArgs { blocks: 0, auto_exit_secs: 0.0, bench_pages: 0 };
    let state = AppState::new(&args, None);
    let id = state.create_page(None);
    assert_eq!(state.open_page.get(), id);
    assert_eq!(state.blocks.row_count(), 0, "new page shows the empty state");
    assert_eq!(state.workspace.borrow().title_of(id), Some("Untitled"));
}

#[test]
fn duplicated_nested_list_keeps_parents_in_the_copy() {
    // page with a bullet + a nested child (parent pointer inside the page)
    let args = HandleArgs { blocks: 0, auto_exit_secs: 0.0, bench_pages: 0 };
    let state = AppState::new(&args, None);
    let page = state.create_page(None);
    let root = quire::core::BlockId(9_000_000_001);
    let child = quire::core::BlockId(9_000_000_002);
    state.doc.borrow_mut().set_page_blocks(
        quire::core::PageId(page as u32 as u64),
        vec![
            quire::core::Block {
                id: root,
                page: quire::core::PageId(page as u32 as u64),
                parent: None,
                order: quire::core::OrderKey(10),
                kind: quire::core::BlockKind::Bullet,
                text: "parent item".into(),
                checked: false,
                marks: Vec::new(),
        color: quire::core::ColorKind::Default,
        background: quire::core::ColorKind::Default,
        page_ref: None,
            },
            quire::core::Block {
                id: child,
                page: quire::core::PageId(page as u32 as u64),
                parent: Some(root),
                order: quire::core::OrderKey(11),
                kind: quire::core::BlockKind::Bullet,
                text: "child item".into(),
                checked: false,
                marks: Vec::new(),
        color: quire::core::ColorKind::Default,
        background: quire::core::ColorKind::Default,
        page_ref: None,
            },
        ],
    );

    let copy = state.duplicate_page(page).expect("duplicate");
    let cpid = quire::core::PageId(copy as u32 as u64);
    let blocks = state.doc.borrow().page_blocks(cpid).to_vec();
    assert_eq!(blocks.len(), 2, "copy carries both blocks");
    // the child in the COPY points at the copied root, not the original
    let copied_child = blocks.iter().find(|b| b.text == "child item").unwrap();
    let copied_root = blocks.iter().find(|b| b.text == "parent item").unwrap();
    assert_ne!(copied_child.parent, Some(root), "stale parent pointer");
    assert_eq!(copied_child.parent, Some(copied_root.id));
    // depth projection agrees (both render, child indented)
    let rows = quire::app::state::project_blocks(&blocks);
    assert_eq!(rows[0].depth, 0);
    assert_eq!(rows[1].depth, 1);
}

#[test]
fn delete_open_page_resets_selection() {
    let args = HandleArgs { blocks: 0, auto_exit_secs: 0.0, bench_pages: 0 };
    let state = AppState::new(&args, None);
    let id = state.create_page(None);
    assert!(state.delete_page(id), "deleting the open page is reported");
    assert_eq!(state.open_page.get(), 0, "selection resets to no-page");
    // the controller picks the fallback; simulate that path
    let fallback = state.workspace.borrow().first_root();
    state.open_page(fallback.expect("sample keeps a root"));
    assert_eq!(state.open_page.get(), fallback.unwrap());
}

#[test]
fn move_page_reparents_refuses_cycles_and_swaps_siblings() {
    use quire::app::state::PAGE_ATLAS;

    let args = HandleArgs { blocks: 0, auto_exit_secs: 0.0, bench_pages: 0 };
    let state = AppState::new(&args, None);

    // reparent: Atlas moves under Getting Started, appended last
    assert!(state.move_page(PAGE_ATLAS, Some(PAGE_GETTING_STARTED)));
    {
        let ws = state.workspace.borrow();
        let kids = ws.children_of(Some(PAGE_GETTING_STARTED));
        assert!(kids.contains(&PAGE_ATLAS), "Atlas is a child now");
        assert!(!ws.children_of(None).contains(&PAGE_ATLAS), "no longer a root");
        assert_eq!(
            ws.get(PAGE_ATLAS).unwrap().parent,
            Some(PAGE_GETTING_STARTED)
        );
    }

    // a page cannot move into its own subtree (Getting Started under Atlas)
    assert!(!state.move_page(PAGE_GETTING_STARTED, Some(PAGE_ATLAS)));

    // sibling swap: Atlas lands one slot up among the children
    let kids = state.workspace.borrow().children_of(Some(PAGE_GETTING_STARTED));
    let count = kids.len();
    let before_neighbor = kids[count - 2];
    assert!(state.move_page_by(PAGE_ATLAS, -1));
    let kids = state.workspace.borrow().children_of(Some(PAGE_GETTING_STARTED));
    assert_eq!(kids.len(), count);
    assert_eq!(kids[count - 2], PAGE_ATLAS, "swapped one slot up");
    assert_eq!(kids[count - 1], before_neighbor);

    // the first child cannot move up further
    let first = kids[0];
    assert!(!state.move_page_by(first, -1));

    // move back to the top level: the page is a root again
    assert!(state.move_page(PAGE_ATLAS, None));
    assert!(state.workspace.borrow().children_of(None).contains(&PAGE_ATLAS));
}

#[test]
fn duplicate_reserves_its_id_range() {
    let args = HandleArgs { blocks: 0, auto_exit_secs: 0.0, bench_pages: 0 };
    let state = AppState::new(&args, None);
    let copy = state
        .duplicate_page(PAGE_GETTING_STARTED)
        .expect("the sample page duplicates");
    let copied_ids: Vec<u64> = {
        let doc = state.doc.borrow();
        doc.page_blocks(core_page_id(copy))
            .iter()
            .map(|b| b.id.as_u64())
            .collect()
    };
    assert!(!copied_ids.is_empty(), "the sample page carries blocks");
    // the whole copied range sits strictly below the next allocation — ids
    // minted without reserving collided with it (M8_FEEDBACK #2)
    let next = state.doc.borrow().next_id_value();
    assert!(
        copied_ids.iter().all(|&id| id < next),
        "next allocation {} would collide with a copied id",
        next
    );
    let fresh = state.doc.borrow_mut().alloc_block_id();
    assert!(!copied_ids.contains(&fresh.as_u64()));
}
