// UI callbacks land here. The controller owns *when* something happens;
// state.rs owns *what* the data looks like; workspace.rs owns the tree
// itself. Nothing here touches DB or disk.
//
// 1.18 global-handle pattern: `ui.global::<UIState>()` borrows the handle, so
// 'static callbacks capture a Weak and upgrade() it at fire time.

use crate::app::state::{
    core_page_id, kind_from_int, AppState, PAGE_GETTING_STARTED, ROW_NEW_PAGE,
};
use crate::core::{BlockId, Change, Command};
use crate::{AppWindow, UIState};
use slint::{ComponentHandle, Global, Model};
use std::rc::Rc;

/// Y offset of the first tree row inside the window: title bar (40) +
/// workspace header (36) + search row (28) + settings row (28) + spacer (8).
pub const TREE_TOP_PX: f32 = 140.0;

/// plain-text marker identifying the app's own drag payload:
/// "slint-notion/block:<id>". Foreign drops (files etc.) don't carry it and
/// are rejected by the row DropAreas.
const BLOCK_DRAG_MIME: &str = "slint-notion/block:";
const PAGE_DRAG_MIME: &str = "slint-notion/page:";

fn block_drag_id(data: &slint::DataTransfer) -> Option<i32> {
    data.plain_text().ok()?.strip_prefix(BLOCK_DRAG_MIME)?.parse().ok()
}

fn page_drag_id(data: &slint::DataTransfer) -> Option<i32> {
    data.plain_text().ok()?.strip_prefix(PAGE_DRAG_MIME)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::BlockKind;

    #[test]
    fn block_drag_payload_roundtrip() {
        let mut data = slint::DataTransfer::default();
        data.set_plain_text(format!("{BLOCK_DRAG_MIME}42").into());
        assert_eq!(block_drag_id(&data), Some(42));
        // foreign payloads (files, external text) never parse to a block
        let mut foreign = slint::DataTransfer::default();
        foreign.set_plain_text("some pasted text".into());
        assert_eq!(block_drag_id(&foreign), None);
        assert_eq!(block_drag_id(&slint::DataTransfer::default()), None);
    }

    #[test]
    fn markdown_shortcuts_convert_and_strip() {
        let conv = |t: &str| markdown_convert(t, Some(BlockKind::Paragraph)).map(|(k, r, _)| (k, r));
        assert_eq!(conv("# "), Some((BlockKind::Heading1, "".to_string())));
        assert_eq!(conv("## Big"), Some((BlockKind::Heading2, "Big".into())));
        assert_eq!(conv("### Small"), Some((BlockKind::Heading3, "Small".into())));
        assert_eq!(conv("- item"), Some((BlockKind::Bullet, "item".into())));
        assert_eq!(conv("* item"), Some((BlockKind::Bullet, "item".into())));
        assert_eq!(conv("1. first"), Some((BlockKind::Numbered, "first".into())));
        assert_eq!(conv("12. x"), Some((BlockKind::Numbered, "x".into())));
        assert_eq!(conv("[] buy"), Some((BlockKind::Todo, "buy".into())));
        assert_eq!(conv("[ ] buy"), Some((BlockKind::Todo, "buy".into())));
        assert_eq!(conv("> note"), Some((BlockKind::Quote, "note".into())));
        assert_eq!(conv("---"), Some((BlockKind::Divider, "".into())));
        assert_eq!(conv("```"), Some((BlockKind::Code, "".into())));
        // non-triggers
        assert_eq!(conv("#no-space"), None);
        assert_eq!(conv("then # "), None);
        assert_eq!(conv("a. x"), None);
        assert_eq!(conv(". x"), None);
        assert_eq!(conv("-"), None);
        assert_eq!(conv("----"), None);
        assert_eq!(conv(""), None);
    }

    #[test]
    fn markdown_shortcut_checked_todo_and_code_exemption() {
        // "[x] " lands checked
        let (kind, rest, checked) = markdown_convert("[x] done", Some(BlockKind::Paragraph)).unwrap();
        assert_eq!(kind, BlockKind::Todo);
        assert_eq!(rest, "done");
        assert_eq!(checked, Some(true));
        // an already-Todo block typing "[x] ": the fn still reports the
        // intent, the caller skips the toggle for same-kind blocks
        let (_, _, checked) = markdown_convert("[x] done", Some(BlockKind::Todo)).unwrap();
        assert_eq!(checked, Some(true));
        // code and divider text is exempt: "# " is legitimate content there
        assert_eq!(markdown_convert("# comment", Some(BlockKind::Code)), None);
        assert_eq!(markdown_convert("---", Some(BlockKind::Divider)), None);
    }
}

pub fn bind(ui: &AppWindow, state: &Rc<AppState>) {
    let g = ui.global::<UIState>();
    state.set_ui(ui.global::<UIState>().as_weak());
    g.set_sidebar_rows(state.sidebar_model());
    g.set_blocks(state.blocks_model());
    g.set_commands(state.commands_model());
    g.set_search_rows(state.search_model());
    g.set_menu_rows(state.menu_model());
    g.set_slash_items(state.slash_model());
    g.set_block_menu_rows(state.block_menu_model());
    let (title, crumb) = state.open_page_info(state.open_page.get());
    g.set_page_title(title.into());
    g.set_page_breadcrumb(crumb.into());
    g.set_renderer_name(renderer_name().into());
    g.set_dark(state.dark_setting());
    g.set_lan_sharing(state.setting_flag("lan.share"));
    g.set_sidebar_open(!state.setting_flag("sidebar.closed"));
    // settings storage row (M8): the database folder, hidden for a
    // memory-only session
    g.set_data_dir(state.data_dir().unwrap_or_default().into());
    g.set_storage_available(state.data_dir().is_some());
    state.update_page_stats();
    if let Some(notice) = state.take_db_notice() {
        g.set_db_notice(notice.into());
    }
    if let Some(notice) = state.take_db_notice() {
        g.set_db_notice(notice.into());
    }
}

/// Select a find hit: route through the hit block's editing input, which
/// recreates the delegate with a pending range selection.
fn apply_find_hit(g: &UIState<'_>, s: &Rc<AppState>, bid: i32, start: usize, end: usize) {
    let text = {
        let d = s.doc.borrow();
        d.block(BlockId(bid as u64))
            .map(|b| b.text.clone())
            .unwrap_or_default()
    };
    g.set_editing_text(text.into());
    g.set_pending_sel_start(start as i32);
    g.set_pending_sel_end(end as i32);
    g.set_editing_id(-1);
    g.set_editing_id(bid);
    g.set_find_target_block(bid);
    let gen = g.get_find_sel_gen() + 1;
    g.set_find_sel_gen(gen);
}

/// Machine-readable renderer id for the A2 measurement JSON
/// ("femtovg"/"skia"/...). The display name lives in `renderer_name`.
pub fn renderer_id() -> &'static str {
    #[cfg(feature = "femtovg")]
    return "femtovg";
    #[cfg(all(not(feature = "femtovg"), feature = "femtovg-wgpu"))]
    return "femtovg-wgpu";
    #[cfg(all(
        not(feature = "femtovg"),
        not(feature = "femtovg-wgpu"),
        any(feature = "skia", feature = "skia-opengl")
    ))]
    return "skia";
    #[cfg(all(
        not(feature = "femtovg"),
        not(feature = "femtovg-wgpu"),
        not(any(feature = "skia", feature = "skia-opengl")),
        feature = "software"
    ))]
    return "software";
    #[cfg(all(
        not(feature = "femtovg"),
        not(feature = "femtovg-wgpu"),
        not(any(feature = "skia", feature = "skia-opengl")),
        not(feature = "software")
    ))]
    return "unknown";
}

fn renderer_name() -> &'static str {
    if cfg!(feature = "femtovg") {
        "FemtoVG · GL"
    } else if cfg!(feature = "femtovg-wgpu") {
        "FemtoVG · wgpu"
    } else if cfg!(feature = "skia") {
        "Skia"
    } else if cfg!(feature = "skia-opengl") {
        "Skia · GL"
    } else if cfg!(feature = "software") {
        "Software"
    } else {
        "unknown"
    }
}

pub fn wire(ui: &AppWindow, state: &Rc<AppState>) {
    let gw = ui.global::<UIState>().as_weak();
    state.set_ui(gw.clone());

    // ---- shell ----
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_toggle_sidebar(move || {
            let g = gw.upgrade().unwrap();
            let open = !g.get_sidebar_open();
            g.set_sidebar_open(open);
            s.record_setting("sidebar.closed", if open { "0" } else { "1" });
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_toggle_theme(move || {
            let g = gw.upgrade().unwrap();
            let dark = !g.get_dark();
            g.set_dark(dark);
            s.set_dark(dark);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_set_dark(move |dark| {
            let g = gw.upgrade().unwrap();
            g.set_dark(dark);
            s.set_dark(dark);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_move_block(move |delta| {
            let g = gw.upgrade().unwrap();
            let cur = g.get_editing_id();
            if cur <= 0 {
                return;
            }
            s.exec_on_open_page(Command::MoveBlock {
                id: BlockId(cur as u64),
                delta,
            });
            refresh_focused_text(&g, &s);
        });
    }

    // ---- grip-handle drag-reorder (Slint DragArea/DropArea) ----
    {
        ui.global::<UIState>().on_block_drag_payload(|id| {
            let mut data = slint::DataTransfer::default();
            data.set_plain_text(format!("{BLOCK_DRAG_MIME}{id}").into());
            data
        });
    }
    // ---- page-tree drag-move (SPEC §八): drop ONTO a page to nest, onto
    // the Workspace header for the top level ----
    {
        ui.global::<UIState>().on_page_drag_payload(|id| {
            let mut data = slint::DataTransfer::default();
            data.set_plain_text(format!("{PAGE_DRAG_MIME}{id}").into());
            data
        });
    }
    {
        let s = state.clone();
        ui.global::<UIState>().on_page_drag_hover(move |data, index| {
            let Some(id) = page_drag_id(&data) else { return false };
            s.page_drop_target_valid(id, index)
        });
    }
    {
        let s = state.clone();
        ui.global::<UIState>().on_page_dropped(move |data, index| {
            let Some(id) = page_drag_id(&data) else { return };
            s.page_dropped(id, index);
        });
    }
    {
        let s = state.clone();
        ui.global::<UIState>().on_block_drag_hover(move |data, index, below| {
            let Some(id) = block_drag_id(&data) else { return false };
            s.can_move_block_to(id, index + (below as i32))
        });
    }
    {
        let s = state.clone();
        ui.global::<UIState>().on_block_dropped(move |data, index, below| {
            let Some(id) = block_drag_id(&data) else { return };
            let _ = s.exec_on_open_page(Command::MoveBlockTo {
                id: BlockId(id as u64),
                index: index + (below as i32),
            });
        });
    }

    {
        let gw = gw.clone();
        ui.global::<UIState>().on_settings_open_requested(move || {
            let g = gw.upgrade().unwrap();
            g.set_settings_open(true);
        });
    }

    {
        let gw = gw.clone();
        ui.global::<UIState>().on_settings_close_requested(move || {
            let g = gw.upgrade().unwrap();
            g.set_settings_open(false);
        });
    }

    // ---- page tree ----
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_node_clicked(move |id| {
            let g = gw.upgrade().unwrap();
            if id == ROW_NEW_PAGE {
                let new_id = s.create_page(None);
                open(&g, &s, new_id);
                g.set_renaming_id(new_id);
                return;
            }
            // favorites/recents/page rows all resolve to a real page
            open(&g, &s, id);
        });
    }

    {
        let s = state.clone();
        ui.global::<UIState>().on_node_expand_toggled(move |id| {
            s.toggle_expanded(id);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_node_context(move |id| {
            let g = gw.upgrade().unwrap();
            s.fill_menu(id);
            g.set_menu_node_id(id);
            // anchor at the row's right edge; MenuRow y comes from the
            // sidebar projection, adjusted by the tree scroll position.
            let row_y = s.sidebar_row_y(id) as f32;
            let y = (TREE_TOP_PX + g.get_tree_viewport_y() + row_y - 4.0)
                .max(48.0)
                .min(g.get_window_h() - 190.0);
            g.set_menu_y(y);
            g.set_menu_x(240.0);
            g.set_menu_open(true);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_menu_action(move |action| {
            let g = gw.upgrade().unwrap();
            let id = g.get_menu_node_id();
            // submenu navigation swaps the rows and keeps the popup open;
            // the taller Move-to list re-anchors so it stays on the window
            if action == crate::app::state::MENU_MOVE_TO {
                s.fill_page_menu_move_to(id);
                let menu_h = g.get_menu_rows().row_count() as f32 * 28.0 + 16.0;
                let y = g.get_menu_y().clamp(
                    48.0,
                    (g.get_window_h() - menu_h - 8.0).max(48.0),
                );
                g.set_menu_y(y);
                return;
            }
            if action == crate::app::state::MENU_BACK {
                s.fill_menu(id);
                return;
            }
            g.set_menu_open(false);
            g.set_menu_node_id(-1);
            match action {
                crate::app::state::MENU_NEW_SUBPAGE => {
                    let new_id = s.create_page(Some(id));
                    open(&g, &s, new_id);
                    g.set_renaming_id(new_id);
                }
                crate::app::state::MENU_RENAME => {
                    g.set_renaming_id(id);
                }
                crate::app::state::MENU_DUPLICATE => {
                    if let Some(new_id) = s.duplicate_page(id) {
                        open(&g, &s, new_id);
                    }
                }
                crate::app::state::MENU_MOVE_UP => {
                    if id > 0 {
                        s.move_page_by(id, -1);
                    }
                }
                crate::app::state::MENU_MOVE_DOWN => {
                    if id > 0 {
                        s.move_page_by(id, 1);
                    }
                }
                crate::app::state::PAGE_MOVE_TO_ROOT => {
                    if id > 0 {
                        s.move_page(id, None);
                    }
                }
                a if (crate::app::state::PAGE_MOVE_TO_BASE
                    ..crate::app::state::PAGE_MOVE_TO_BASE + 1_000_000)
                    .contains(&a) =>
                {
                    if id > 0 {
                        s.move_page(id, Some(a - crate::app::state::PAGE_MOVE_TO_BASE));
                    }
                }
                crate::app::state::MENU_FAVORITE => s.toggle_favorite(id),
                crate::app::state::MENU_DELETE => {
                    let (_title, message) = s.delete_dialog_text(id);
                    g.set_dialog_title("Delete page?".into());
                    g.set_dialog_message(message.into());
                    g.set_dialog_open(true);
                }
                _ => {}
            }
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_page_menu_requested(move || {
            let g = gw.upgrade().unwrap();
            let id = g.get_sidebar_selected_id();
            if !s.workspace.borrow().contains(id) {
                return;
            }
            s.fill_menu(id);
            g.set_menu_node_id(id);
            // under the ⋯ button, top-right of the window
            g.set_menu_x(g.get_window_w() - 224.0);
            g.set_menu_y(44.0);
            g.set_menu_open(true);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_page_create_requested(move || {
            let g = gw.upgrade().unwrap();
            let new_id = s.create_page(None);
            open(&g, &s, new_id);
            g.set_renaming_id(new_id);
        });
    }

    // ---- rename ----
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_rename_committed(move |id, text| {
            let g = gw.upgrade().unwrap();
            g.set_renaming_id(-1);
            s.rename_page(id, &text);
            if s.open_page.get() == id {
                let (title, crumb) = s.open_page_info(id);
                g.set_page_title(title.into());
                g.set_page_breadcrumb(crumb.into());
            }
        });
    }

    {
        let gw = gw.clone();
        ui.global::<UIState>().on_rename_cancelled(move || {
            let g = gw.upgrade().unwrap();
            g.set_renaming_id(-1);
        });
    }

    // ---- delete dialog ----
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_dialog_confirmed(move || {
            let g = gw.upgrade().unwrap();
            g.set_dialog_open(false);
            if let Some(id) = s.pending_delete.take() {
                let was_open = s.delete_page(id);
                if was_open {
                    let fallback = s.workspace.borrow().first_root();
                    match fallback {
                        Some(fid) => open(&g, &s, fid),
                        None => {
                            g.set_page_title("Quire".into());
                            g.set_page_breadcrumb("".into());
                            s.blocks.set_vec(Vec::new());
                            g.set_sidebar_selected_id(0);
                        }
                    }
                }
            }
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_dialog_cancelled(move || {
            let g = gw.upgrade().unwrap();
            g.set_dialog_open(false);
            s.pending_delete.set(None);
        });
    }

    // ---- command palette ----
    {
        let gw = gw.clone();
        ui.global::<UIState>().on_palette_open_requested(move || {
            let g = gw.upgrade().unwrap();
            g.set_palette_focus(0);
            g.set_command_selected_id(-1);
            g.set_palette_open(true);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_palette_query_changed(move || {
            let g = gw.upgrade().unwrap();
            let q = g.get_palette_query().to_string();
            s.set_query(&q);
            g.set_palette_focus(0);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_command_invoked(move || {
            let g = gw.upgrade().unwrap();
            let mut id = g.get_command_selected_id();
            if id < 0 {
                // keyboard Enter: resolve by focus index into the filtered model
                let focus = g.get_palette_focus() as usize;
                id = s.commands.iter().nth(focus).map_or(-1, |c| c.id);
            }
            g.set_palette_open(false);
            g.set_palette_query("".into());
            s.set_query("");
            g.set_palette_focus(0);
            match id {
                1 => {
                    let new_id = s.create_page(None);
                    open(&g, &s, new_id);
                    g.set_renaming_id(new_id);
                }
                2 => {
                    g.set_search_open(true);
                }
                3 => g.set_sidebar_open(!g.get_sidebar_open()),
                4 => g.set_dark(!g.get_dark()),
                5 => g.set_settings_open(true),
                6 => {
                    g.set_renaming_id(g.get_sidebar_selected_id());
                }
                7 => {
                    let cur = s.open_page.get();
                    if let Some(new_id) = s.duplicate_page(cur) {
                        open(&g, &s, new_id);
                    }
                }
                8 => {
                    let cur = s.open_page.get();
                    if s.workspace.borrow().contains(cur) {
                        let (_t, message) = s.delete_dialog_text(cur);
                        g.set_dialog_title("Delete page?".into());
                        g.set_dialog_message(message.into());
                        g.set_dialog_open(true);
                    }
                }
                // fully qualified on purpose: a bare identifier in a match
                // pattern silently becomes a catch-all binding when its
                // import is missing (that exact bug shadowed every command
                // with id >= 9 for one round — rustc warns, tests don't walk
                // this dispatch)
                crate::app::state::CMD_EXPORT_PAGE => export_current_page(&g, &s),
                crate::app::state::CMD_IMPORT_MD => import_markdown_dialog(&g, &s),
                crate::app::state::CMD_COPY_MD => copy_current_page_markdown(&g, &s),
                crate::app::state::CMD_NAV_BACK => navigate(&g, &s, false),
                crate::app::state::CMD_NAV_FORWARD => navigate(&g, &s, true),
                other if other >= crate::app::state::CMD_PAGE_BASE => {
                    open(&g, &s, other - crate::app::state::CMD_PAGE_BASE);
                }
                _ => {}
            }
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_palette_move(move |delta| {
            let g = gw.upgrade().unwrap();
            let count = s.commands.row_count() as i32;
            if count == 0 {
                g.set_palette_focus(0);
                return;
            }
            let next = (g.get_palette_focus() + delta).clamp(0, count - 1);
            g.set_palette_focus(next);
        });
    }

    // ---- search panel ----
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_search_open_requested(move || {
            let g = gw.upgrade().unwrap();
            g.set_search_query("".into());
            s.set_search_query("");
            g.set_search_focus(0);
            g.set_search_open(true);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_search_query_changed(move || {
            let g = gw.upgrade().unwrap();
            let q = g.get_search_query().to_string();
            s.set_search_query(&q);
            g.set_search_focus(0);
            poll_arm(&gw, &s);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_search_move(move |delta| {
            let g = gw.upgrade().unwrap();
            let count = s.search.row_count() as i32;
            if count == 0 {
                g.set_search_focus(0);
                return;
            }
            let next = (g.get_search_focus() + delta).clamp(0, count - 1);
            g.set_search_focus(next);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_search_open_hit(move |page, block| {
            let g = gw.upgrade().unwrap();
            if page <= 0 || block <= 0 {
                return;
            }
            g.set_search_open(false);
            g.set_search_query("".into());
            open(&g, &s, page);
            // open the hit block in edit mode with its text selected — the
            // same mechanism the find bar uses for its navigation
            let (text, len) = {
                let d = s.doc.borrow();
                d.block(crate::core::BlockId(block as u64))
                    .map(|b| (b.text.clone(), b.text.len()))
                    .unwrap_or_default()
            };
            if len > 0 {
                g.set_editing_text(text.into());
                g.set_pending_sel_start(0);
                g.set_pending_sel_end(len as i32);
                g.set_editing_id(-1);
                g.set_editing_id(block);
                let gen = g.get_find_sel_gen() + 1;
                g.set_find_sel_gen(gen);
            }
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_search_invoked(move || {
            let g = gw.upgrade().unwrap();
            let id = search_target(&g, &s);
            g.set_search_open(false);
            g.set_search_query("".into());
            open(&g, &s, id);
        });
    }

    // ---- blocks ----
    {
        let s = state.clone();
        ui.global::<UIState>().on_todo_toggled(move |id| {
            if id <= 0 {
                return;
            }
            // route through the command layer; targeted row update only
            let changes = crate::core::command::exec(
                &mut s.doc.borrow_mut(),
                &mut s.history.borrow_mut(),
                core_page_id(s.open_page.get()),
                Command::ToggleTodoChecked {
                    id: BlockId(id as u64),
                },
            );
            if changes.is_some() {
                let checked = s
                    .doc
                    .borrow()
                    .block(BlockId(id as u64))
                    .map(|b| b.checked)
                    .unwrap_or(false);
                let mut i = 0;
                while let Some(mut row) = s.blocks.row_data(i) {
                    if row.id == id {
                        row.checked = checked;
                        s.blocks.set_row_data(i, row);
                        return;
                    }
                    i += 1;
                }
            }
        });
    }

    // ---- block editing (M4) ----
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_block_activate(move |id| {
            let g = gw.upgrade().unwrap();
            if id <= 0 {
                return;
            }
            flush_pending_edit(&g, &s);
            // a Page block is open-only: activating it opens the child page
            // (the row is never editable, so this is its whole interaction)
            if let Some(child) = s.block_page_ref(id) {
                open(&g, &s, child);
                return;
            }
            let (text, len) = {
                let d = s.doc.borrow();
                d.block(BlockId(id as u64))
                    .map(|b| (b.text.clone(), b.text.len()))
                    .unwrap_or_default()
            };
            g.set_editing_text(text.into());
            g.set_pending_caret(len as i32);
            g.set_editing_id(id);
        });
    }

    // rich paste (SPEC §二十七): the clipboard's markdown structure lands
    // as blocks. The clipboard read is direct Win32 FFI (microseconds — a
    // Get-Clipboard subprocess measured 7-10 s on the dev desktop); a false
    // return lets the key fall through to the native plain paste.
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_rich_paste(move |id| -> bool {
            let g = gw.upgrade().unwrap();
            if id <= 0 {
                return false;
            }
            flush_pending_edit(&g, &s);
            let Some(text) = crate::platform::read_clipboard() else {
                return false;
            };
            let Some(parsed) =
                crate::services::import_service::parse_if_block_structure(&text)
            else {
                return false;
            };
            if s.paste_block_structure(id, &parsed) {
                // the row's content changed under the input; end editing so
                // the rendered row (and its marks) take over
                g.set_editing_id(-1);
                true
            } else {
                false
            }
        });
    }

    // persistence: arm the flush timer hook; every recorded batch restarts
    // it, so the write lands once the user has been quiet for 600 ms
    {
        let t: &'static slint::Timer = Box::leak(Box::new(slint::Timer::default()));
        let sw = std::rc::Rc::downgrade(&state);
        state.install_flush_hook(Box::new(move || {
            let sw = sw.clone();
            t.start(
                slint::TimerMode::SingleShot,
                std::time::Duration::from_millis(600),
                move || {
                    if let Some(s) = sw.upgrade() {
                        s.persistence_force_flush();
                    }
                },
            );
        }));
    }

    {
        let s = state.clone();
        ui.global::<UIState>().on_save_requested(move || {
            s.persistence_force_flush();
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_nav_back_requested(move || {
            let g = gw.upgrade().unwrap();
            navigate(&g, &s, false);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_nav_forward_requested(move || {
            let g = gw.upgrade().unwrap();
            navigate(&g, &s, true);
        });
    }

    // settings storage row (M8): open the database folder / snapshot now
    {
        let s = state.clone();
        ui.global::<UIState>().on_open_data_folder(move || {
            if let Some(dir) = s.data_dir() {
                crate::platform::open_folder(&dir);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_backup_now(move || {
            let g = gw.upgrade().unwrap();
            let text = match s.backup_now() {
                Ok(msg) => msg,
                Err(e) => format!("Backup failed: {e}"),
            };
            g.set_db_notice(text.into());
        });
    }

    // debounce the typing commit: each keystroke restarts the timer; the
    // Timer must outlive this scope (leaked, like the bench timers)
    {
        let gw = gw.clone();
        let s = state.clone();
        let t: &'static slint::Timer = Box::leak(Box::new(slint::Timer::default()));
        ui.global::<UIState>()
            .on_editing_changed(move |row_y, row_h, content_x| {
                let gw = gw.clone();
                let s = s.clone();
                if let Some(g) = gw.upgrade() {
                    let text = g.get_editing_text().to_string();
                    // markdown line-shortcuts convert before anything else
                    // looks at the text (slash filter included); the block
                    // kind gates which triggers are eligible
                    let editing = g.get_editing_id();
                    if editing > 0 {
                        let current = s.block_kind(editing);
                        if let Some((kind, cleaned, checked)) =
                            markdown_convert(&text, current)
                        {
                            let bid = BlockId(editing as u64);
                            let mut cmds = vec![
                                Command::ReplaceText { id: bid, text: cleaned.clone() },
                                Command::SetBlockType { id: bid, kind },
                            ];
                            // "[x] " lands checked — unless it already is one
                            if checked == Some(true) && current != Some(crate::core::BlockKind::Todo) {
                                cmds.push(Command::ToggleTodoChecked { id: bid });
                            }
                            let _ = s.exec_all_on_open_page(cmds);
                            g.set_slash_open(false);
                            g.set_editing_text(cleaned.into());
                            g.set_pending_caret(0);
                            g.set_editing_id(editing);
                            return;
                        }
                    }
                    // "/" at block start opens the slash menu with the rest of
                    // the line as the filter (SPEC §十五); anchored below the
                    // editing block. The "+"-handle insert menu filters on
                    // the whole line instead and keeps its original anchor;
                    // the page picker (Link to page) filters the same way.
                    let pick_mode = g.get_slash_pick_page() && g.get_slash_open();
                    let insert_mode = g.get_slash_insert() && g.get_slash_open();
                    if pick_mode {
                        s.open_slash_pick(&text);
                        g.set_slash_filter(text.into());
                        g.set_slash_focus(0);
                    } else if insert_mode {
                        s.open_slash_insert(&text);
                        g.set_slash_filter(text.into());
                        g.set_slash_focus(0);
                    } else if let Some(filter) = text.strip_prefix('/') {
                        s.open_slash(filter);
                        g.set_slash_filter(filter.into());
                        g.set_slash_focus(0);
                        let scroll = g.get_editor_scroll_y();
                        let edge = if g.get_sidebar_open() { 260.0 } else { 0.0 };
                        let y = (40.0 + (row_y as f32) - scroll + (row_h as f32) + 4.0)
                            .clamp(48.0, g.get_window_h() - 350.0);
                        g.set_slash_x(edge + (content_x as f32));
                        g.set_slash_y(y);
                        g.set_slash_open(true);
                    } else {
                        g.set_slash_open(false);
                    }
                }
                t.start(
                    slint::TimerMode::SingleShot,
                    std::time::Duration::from_millis(300),
                    move || {
                        if let Some(g) = gw.upgrade() {
                            flush_pending_edit(&g, &s);
                        }
                    },
                );
            });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_enter_at_caret(move |caret| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            let cur = g.get_editing_id();
            if cur <= 0 {
                return;
            }
            let changes = s.exec_on_open_page(Command::SplitBlock {
                id: BlockId(cur as u64),
                caret: caret.max(0) as usize,
            });
            let new_id = changes.as_deref().and_then(find_inserted_id);
            if let Some(nid) = new_id {
                focus_block(&g, &s, nid, 0);
            }
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_backspace_at_start(move || {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            let cur = g.get_editing_id();
            if cur <= 0 {
                return;
            }
            let page = core_page_id(s.open_page.get());
            // capture the previous block so the caret can land at the seam
            let prev = {
                let d = s.doc.borrow();
                let blocks = d.page_blocks(page);
                blocks
                    .iter()
                    .position(|b| b.id.0 as i32 == cur)
                    .and_then(|i| i.checked_sub(1))
                    .and_then(|i| blocks.get(i))
                    .map(|b| (b.id.0 as i32, b.text.len() as i32))
            };
            let changes = s.exec_on_open_page(Command::MergeBackward {
                id: BlockId(cur as u64),
            });
            if changes.is_some() {
                match prev {
                    Some((pid, plen)) => focus_block(&g, &s, pid, plen),
                    None => g.set_editing_id(-1), // first block deleted
                }
            }
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_focus_move(move |delta| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            let cur = g.get_editing_id();
            if cur <= 0 {
                return;
            }
            let page = core_page_id(s.open_page.get());
            let target = {
                let d = s.doc.borrow();
                let blocks = d.page_blocks(page);
                blocks
                    .iter()
                    .position(|b| b.id.0 as i32 == cur)
                    .map(|i| i as i32 + delta)
                    .and_then(|t| {
                        if t < 0 {
                            None
                        } else {
                            blocks
                                .get(t as usize)
                                .map(|b| (b.id.0 as i32, b.text.len() as i32, delta < 0))
                        }
                    })
            };
            if let Some((tid, tlen, up)) = target {
                focus_block(&g, &s, tid, if up { tlen } else { 0 });
            }
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_cancel_editing(move || {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            g.set_editing_id(-1);
        });
    }

    // ---- slash menu keyboard + apply ----
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_slash_move(move |delta| {
            let g = gw.upgrade().unwrap();
            let count = s.slash_focus_count();
            if count == 0 {
                return;
            }
            let next = s.slash_next_focus(g.get_slash_focus(), delta);
            g.set_slash_focus(next);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_slash_apply_selected(move || {
            let g = gw.upgrade().unwrap();
            let focus = g.get_slash_focus();
            // page-picker mode: the focused row IS a page — convert the empty
            // line into a Link-to-page block pointing at it
            if g.get_slash_pick_page() {
                let id = g.get_editing_id();
                if let Some(page) = s.slash_selected_page(focus) {
                    if id > 0 {
                        s.create_page_link_block(id, page);
                    }
                }
                g.set_slash_open(false);
                g.set_slash_pick_page(false);
                g.set_editing_id(-1);
                return;
            }
            let kind = match s.slash_selected_kind(focus) {
                Some(k) => k,
                None => return,
            };
            let text = g.get_editing_text().to_string();
            let insert_mode = g.get_slash_insert();
            // insert mode: the "+" menu discards the typed filter text;
            // slash mode: strip "/filter" (the menu only triggers on the
            // block's first token; a space closes the menu before this point)
            let filter_len = g.get_slash_filter().len();
            let cleaned = if insert_mode {
                String::new()
            } else if text.len() >= 1 + filter_len && text.is_char_boundary(1 + filter_len) {
                text[1 + filter_len..].to_string()
            } else {
                String::new()
            };
            let id = g.get_editing_id();
            if id <= 0 {
                return;
            }
            // Page is insert-menu-only: applying it creates the child page
            // and converts the (empty insert-mode) row; no text edit involved
            if kind == crate::core::BlockKind::Page {
                s.create_page_block(id);
                g.set_slash_open(false);
                g.set_slash_insert(false);
                return;
            }
            // Link to page switches the popup to the page picker: the typed
            // filter is discarded, the anchor stays, and the next pick
            // converts the line
            if kind == crate::core::BlockKind::Link {
                let _ = s.exec_on_open_page(Command::ReplaceText {
                    id: BlockId(id as u64),
                    text: String::new(),
                });
                g.set_editing_text("".into());
                g.set_pending_caret(0);
                s.open_slash_pick("");
                g.set_slash_filter("".into());
                g.set_slash_focus(0);
                g.set_slash_pick_page(true);
                return;
            }
            let _ = s.exec_on_open_page(Command::ReplaceText {
                id: BlockId(id as u64),
                text: cleaned.clone(),
            });
            let _ = s.exec_on_open_page(Command::SetBlockType {
                id: BlockId(id as u64),
                kind,
            });
            g.set_slash_open(false);
            g.set_slash_insert(false);
            g.set_editing_text(cleaned.clone().into());
            g.set_pending_caret(cleaned.len() as i32);
            g.set_editing_id(id);
        });
    }

    {
        let gw = gw.clone();
        ui.global::<UIState>().on_slash_close(move || {
            let g = gw.upgrade().unwrap();
            g.set_slash_open(false);
        });
    }

    // ---- block handle menu (+/⋮⋮) and clipboard ----
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>()
            .on_block_menu_opened(move |id, handle_y, content_x| {
                let g = gw.upgrade().unwrap();
                s.fill_block_menu();
                // anchor beside the handle: window y = top bar + list-layout
                // y (pre-scroll) - scroll; x aligns with the text column.
                // The height follows the row count so the tall root menu
                // (and its submenus) never anchor below the window.
                let scroll = g.get_editor_scroll_y();
                let edge = if g.get_sidebar_open() { 260.0 } else { 0.0 };
                let menu_h = g.get_block_menu_rows().row_count() as f32 * 28.0 + 16.0;
                let y = (40.0 + handle_y as f32 - scroll + 2.0)
                    .clamp(48.0, (g.get_window_h() - menu_h).max(48.0));
                g.set_block_menu_x(edge + content_x as f32 + 2.0);
                g.set_block_menu_y(y);
                g.set_block_menu_open_id(id);
            });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_block_menu_action(move |action| {
            let g = gw.upgrade().unwrap();
            let id = g.get_block_menu_open_id();
            if id <= 0 {
                return;
            }
            // submenu navigation swaps the rows and keeps the popup open
            if action == 7 {
                s.fill_block_menu_turn_into(id);
                return;
            }
            if action == 8 {
                s.fill_block_menu();
                return;
            }
            if action == 10 {
                s.fill_block_menu_move_to();
                return;
            }
            if action == 11 {
                s.fill_block_menu_colors(id, false);
                return;
            }
            if action == 12 {
                s.fill_block_menu_colors(id, true);
                return;
            }
            // color picks stay open (Notion-style live preview); everything
            // else closes first
            let color_pick = (AppState::COLOR_TEXT_BASE..AppState::COLOR_BG_BASE + 100)
                .contains(&action);
            if !color_pick {
                g.set_block_menu_open_id(-1);
            }
            if action == 9 {
                crate::platform::copy_to_clipboard(&s.block_link(id));
                return;
            }
            if (AppState::MOVE_TO_BASE..AppState::COLOR_TEXT_BASE).contains(&action) {
                let page = action - AppState::MOVE_TO_BASE;
                if s.move_block_to_page(id, page) && g.get_editing_id() == id {
                    g.set_editing_id(-1);
                }
                return;
            }
            if (AppState::COLOR_TEXT_BASE..AppState::COLOR_BG_BASE).contains(&action) {
                s.set_block_color_slot(id, false, action - AppState::COLOR_TEXT_BASE);
                // refill so the current-pick check follows the pick
                s.fill_block_menu_colors(id, false);
                return;
            }
            if (AppState::COLOR_BG_BASE..AppState::COLOR_BG_BASE + 100).contains(&action) {
                s.set_block_color_slot(id, true, action - AppState::COLOR_BG_BASE);
                s.fill_block_menu_colors(id, true);
                return;
            }
            match action {
                1 => {
                    let _ = s.exec_on_open_page(Command::MoveBlock {
                        id: BlockId(id as u64),
                        delta: -1,
                    });
                }
                2 => {
                    let _ = s.exec_on_open_page(Command::MoveBlock {
                        id: BlockId(id as u64),
                        delta: 1,
                    });
                }
                3 => {
                    // a Page block duplicates its child page too, so the copy
                    // never shares a target with the original
                    if s
                        .duplicate_page_block(id)
                        .is_none()
                    {
                        let _ = s.exec_on_open_page(Command::DuplicateBlock {
                            id: BlockId(id as u64),
                        });
                    }
                }
                4 => s.copy_block(id),
                5 => {
                    s.paste_below(id);
                }
                6 => {
                    // deleting a Page block takes its child page with it; a
                    // Link block's target is unowned and survives
                    let kind = s.block_kind_of(id);
                    let page_ref = s.block_page_ref(id);
                    let _ = s.exec_on_open_page(Command::DeleteBlock {
                        id: BlockId(id as u64),
                    });
                    if kind == Some(crate::core::BlockKind::Page) {
                        if let Some(child) = page_ref {
                            s.delete_page(child);
                        }
                    }
                    if g.get_editing_id() == id {
                        g.set_editing_id(-1);
                    }
                }
                a if (100..200).contains(&a) => {
                    // leaving Page/Link via Turn-into drops the reference; a
                    // Page's child page survives in the tree, unowned
                    let old_kind = s.block_kind_of(id);
                    let _ = s.exec_on_open_page(Command::SetBlockType {
                        id: BlockId(id as u64),
                        kind: kind_from_int(a - 100),
                    });
                    if matches!(
                        old_kind,
                        Some(crate::core::BlockKind::Page)
                            | Some(crate::core::BlockKind::Link)
                    ) {
                        s.clear_block_ref(id);
                    }
                }
                _ => {}
            }
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_block_plus(move |id, row_bottom, content_x| {
            let g = gw.upgrade().unwrap();
            if id <= 0 {
                return;
            }
            let changes = s.exec_on_open_page(Command::InsertBlockAfter {
                id: BlockId(id as u64),
                kind: crate::core::BlockKind::Paragraph,
                text: String::new(),
            });
            if let Some(nid) = changes.as_deref().and_then(find_inserted_id) {
                focus_block(&g, &s, nid, 0);
                // Notion's "+": the new empty line gains the insert menu.
                // Picking a row types the block; clicking away (or Escape)
                // keeps the empty paragraph, exactly like Notion.
                s.open_slash_insert("");
                let count = g.get_slash_items().row_count() as f32;
                let menu_h = count * 32.0 + 8.0;
                let scroll = g.get_editor_scroll_y();
                let edge = if g.get_sidebar_open() { 260.0 } else { 0.0 };
                let y = (40.0 + row_bottom as f32 - scroll + 4.0)
                    .clamp(48.0, (g.get_window_h() - menu_h - 8.0).max(48.0));
                g.set_slash_x(edge + content_x as f32);
                g.set_slash_y(y);
                g.set_slash_filter("".into());
                g.set_slash_focus(0);
                g.set_slash_insert(true);
                g.set_slash_open(true);
            }
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>()
            .on_toggle_mark(move |kind, start, end| {
                let g = gw.upgrade().unwrap();
                flush_pending_edit(&g, &s);
                let cur = g.get_editing_id();
                if cur <= 0 {
                    return;
                }
                let kind = match kind {
                    0 => Some(crate::core::MarkKind::Bold),
                    1 => Some(crate::core::MarkKind::Italic),
                    2 => Some(crate::core::MarkKind::Strike),
                    3 => Some(crate::core::MarkKind::Code),
                    _ => None,
                };
                if let Some(kind) = kind {
                    let _ = s.exec_on_open_page(Command::ToggleMark {
                        id: BlockId(cur as u64),
                        start: start.min(end).max(0) as usize,
                        end: end.max(start).max(0) as usize,
                        kind,
                        url: String::new(),
                    });
                }
            });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_open_link(move |url| {
            let g = gw.upgrade().unwrap();
            let url = url.to_string();
            if url.is_empty() {
                return;
            }
            // internal anchors first: a block link jumps to the block (its
            // page opens when it isn't the current one); a page link opens
            // the page. Unknown quire URLs are swallowed, not shelled out.
            if let Some(rest) = url.strip_prefix("quire://block/") {
                if let Ok(bid) = rest.parse::<u64>() {
                    let (bpage, text_len) = {
                        let d = s.doc.borrow();
                        d.block(BlockId(bid))
                            .map(|b| (b.page.0 as i32, b.text.len() as i32))
                            .unwrap_or((0, 0))
                    };
                    if bpage > 0 && s.workspace.borrow().contains(bpage) {
                        flush_pending_edit(&g, &s);
                        open(&g, &s, bpage);
                        // -1 first: recreate the delegate so the input takes over
                        g.set_editing_id(-1);
                        focus_block(&g, &s, bid as u32 as i32, text_len);
                    }
                }
                return;
            }
            if let Some(rest) = url.strip_prefix("quire://page/") {
                if let Ok(pid) = rest.parse::<i32>() {
                    open(&g, &s, pid);
                }
                return;
            }
            // Windows shell open; cfg-gated so other targets simply no-op
            #[cfg(target_os = "windows")]
            {
                let _ = std::process::Command::new("cmd")
                    .args(["/C", "start", "", &url])
                    .spawn();
            }
            #[cfg(not(target_os = "windows"))]
            {
                eprintln!("quire: open {url} (not supported on this platform yet)");
            }
        });
    }

    {
        let gw = gw.clone();
        ui.global::<UIState>().on_close_db_notice(move || {
            let g = gw.upgrade().unwrap();
            g.set_db_notice("".into());
        });
    }

    // ---- in-page find (Ctrl+F) ----
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_find_start(move || {
            let g = gw.upgrade().unwrap();
            let term = g.get_find_term().to_string();
            s.find_start(&term);
            g.set_find_label(s.find_label().into());
            // jump to the first hit right away: a "0 / N" position selects
            // nothing, which read as a broken find in the A4 sweep
            if s.find_label().starts_with("0 / ") {
                flush_pending_edit(&g, &s);
                if let Some((bid, start, end)) = s.find_step(true) {
                    apply_find_hit(&g, &s, bid, start, end);
                }
                g.set_find_label(s.find_label().into());
            }
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_find_step(move |delta| {
            let g = gw.upgrade().unwrap();
            if !g.get_find_open() {
                return;
            }
            flush_pending_edit(&g, &s);
            if let Some((bid, start, end)) = s.find_step(delta < 0) {
                apply_find_hit(&g, &s, bid, start, end);
            } else {
                g.set_find_label(s.find_label().into());
            }
        });
    }

    {
        let s = state.clone();
        ui.global::<UIState>().on_find_close(move || {
            s.find_close();
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_duplicate_block(move |id| {
            let g = gw.upgrade().unwrap();
            if id <= 0 {
                return;
            }
            flush_pending_edit(&g, &s);
            if let Some(nid) = s
                .exec_on_open_page(Command::DuplicateBlock { id: BlockId(id as u64) })
                .as_deref()
                .and_then(find_inserted_id)
            {
                focus_block(&g, &s, nid, 0);
            }
        });
    }

    // ---- list nesting (Tab / Shift+Tab) ----
    {
        let gw = gw.clone();
        let s = state.clone();
        let nest = move |g: &UIState<'_>, s: &Rc<AppState>, id: i32, indent: bool| {
            flush_pending_edit(g, s);
            let cmd = if indent {
                Command::IndentList { id: BlockId(id as u64) }
            } else {
                Command::OutdentList { id: BlockId(id as u64) }
            };
            if s.exec_on_open_page(cmd).is_some() {
                refresh_focused_text(g, s);
            }
        };
        ui.global::<UIState>().on_indent_list(move |id| {
            let g = gw.upgrade().unwrap();
            nest(&g, &s, id, true);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_outdent_list(move |id| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            if s.exec_on_open_page(Command::OutdentList { id: BlockId(id as u64) }).is_some() {
                refresh_focused_text(&g, &s);
            }
        });
    }

    {
        let s = state.clone();
        ui.global::<UIState>().on_toggle_lan_sharing(move || {
            let on = !s.setting_flag("lan.share");
            s.record_setting("lan.share", if on { "1" } else { "0" });
        });
    }

    // ---- page title in-place editing ----
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_title_commit(move |text| {
            let g = gw.upgrade().unwrap();
            let title = text.trim().to_string();
            let cur = s.open_page.get();
            g.set_title_editing(false);
            if cur <= 0 {
                return;
            }
            if title.is_empty() {
                // empty title reverts to the stored one
                let t = s.workspace.borrow().title_of(cur).unwrap_or("").to_string();
                g.set_page_title(t.into());
                return;
            }
            s.rename_page(cur, &title);
            g.set_page_title(title.into());
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_title_cancel(move || {
            let g = gw.upgrade().unwrap();
            g.set_title_editing(false);
            let cur = s.open_page.get();
            let t = s.workspace.borrow().title_of(cur).unwrap_or("").to_string();
            g.set_page_title(t.into());
        });
    }

    // ---- link dialog (M6) ----
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_link_apply(move || {
            let g = gw.upgrade().unwrap();
            let mut url = g.get_link_url().to_string();
            g.set_link_open(false);
            let cur = g.get_editing_id();
            if cur <= 0 {
                return;
            }
            // bare domains get an https scheme so the browser opens them
            let trimmed = url.trim();
            if !trimmed.is_empty()
                && !trimmed.contains("://")
                && !trimmed.starts_with("mailto:")
            {
                url = format!("https://{trimmed}");
            }
            let _ = s.exec_on_open_page(Command::ToggleMark {
                id: BlockId(cur as u64),
                start: g.get_link_start().max(0) as usize,
                end: g.get_link_end().max(0) as usize,
                kind: crate::core::MarkKind::Link,
                url,
            });
            refresh_focused_text(&g, &s);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_link_remove(move || {
            let g = gw.upgrade().unwrap();
            g.set_link_open(false);
            let cur = g.get_editing_id();
            if cur <= 0 {
                return;
            }
            // a covering Link mark is toggled off by the same range
            let _ = s.exec_on_open_page(Command::ToggleMark {
                id: BlockId(cur as u64),
                start: g.get_link_start().max(0) as usize,
                end: g.get_link_end().max(0) as usize,
                kind: crate::core::MarkKind::Link,
                url: String::new(),
            });
            refresh_focused_text(&g, &s);
        });
    }

    {
        let gw = gw.clone();
        ui.global::<UIState>().on_link_cancel(move || {
            let g = gw.upgrade().unwrap();
            g.set_link_open(false);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_undo_requested(move || {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            s.undo_open_page();
            refresh_focused_text(&g, &s);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_redo_requested(move || {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            s.redo_open_page();
            refresh_focused_text(&g, &s);
        });
    }
}

/// Arm the one-shot poll timer for an in-flight async search. Each poll
/// that comes back empty re-arms; a landed result stops the cycle.
fn poll_arm(gw: &slint::Weak<UIState<'static>>, s: &Rc<AppState>) {
    if !s.search_in_flight() {
        return;
    }
    let t: &'static slint::Timer = Box::leak(Box::new(slint::Timer::default()));
    let gw = gw.clone();
    let s = s.clone();
    t.start(
        slint::TimerMode::SingleShot,
        std::time::Duration::from_millis(30),
        move || {
            let g = gw.upgrade().unwrap();
            if let Some(rows) = s.poll_search() {
                s.search.set_vec(rows);
                g.set_search_focus(0);
            } else if s.search_in_flight() {
                poll_arm(&gw, &s);
            }
        },
    );
}

/// Commit the live editing text as a `ReplaceText` command (no-op when the
/// text is unchanged). Called by the debounce timer and before every
/// structural operation so undo history stays consistent.
fn flush_pending_edit(g: &UIState<'_>, state: &Rc<AppState>) {
    let editing = g.get_editing_id();
    if editing <= 0 {
        return;
    }
    let id = BlockId(editing as u64);
    let text = g.get_editing_text().to_string();
    let applied = state.exec_editor(Command::ReplaceText {
        id,
        text: text.clone(),
    });
    if applied.is_some() {
        // targeted row sync; no delegate rebuild
        let mut i = 0;
        while let Some(mut row) = state.blocks.row_data(i) {
            if row.id == editing {
                row.text = text.into();
                state.blocks.set_row_data(i, row);
                return;
            }
            i += 1;
        }
    }
}

/// Move the live editor onto another block with the caret at `caret` bytes.
fn focus_block(g: &UIState<'_>, state: &Rc<AppState>, id: i32, caret: i32) {
    let (text, len) = {
        let d = state.doc.borrow();
        d.block(BlockId(id as u64))
            .map(|b| (b.text.clone(), b.text.len()))
            .unwrap_or_default()
    };
    g.set_editing_text(text.into());
    g.set_pending_caret(caret.clamp(0, len as i32));
    g.set_editing_id(id);
}

/// After undo/redo: keep the editor on its block if it still exists.
fn refresh_focused_text(g: &UIState<'_>, state: &Rc<AppState>) {
    let cur = g.get_editing_id();
    if cur <= 0 {
        return;
    }
    let text = {
        let d = state.doc.borrow();
        d.block(BlockId(cur as u64)).map(|b| b.text.clone())
    };
    match text {
        Some(t) => {
            g.set_editing_text(t.into());
            g.set_pending_caret(-1);
        }
        None => g.set_editing_id(-1),
    }
}

/// Import every (title, markdown) pair from a LAN pull as new pages.
/// Returns the number of pages imported.
pub fn import_lan_pages(
    g: &UIState<'_>,
    state: &Rc<AppState>,
    url: &str,
    pages: Vec<(String, String)>,
) -> usize {
    let mut imported = 0;
    for (title, md) in pages {
        let safe_title = if title.trim().is_empty() {
            "Imported".to_string()
        } else {
            title.trim().to_string()
        };
        let new_id = state.create_page(None);
        let core_page = crate::core::Page {
            id: crate::core::PageId(new_id as u32 as u64),
            title: safe_title.clone(),
            parent: None,
            order: state.page_order_of(new_id),
            favorite: false,
            expanded: false,
        };
        let changes = {
            let mut doc = state.doc.borrow_mut();
            let mut alloc = || doc.alloc_block_id();
            crate::services::import_service::import_markdown(&md, &core_page, &mut alloc)
        };
        let rest = changes.into_iter().skip(1).collect::<Vec<_>>();
        {
            let mut d = state.doc.borrow_mut();
            d.apply(&rest);
        }
        state.record(rest);
        state.rename_page(new_id, &safe_title);
        imported += 1;
    }
    if imported > 0 {
        g.set_db_notice(format!("Imported {imported} page(s) from {url}").into());
        open(g, state, state.open_page.get());
        state.reproject_blocks();
    }
    imported
}

/// Export the open page's blocks to a .md file via the native save dialog.
/// "Copy Page as Markdown" (palette): the page through the exporter onto
/// the clipboard. The FFI write path is mandatory here — a markdown page
/// routinely carries CJK, which clip.exe's OEM stdin garbles (ADR-0025).
fn copy_current_page_markdown(g: &UIState<'_>, s: &Rc<AppState>) {
    let page = s.open_page.get();
    let md = {
        let d = s.doc.borrow();
        crate::services::export_service::export_page(d.page_blocks(core_page_id(page)))
    };
    let notice = if md.trim().is_empty() {
        "This page has nothing to copy yet.".to_string()
    } else if crate::platform::copy_to_clipboard(&md) {
        "Page copied as Markdown.".to_string()
    } else {
        "Could not reach the clipboard.".to_string()
    };
    g.set_db_notice(notice.into());
}

fn export_current_page(g: &UIState<'_>, state: &Rc<AppState>) {
    let page = state.open_page.get();
    let title = state
        .workspace
        .borrow()
        .title_of(page)
        .unwrap_or("page")
        .to_string();
    let md = {
        let d = state.doc.borrow();
        crate::services::export_service::export_page(d.page_blocks(core_page_id(page)))
    };
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Markdown", &["md"])
        .set_file_name(&format!("{}.md", title))
        .save_file()
    {
        match std::fs::write(&path, md) {
            Ok(()) => eprintln!("quire: exported {}", path.display()),
            Err(e) => eprintln!("quire: export failed: {e}"),
        }
    }
    let _ = g;
}

/// Import a .md file as a new page via the native open dialog.
fn import_markdown_dialog(g: &UIState<'_>, state: &Rc<AppState>) {
    if let Some(path) = rfd::FileDialog::new()
        .add_filter("Markdown", &["md"])
        .pick_file()
    {
        import_from_path(g, state, &path);
    }
}

/// Import `path` as a new page, record it, and open it. Shared by the
/// palette command and the .md file-association dispatch (--open).
pub fn import_from_path(g: &UIState<'_>, state: &Rc<AppState>, path: &std::path::Path) {
    let Ok(src) = std::fs::read_to_string(path) else {
        eprintln!("quire: import failed: cannot read {}", path.display());
        return;
    };
    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Imported".into());

    let new_id = state.create_page(None);
    g.set_db_notice(format!("Imported {} as a new page", title).into());
    let core_page = crate::core::Page {
        id: crate::core::PageId(new_id as u32 as u64),
        title: title.clone(),
        parent: None,
        order: state.page_order_of(new_id),
        favorite: false,
        expanded: false,
    };
    let changes = {
        let mut doc = state.doc.borrow_mut();
        let mut alloc = || doc.alloc_block_id();
        crate::services::import_service::import_markdown(&src, &core_page, &mut alloc)
    };
    // the service's PageCreated replaces create_page's "Untitled" record
    let rest = changes.into_iter().skip(1).collect::<Vec<_>>();
    {
        let mut d = state.doc.borrow_mut();
        d.apply(&rest);
    }
    state.record(rest);
    state.rename_page(new_id, &title);
    open(g, state, new_id);
}

fn find_inserted_id(changes: &[Change]) -> Option<i32> {
    changes.iter().find_map(|c| match c {
        Change::BlockInserted(b) => Some(b.id.0 as i32),
        _ => None,
    })
}

/// Markdown line-shortcuts (ADR-0022): typing a trigger prefix + space (or
/// the exact token) converts the block being edited, with the trigger
/// stripped. The slash menu and Turn-into list omit every kind reachable
/// this way, so the symbol is the only path to them. Returns
/// `(kind, remaining text, set-checked)`. Code and divider blocks never
/// convert — their text legitimately starts with these characters.
fn markdown_convert(
    text: &str,
    current: Option<crate::core::BlockKind>,
) -> Option<(crate::core::BlockKind, String, Option<bool>)> {
    use crate::core::BlockKind;
    if matches!(current, Some(BlockKind::Code) | Some(BlockKind::Divider)) {
        return None;
    }
    let conv = |kind, rest: &str| Some((kind, rest.to_string(), None));
    if let Some(rest) = text.strip_prefix("### ") {
        conv(BlockKind::Heading3, rest)
    } else if let Some(rest) = text.strip_prefix("## ") {
        conv(BlockKind::Heading2, rest)
    } else if let Some(rest) = text.strip_prefix("# ") {
        conv(BlockKind::Heading1, rest)
    } else if let Some(rest) = text.strip_prefix("- ").or_else(|| text.strip_prefix("* ")) {
        conv(BlockKind::Bullet, rest)
    } else if let Some(rest) = text.strip_prefix("[x] ") {
        Some((BlockKind::Todo, rest.to_string(), Some(true)))
    } else if let Some(rest) = text.strip_prefix("[] ").or_else(|| text.strip_prefix("[ ] ")) {
        conv(BlockKind::Todo, rest)
    } else if let Some(rest) = text.strip_prefix("> ") {
        conv(BlockKind::Quote, rest)
    } else if let Some(rest) = numbered_prefix(text) {
        conv(BlockKind::Numbered, rest)
    } else if text == "---" {
        conv(BlockKind::Divider, "")
    } else if text == "```" {
        conv(BlockKind::Code, "")
    } else {
        None
    }
}

/// `12. rest` → `rest`; digits + ". " only.
fn numbered_prefix(text: &str) -> Option<&str> {
    let dot = text.find(". ")?;
    if dot == 0 || !text[..dot].bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.get(dot + 2..)
}

/// Open a page, sync the top bar, and highlight it in the tree.
fn open(g: &UIState<'_>, state: &Rc<AppState>, id: i32) {
    if !state.workspace.borrow().contains(id) {
        return;
    }
    // the typing flush is debounced 300 ms and resolves against the open
    // page, so commit it before the page under it changes
    flush_pending_edit(g, state);
    state.nav_record(state.open_page.get(), id);
    state.open_page(id);
    show_open_page(g, state);
}

/// Go Back / Go Forward (SPEC §十六): move along the session's page history.
/// `nav_step` already parked the page being left onto the opposite stack, so
/// this must not record again.
fn navigate(g: &UIState<'_>, state: &Rc<AppState>, forward: bool) {
    flush_pending_edit(g, state);
    let Some(id) = state.nav_step(forward) else {
        return;
    };
    state.open_page(id);
    show_open_page(g, state);
}

/// Paint the shell for whatever `state.open_page` now holds.
fn show_open_page(g: &UIState<'_>, state: &Rc<AppState>) {
    let id = state.open_page.get();
    let (title, crumb) = state.open_page_info(id);
    g.set_page_title(title.into());
    g.set_page_breadcrumb(crumb.into());
    g.set_sidebar_selected_id(id);
}

fn search_target(g: &UIState<'_>, state: &Rc<AppState>) -> i32 {
    let mut id = g.get_search_selected_id();
    if id < 0 {
        let focus = g.get_search_focus() as usize;
        id = state.search.iter().nth(focus).map_or(-1, |r| r.page_id);
    }
    if id < 0 {
        id = PAGE_GETTING_STARTED;
    }
    id
}

/// Benchmark scene G: switch to the next bench page (called from a timer).
pub fn bench_switch_next(
    ui: &AppWindow,
    state: &Rc<AppState>,
    bench_ids: &[i32],
    cursor: &mut usize,
) {
    if bench_ids.is_empty() {
        return;
    }
    let id = bench_ids[*cursor % bench_ids.len()];
    *cursor = (*cursor + 1) % bench_ids.len();
    let g = ui.global::<UIState>();
    open(&g, state, id);
}

// Scenes for headless visual captures (--scene <name>).
pub fn apply_scene(ui: &AppWindow, state: &Rc<AppState>, scene: &str) {
    let g = ui.global::<UIState>();
    match scene {
        "dark" => g.set_dark(true),
        "palette" | "search" | "search-notes" | "menu" | "dialog" | "settings" => {
            apply_scene_overlay(ui, state, scene)
        }
        "slash" => {
            // base: focus the first paragraph (overlay opens the menu)
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(core_page_id(state.open_page.get()))
                    .iter()
                    .find(|b| b.kind == crate::core::BlockKind::Paragraph && !b.text.is_empty())
                    .map(|b| (b.id.0 as i32, b.text.len() as i32, b.text.clone()))
            };
            if let Some((id, len, text)) = target {
                g.set_editing_text(text.into());
                g.set_pending_caret(len);
                g.set_editing_id(id);
            }
        }
        "block-menu" => {}
        "link" => apply_scene_overlay(ui, state, "link-dlg"),
        "dark-slash" => {
            g.set_dark(true);
            apply_scene(ui, state, "slash");
        }
        "dark-find" => {
            g.set_dark(true);
            apply_scene(ui, state, "find");
        }
        "dark-marks" => {
            g.set_dark(true);
            apply_scene(ui, state, "marks");
        }
        "dark-link" => {
            g.set_dark(true);
            apply_scene_overlay(ui, state, "link-dlg");
        }
        "dark-block-menu" => {
            g.set_dark(true);
            apply_scene_overlay(ui, state, "block-menu");
        }
        "dark-block-colors" => {
            g.set_dark(true);
            apply_scene(ui, state, "block-colors");
        }
        "dark-title-edit" => {
            g.set_dark(true);
            apply_scene(ui, state, "title-edit");
        }
        "title-edit" => {
            g.set_page_title("Renaming in place…".into());
            g.set_title_editing(true);
        }
        "recovered" => {
            // mirror the real string from main.rs (the library lives in the
            // per-user profile since ADR-0020; a sweep shot that invents copy
            // gets judged as if users read it)
            g.set_db_notice(
                "the database was damaged — restored from a backup (C:\\Users\\you\\AppData\\Roaming\\Quire\\quire.db.bak1).".into(),
            );
        }

        "page-block" => {
            // seed an embedded child page: an empty line after the first
            // block turns into a Page block (PageCreated + ref + kind in one
            // batch), then the child gets a real title for the shot
            let first = {
                let d = state.doc.borrow();
                d.page_blocks(core_page_id(state.open_page.get()))
                    .first()
                    .map(|b| b.id.0 as i32)
            };
            let Some(first) = first else { return };
            let changes = state.exec_on_open_page(Command::InsertBlockAfter {
                id: BlockId(first as u64),
                kind: crate::core::BlockKind::Paragraph,
                text: String::new(),
            });
            if let Some(nid) = changes.as_deref().and_then(find_inserted_id) {
                if let Some(child) = state.create_page_block(nid) {
                    state.rename_page(child, "Project Atlas");
                }
            }
        }

        "link-block" => {
            // seed a Link-to-page block pointing at an existing page: the
            // unowned reference renders like a page block with a link icon
            let first = {
                let d = state.doc.borrow();
                d.page_blocks(core_page_id(state.open_page.get()))
                    .first()
                    .map(|b| b.id.0 as i32)
            };
            let target = state
                .workspace
                .borrow()
                .dfs_order()
                .into_iter()
                .find(|id| *id != state.open_page.get());
            let (Some(first), Some(target)) = (first, target) else {
                return;
            };
            let changes = state.exec_on_open_page(Command::InsertBlockAfter {
                id: BlockId(first as u64),
                kind: crate::core::BlockKind::Paragraph,
                text: String::new(),
            });
            if let Some(nid) = changes.as_deref().and_then(find_inserted_id) {
                state.create_page_link_block(nid, target);
            }
        }

        "rename" => {
            g.set_renaming_id(108);
        }

        "empty" => open(&g, state, 113),
        "nest" => {
            // indent the second bullet under the first (visual test)
            let page = core_page_id(state.open_page.get());
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .filter(|b| b.kind == crate::core::BlockKind::Bullet)
                    .nth(1)
                    .map(|b| b.id)
            };
            if let Some(id) = target {
                let _ = state.exec_on_open_page(Command::IndentList { id });
            }
        }
        "find" => {
            g.set_find_open(true);
            g.set_find_term("the".into());
            state.find_start("the");
            g.set_find_label(state.find_label().into());
            // jump to the first hit like the live path does, so the scene
            // shows a selection, not a bare "0 / N"
            if let Some((bid, start, end)) = state.find_step(true) {
                apply_find_hit(&g, state, bid, start, end);
                g.set_find_label(state.find_label().into());
            }
        }
        "marks" => {
            // seed inline marks on the first paragraph (visual test only,
            // applied directly like an editor toggle would). Offsets are
            // derived from the words themselves — the A4 sweep caught the
            // old hardcoded bytes drifting off the words they demoed.
            let page = core_page_id(state.open_page.get());
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .find(|b| b.kind == crate::core::BlockKind::Paragraph && !b.text.is_empty())
                    .map(|b| (b.id, b.text.clone()))
            };
            if let Some((id, text)) = target {
                let span = |needle: &str, kind: crate::core::MarkKind| crate::core::Mark {
                    start: text.find(needle).unwrap_or(0),
                    end: text
                        .find(needle)
                        .map(|s| s + needle.len())
                        .unwrap_or(0),
                    kind,
                    url: if kind == crate::core::MarkKind::Link {
                        "https://example.com".into()
                    } else {
                        String::new()
                    },
                };
                let marks = vec![
                    span("home", crate::core::MarkKind::Bold),
                    span("for thinking.", crate::core::MarkKind::Bold),
                    span("collects notes", crate::core::MarkKind::Strike),
                    span("Quire itself", crate::core::MarkKind::Code),
                    span("plans, and references", crate::core::MarkKind::Link),
                ];
                state
                    .doc
                    .borrow_mut()
                    .apply(&[crate::core::Change::BlockMarksSet { id, marks }]);
                state.reproject_blocks();
            }
        }
        "edit" => {
            // focus the first paragraph of the landing page
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(core_page_id(state.open_page.get()))
                    .iter()
                    .find(|b| b.kind == crate::core::BlockKind::Paragraph && !b.text.is_empty())
                    .map(|b| (b.id.0 as i32, b.text.len() as i32, b.text.clone()))
            };
            if let Some((id, len, text)) = target {
                g.set_editing_text(text.into());
                g.set_pending_caret(len);
                g.set_editing_id(id);
            }
        }
        "block-colors" => {
            // color a few blocks + add a callout near the top (visual test
            // only; applied directly like the menu would)
            let page = core_page_id(state.open_page.get());
            let mut colors: Vec<(BlockId, crate::core::ColorKind, crate::core::ColorKind)> =
                {
                    let d = state.doc.borrow();
                    let blocks = d.page_blocks(page);
                    let mut v = Vec::new();
                    if let Some(h) = blocks
                        .iter()
                        .find(|b| matches!(b.kind, crate::core::BlockKind::Heading1 | crate::core::BlockKind::Heading2 | crate::core::BlockKind::Heading3))
                    {
                        v.push((h.id, crate::core::ColorKind::Blue, crate::core::ColorKind::Default));
                    }
                    if let Some(p) = blocks
                        .iter()
                        .find(|b| b.kind == crate::core::BlockKind::Paragraph && !b.text.is_empty())
                    {
                        v.push((p.id, crate::core::ColorKind::Red, crate::core::ColorKind::Default));
                    }
                    if let Some(q) = blocks.iter().find(|b| b.kind == crate::core::BlockKind::Quote) {
                        v.push((q.id, crate::core::ColorKind::Green, crate::core::ColorKind::Yellow));
                    }
                    if let Some(t) = blocks.iter().find(|b| b.kind == crate::core::BlockKind::Todo) {
                        v.push((t.id, crate::core::ColorKind::Default, crate::core::ColorKind::Blue));
                    }
                    v
                };
            let doc_changes: Vec<crate::core::Change> = colors
                .drain(..)
                .map(|(id, c, bg)| crate::core::Change::BlockColorSet { id, color: c, background: bg })
                .collect();
            state.doc.borrow_mut().apply(&doc_changes);
            // the callout goes right under the first paragraph so it is on
            // screen in a headless capture
            let anchor = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .find(|b| b.kind == crate::core::BlockKind::Paragraph && !b.text.is_empty())
                    .map(|b| b.id)
                    .or_else(|| d.page_blocks(page).last().map(|b| b.id))
                    .unwrap_or(BlockId(1))
            };
            let _ = state.exec_on_open_page(Command::InsertBlockAfter {
                id: anchor,
                kind: crate::core::BlockKind::Callout,
                text: "Callouts stand out — an emoji, a tinted box, and text.".into(),
            });
            state.reproject_blocks();
        }
        _ => {}
    }
}

/// Popup-opening half of a scene: called AFTER the first render pass so the
/// delegate-owned popups (slash, block menu) see a false -> true transition
/// and their `changed` handlers fire.
pub fn apply_scene_overlay(ui: &AppWindow, state: &Rc<AppState>, scene: &str) {
    let g = ui.global::<UIState>();
    match scene {
        "palette" => g.set_palette_open(true),
        // The Navigate rows sit below the palette's visible fold when the query
        // is empty, so a scene that filters to them is the only way to see them.
        "palette-nav" => {
            state.set_query("go");
            g.set_palette_query("go".into());
            g.set_palette_open(true);
        }
        "search" => {
            g.set_search_open(true);
        }
        "search-notes" => {
            g.set_search_query("notes".into());
            // headless: blocking query (the GUI path polls async instead)
            state.set_search_rows_sync("notes");
            g.set_search_open(true);
        }
        "menu" => {
            state.fill_menu(106);
            g.set_menu_node_id(106);
            let row_y = state.sidebar_row_y(106) as f32;
            g.set_menu_y(TREE_TOP_PX + row_y - 4.0);
            g.set_menu_x(240.0);
            g.set_menu_open(true);
        }
        "page-move-to" => {
            // the sidebar menu's Move-to submenu for page 106: Back, Top
            // level, then every legal target (106's own subtree is skipped)
            state.fill_page_menu_move_to(106);
            g.set_menu_node_id(106);
            let row_y = state.sidebar_row_y(106) as f32;
            g.set_menu_y(TREE_TOP_PX + row_y - 4.0);
            g.set_menu_x(240.0);
            g.set_menu_open(true);
        }
        "dialog" => {
            let (_title, message) = state.delete_dialog_text(105);
            g.set_dialog_title("Delete page?".into());
            g.set_dialog_message(message.into());
            g.set_dialog_open(true);
        }
        "settings" => g.set_settings_open(true),
        "dark-slash" => apply_scene_overlay(ui, state, "slash"),
        "slash" => {
            state.open_slash("");
            g.set_slash_focus(0);
            // headless estimate: landing-page paragraph position
            g.set_slash_x(340.0);
            g.set_slash_y(260.0);
            g.set_slash_open(true);
        }
        "block-menu" => {
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(core_page_id(state.open_page.get()))
                    .get(4)
                    .map(|b| b.id.0 as i32)
            };
            if let Some(id) = target {
                state.fill_block_menu();
                g.set_block_menu_x(320.0);
                g.set_block_menu_y(300.0);
                g.set_block_menu_open_id(id);
            }
        }
        // the "+"-handle insert menu, anchored below an empty new line
        "plus" => {
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(core_page_id(state.open_page.get()))
                    .get(4)
                    .map(|b| b.id.0 as i32)
            };
            if let Some(id) = target {
                g.invoke_block_plus(id, 280.0, 340.0);
            }
        }
        "move-to" => {
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(core_page_id(state.open_page.get()))
                    .get(4)
                    .map(|b| b.id.0 as i32)
            };
            if let Some(id) = target {
                state.fill_block_menu_move_to();
                g.set_block_menu_x(320.0);
                g.set_block_menu_y(120.0);
                g.set_block_menu_open_id(id);
            }
        }
        "text-color" => {
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(core_page_id(state.open_page.get()))
                    .get(4)
                    .map(|b| b.id.0 as i32)
            };
            if let Some(id) = target {
                state.fill_block_menu_colors(id, false);
                g.set_block_menu_x(320.0);
                g.set_block_menu_y(200.0);
                g.set_block_menu_open_id(id);
            }
        }
        "bg-color" => {
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(core_page_id(state.open_page.get()))
                    .get(4)
                    .map(|b| b.id.0 as i32)
            };
            if let Some(id) = target {
                state.fill_block_menu_colors(id, true);
                g.set_block_menu_x(320.0);
                g.set_block_menu_y(120.0);
                g.set_block_menu_open_id(id);
            }
        }
        "link-dlg" => {
            g.set_link_start(0);
            g.set_link_end(20);
            g.set_link_url("https://github.com/slint-ui/slint".into());
            g.set_link_open(true);
        }

        _ => {}
    }
}
