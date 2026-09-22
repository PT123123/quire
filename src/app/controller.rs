// UI callbacks land here. The controller owns *when* something happens;
// state.rs owns *what* the data looks like; workspace.rs owns the tree
// itself. Nothing here touches DB or disk.
//
// 1.18 global-handle pattern: `ui.global::<UIState>()` borrows the handle, so
// 'static callbacks capture a Weak and upgrade() it at fire time.

use crate::app::state::{
    core_page_id, kind_from_int, palette_action, AppState, PaletteAction, PAGE_GETTING_STARTED,
    ROW_NEW_PAGE,
};
use crate::core::{BlockId, Change, Command, Lang};
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
            // the delegate reports a row index; the command counts model
            // positions, and a folded subtree makes those differ
            let Some(target) = s.drop_index_for_row(index, below) else { return false };
            s.can_move_block_to(id, target)
        });
    }
    {
        let s = state.clone();
        ui.global::<UIState>().on_block_dropped(move |data, index, below| {
            let Some(id) = block_drag_id(&data) else { return };
            let Some(target) = s.drop_index_for_row(index, below) else { return };
            let _ = s.exec_on_open_page(Command::MoveBlockTo {
                id: BlockId(id as u64),
                index: target,
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
            if action == crate::app::state::MENU_MOVE_TO
                || action == crate::app::state::MENU_PAGE_STYLE
            {
                if action == crate::app::state::MENU_MOVE_TO {
                    s.fill_page_menu_move_to(id);
                } else {
                    s.fill_page_menu_style(id);
                }
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
                a if (crate::app::state::PAGE_FONT_BASE
                    ..crate::app::state::PAGE_FONT_BASE
                        + crate::core::PageFont::ALL.len() as i32)
                    .contains(&a) =>
                {
                    if id > 0 {
                        let font = crate::core::PageFont::ALL[(a
                            - crate::app::state::PAGE_FONT_BASE)
                            as usize];
                        s.set_page_font(id, font);
                    }
                }
                crate::app::state::MENU_PAGE_FULL_WIDTH => {
                    if id > 0 {
                        s.toggle_page_full_width(id);
                    }
                }
                crate::app::state::MENU_PAGE_SMALL_TEXT => {
                    if id > 0 {
                        s.toggle_page_small_text(id);
                    }
                }
                // The emoji grid replaces this menu rather than nesting under
                // it: it is the taller of the two, and it is anchored at the
                // same place, so keeping the menu open would put one popup
                // behind the other.
                crate::app::state::MENU_PAGE_ICON => {
                    if id > 0 {
                        s.fill_icon_picker(id);
                        // the grid is 8 cells wide and 13 rows tall; a short
                        // window clips it, and the popup scrolls the rest
                        g.set_icon_picker_x(
                            g.get_menu_x()
                                .min((g.get_window_w() - 256.0).max(8.0)),
                        );
                        g.set_icon_picker_y(
                            g.get_menu_y()
                                .max(48.0)
                                .min((g.get_window_h() - 160.0).max(48.0)),
                        );
                        g.set_icon_picker_open(true);
                    }
                }
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
        ui.global::<UIState>().on_icon_picked(move |glyph| {
            let g = gw.upgrade().unwrap();
            let id = g.get_icon_picker_page();
            // the pick closes the grid either way; "" is its "None" cell, and
            // picking it on a page with no icon is a write of the empty string
            // the storage layer already treats as "unset"
            g.set_icon_picker_open(false);
            g.set_icon_picker_page(-1);
            if id > 0 {
                s.set_page_icon(id, glyph.as_str());
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
            // One id -> action mapping in state.rs, matched here with no
            // wildcard arm: this dispatch used to be `match id` over literals
            // plus constants, and a constant whose import was missing turns
            // into a catch-all *binding* rather than a pattern — that shadowed
            // every command with id >= 9 for one round (`aaa3763`). An enum
            // path cannot bind, and a variant with no arm here no longer
            // compiles.
            match palette_action(id) {
                PaletteAction::NewPage => {
                    let new_id = s.create_page(None);
                    open(&g, &s, new_id);
                    g.set_renaming_id(new_id);
                }
                PaletteAction::SearchPages => {
                    g.set_search_open(true);
                }
                PaletteAction::ToggleSidebar => g.set_sidebar_open(!g.get_sidebar_open()),
                PaletteAction::ToggleTheme => g.set_dark(!g.get_dark()),
                PaletteAction::Settings => g.set_settings_open(true),
                PaletteAction::RenamePage => {
                    g.set_renaming_id(g.get_sidebar_selected_id());
                }
                PaletteAction::DuplicatePage => {
                    let cur = s.open_page.get();
                    if let Some(new_id) = s.duplicate_page(cur) {
                        open(&g, &s, new_id);
                    }
                }
                PaletteAction::DeletePage => {
                    let cur = s.open_page.get();
                    if s.workspace.borrow().contains(cur) {
                        let (_t, message) = s.delete_dialog_text(cur);
                        g.set_dialog_title("Delete page?".into());
                        g.set_dialog_message(message.into());
                        g.set_dialog_open(true);
                    }
                }
                PaletteAction::ExportMarkdown => export_current_page(&g, &s),
                PaletteAction::ImportMarkdown => import_markdown_dialog(&g, &s),
                PaletteAction::CopyMarkdown => copy_current_page_markdown(&g, &s),
                PaletteAction::NavigateBack => navigate(&g, &s, false),
                PaletteAction::NavigateForward => navigate(&g, &s, true),
                PaletteAction::OpenPage(page) => open(&g, &s, page),
                PaletteAction::None => {}
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

    // toggle block (SPEC §三十七): the chevron hides/shows the subtree. The
    // rows themselves come and go, so this takes the full reproject that
    // `exec_on_open_page` does — unlike the todo patch above. The pending
    // typing commits first: the block losing its row must not eat it.
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_toggle_fold(move |id| {
            if id <= 0 {
                return;
            }
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            s.exec_on_open_page(Command::ToggleFold {
                id: BlockId(id as u64),
            });
        });
    }

    // picture block (SPEC §三十七 批次 A): the row asks for its raster by
    // attachments id. Both callbacks run during layout of a realized row, so
    // a page with five hundred pictures decodes the handful on screen.
    {
        let s = state.clone();
        ui.global::<UIState>().on_image_for(move |id| s.image_for(id));
    }
    {
        let s = state.clone();
        ui.global::<UIState>().on_image_aspect(move |id| s.image_aspect(id));
    }

    // file block (SPEC §三十七 批次 A): the size label is a lookup, the two
    // buttons hand the stored bytes to the system.
    {
        let s = state.clone();
        ui.global::<UIState>()
            .on_attachment_size(move |id| s.attachment_size(id).into());
    }
    // math block (SPEC §三十七 批次 C): no state to consult — the source is in
    // the row, and the conversion is a function of it.
    ui.global::<UIState>()
        .on_math_render(move |src| crate::core::math::to_unicode(&src).into());
    // embed card (SPEC §三十七 批次 C): the same shape as the math row — the
    // address is the block's own text, and both lines the card shows are
    // functions of it. Nothing here looks anything up.
    ui.global::<UIState>()
        .on_embed_label(move |url| crate::core::embed::describe(&url).into());
    ui.global::<UIState>()
        .on_embed_url(move |url| crate::core::embed::with_scheme(&url).into());
    // code highlight (SPEC §三十七 批次 C): the same shape as the two above —
    // one pure function of what the row already holds, no state to consult. The
    // row asks for each colour of its block separately; the lexing is done once
    // per call by Slint's own cache of a pure callback.
    ui.global::<UIState>().on_code_layer(move |text, lang, frame, advance, kind| {
        crate::core::highlight::layer(
            &text,
            Lang::try_from_str(&lang).unwrap_or(Lang::Plain),
            frame,
            advance,
            crate::core::highlight::Kind::from_int(kind),
        )
        .into()
    });
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_attachment_opened(move |id| {
            let g = gw.upgrade().unwrap();
            attachment_action(&g, &s, id, false);
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_attachment_saved(move |id| {
            let g = gw.upgrade().unwrap();
            attachment_action(&g, &s, id, true);
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

    // toc (SPEC §三十七 批次 C): a contents line names a block on this page,
    // so the click walks the same path a `quire://block/` link walks — minus
    // the url parse and the page open, since the list is built from the page
    // that is already showing.
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_toc_jump(move |id| {
            let g = gw.upgrade().unwrap();
            if id <= 0 {
                return;
            }
            let len = {
                let d = s.doc.borrow();
                d.block(BlockId(id as u64))
                    .map(|b| b.text.len() as i32)
                    .unwrap_or(0)
            };
            flush_pending_edit(&g, &s);
            // -1 first: recreate the delegate so the input takes over
            g.set_editing_id(-1);
            focus_block(&g, &s, id, len);
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
            let clip = crate::platform::read_clipboard();
            let Some(text) = clip else {
                // No text on the clipboard at all, so the other thing worth
                // pasting gets its turn: a screenshot. When a copy carries
                // both, the words win — a rich app's text must not be turned
                // into a picture of itself.
                return match crate::platform::read_clipboard_image() {
                    None => false,
                    Some(png) => {
                        if s.paste_image(id, &png) {
                            g.set_editing_id(-1);
                            true
                        } else {
                            g.set_db_notice("That clipboard picture could not be stored.".into());
                            false
                        }
                    }
                };
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
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_reclaim_attachments(move || {
            let g = gw.upgrade().unwrap();
            let text = match s.reclaim_attachments() {
                Ok(msg) => msg,
                Err(e) => format!("Reclaim failed: {e}"),
            };
            g.set_db_notice(text.into());
        });
    }

    // debounce the typing commit: each keystroke restarts the timer; the
    // Timer must outlive this scope (leaked, like the bench timers). The table
    // cells arm the same timer — they skip everything above it, because a cell
    // is not a line of prose: no markdown shortcut converts in a grid, and the
    // slash menu has no room to open in one.
    let t: &'static slint::Timer = Box::leak(Box::new(slint::Timer::default()));
    {
        let gw = gw.clone();
        let s = state.clone();
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
                        // the same fit-the-window rule the "+" menu uses: the
                        // list is as tall as its rows, so a hardcoded reserve
                        // would push the last entries off the bottom as soon
                        // as the menu grew another kind
                        let menu_h = g.get_slash_items().row_count() as f32 * 32.0 + 8.0;
                        let y = (40.0 + (row_y as f32) - scroll + (row_h as f32) + 4.0)
                            .clamp(48.0, (g.get_window_h() - menu_h - 8.0).max(48.0));
                        g.set_slash_x(edge + (content_x as f32));
                        g.set_slash_y(y);
                        g.set_slash_open(true);
                    } else {
                        g.set_slash_open(false);
                    }
                }
                debounce_arm(t, &gw, &s);
            });
    }

    // ---- table grid (SPEC §三十七 批次 B) ----
    // The cells are child blocks the projection hides, so everything here
    // goes through `editing-id`: the row model carries them, Rust reads the
    // focused cell back out of the UI state, and the grid's own text never
    // touches `editing-changed` (a cell has no line shortcuts to run).
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_table_cell_move(move |cell, delta| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            // Tab off the last cell appends a row first; Shift-Tab off the
            // front stays put, which is what `None` means
            if let Some(next) = s.table_step(cell, delta) {
                focus_block(&g, &s, next, i32::MAX);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_cell_changed(move || {
            debounce_arm(t, &gw, &s);
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_table_row_added(move |table| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            s.table_add_row(table, g.get_editing_id());
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_table_column_added(move |table| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            s.table_add_column(table, g.get_editing_id());
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_table_row_removed(move |table| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            let cell = g.get_editing_id();
            let at = s.table_cell_index(cell);
            if s.table_delete_row(table, cell) {
                table_refocus(&g, &s, table, at);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_table_column_removed(move |table| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            let cell = g.get_editing_id();
            let at = s.table_cell_index(cell);
            if s.table_delete_column(table, cell) {
                table_refocus(&g, &s, table, at);
            }
        });
    }

    // ---- columns layout (SPEC §三十七 批次 B) ----
    // The boxes and their blocks are child blocks the projection hides, so —
    // exactly as for a grid — everything here goes through `editing-id`, and
    // the layout's row never sees `editing-changed` for them.
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_column_item_move(move |item, delta| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            // either end of the layout stops: a box is left by clicking out,
            // not by tabbing past the edge
            if let Some(next) = s.column_step(item, delta) {
                focus_block(&g, &s, next, i32::MAX);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_column_added(move |layout| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            if s.column_add(layout) {
                columns_refocus(&g, &s);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_column_removed(move |layout| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            // the last box's blocks move into the one before it, so the
            // focused block survives its own box going away
            if s.column_remove(layout) {
                columns_refocus(&g, &s);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_column_fill(move |column| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            // the empty box just got a line, and the caret should be on it —
            // otherwise the click looks like it did nothing
            if let Some(id) = s.column_fill(column) {
                focus_block(&g, &s, id, i32::MAX);
            }
        });
    }

    // ---- database views (SPEC §三十九) ----
    //
    // A database is the one block whose rows are not blocks, so none of these
    // callbacks goes through `editing-id`: a cell is named by
    // (block, record, property) and the write paths (`AppState::db_*`) parse,
    // plan and re-read the window. Two rules hold across the whole section:
    //
    // * **the "one live TextInput" discipline still holds** — a cell's editor
    //   exists only while `editing-id < 0`, and opening a cell closes the
    //   block editor (and the other way round, in `focus_block`'s caller),
    //   so a page can never have two inputs whatever order the two were set;
    // * **a structural change re-fills that one row.** A window read mutates
    //   the rows model in place (that is why scrolling a 10 000-row table is
    //   one query), but the *shape* the delegate lays out — row count, window
    //   start, columns, tabs — rides on the `BlockRow`, which only
    //   `db_fill_row` rebuilds. `db_refill_row` is the one-row version of the
    //   page projection, and it is what keeps the block as tall as the table.
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>()
            .on_db_cell_activated(move |block, record, property| {
                let g = gw.upgrade().unwrap();
                // a block editor's pending keystrokes land first: closing it
                // below would otherwise drop everything typed in the last
                // debounce window
                flush_pending_edit(&g, &s);
                // the *stored* value, not the painted one: a number with a
                // format paints (`50%`) differently from what an editor must
                // accept (`0.5`)
                let text = s
                    .db_cell_text(block, record as i64, property)
                    .unwrap_or_default();
                g.set_editing_id(-1);
                g.set_editing_text(text.into());
                g.set_db_editing_block(block);
                g.set_db_editing_record(record);
                g.set_db_editing_property(property);
                // a click is also the way out of an open option list
                g.set_db_picking_record(-1);
                g.set_db_picking_property(-1);
            });
    }
    {
        // the debounced commit while typing: the same 300 ms timer the prose
        // rows use, with the database's own ids read back instead of
        // `editing-id`.
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_cell_changed(move || {
            db_debounce_arm(t, &gw, &s);
        });
    }
    {
        // the live cell is going away (Enter / Escape / Tab): commit **now**.
        // Leaving it to the debounce would lose the text typed in the last
        // 300 ms, because closing resets the two ids that commit reads back.
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_cell_closed(move || {
            let g = gw.upgrade().unwrap();
            db_commit_cell(&g, &s);
            g.set_db_editing_block(-1);
            g.set_db_editing_record(-1);
            g.set_db_editing_property(-1);
            // the next editor is created with the focus and gets its text from
            // Rust; an empty mirror here is what keeps a stale value from
            // flashing up first
            g.set_editing_text("".into());
            g.set_db_picking_record(-1);
            g.set_db_picking_property(-1);
        });
    }
    {
        let s = state.clone();
        ui.global::<UIState>()
            .on_db_cell_toggled(move |block, record, property, checked| {
                // the inverse of what the cell shows, which is the stored flag
                // (the painted word is ADR-0065's `Yes`/`No`); the rows model is
                // re-read in place, so no `db_refill_row` is needed — a toggle
                // changes no row's existence
                s.db_toggle_checkbox(block, record as i64, property, checked);
            });
    }
    {
        let s = state.clone();
        ui.global::<UIState>()
            .on_db_option_picked(move |block, record, property, option, current| {
                // `option` is the stored id (ADR-0061 stores ids, not labels);
                // picking the one a cell already holds clears it
                s.db_pick_option(block, record as i64, property, &option, &current);
            });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_row_added(move |block| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            // the key is asked of the store (a `MAX(ord)` on an index): the app
            // deliberately does not hold the rows to find the last one (ADR-0067)
            let ord = s.db_next_row_ord(block);
            if s.db_add_record(block, ord).is_some() {
                db_refill_row(&s, block);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_row_removed(move |block, record| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            if s.db_delete_record(block, record as i64) {
                // the row menu's delete is ADR-0063's plan ([values, record,
                // page?]) and one Ctrl+Z, so nothing here touches the history
                db_refill_row(&s, block);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>()
            .on_db_column_resized(move |block, property, permille| {
                // applied on release, never per move: the write is one
                // view-document edit and one undo step (ADR-0064's `widths`)
                if s.db_set_column_width(block, property, permille) {
                    db_refill_row(&s, block);
                }
            });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>()
            .on_db_columns_opened(move |block, x, y| {
                let g = gw.upgrade().unwrap();
                db_push_columns(&g, &s, block);
                // `absolute-position` is window-relative, so the anchor needs
                // no conversion — only a clamp: a database near the bottom of
                // the window must not open its popup off screen.
                let rows = g.get_db_columns_rows().row_count() as f32;
                let popup_h = rows * 30.0 + 8.0;
                let y = y.clamp(48.0, (g.get_window_h() - popup_h - 8.0).max(48.0));
                let x = x.clamp(8.0, (g.get_window_w() - 232.0).max(8.0));
                g.set_db_columns_x(x);
                g.set_db_columns_y(y);
                g.set_db_columns_block(block);
                g.set_db_columns_open(true);
            });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_column_toggled(move |block, property| {
            let g = gw.upgrade().unwrap();
            if s.db_toggle_column(block, property) {
                // the view shows different columns now: the row is re-filled
                // (header and cells) and the popup's own rows are rebuilt in
                // place, because it stays open across the click
                db_refill_row(&s, block);
                db_push_columns(&g, &s, block);
            }
        });
    }
    {
        let gw = gw.clone();
        ui.global::<UIState>().on_db_columns_closed(move || {
            let g = gw.upgrade().unwrap();
            g.set_db_columns_open(false);
        });
    }
    {
        let s = state.clone();
        ui.global::<UIState>().on_db_view_picked(move |block, view| {
            // session state, not a column (ADR-0073): a different view has
            // different columns, so the row is re-read and re-filled
            if s.db_pick_view(block, view) {
                db_refill_row(&s, block);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_viewport(move |block, top_in_view| {
            let g = gw.upgrade().unwrap();
            // the editor's height is the viewport every database on the page is
            // windowed against; the delegate reports it once per resize
            s.editor_viewport_h.set(g.get_editor_viewport_h());
            // a scroll costs the arithmetic and, only when the window moved past
            // its overscan, one query (`db_watch` is that gate). The rows model
            // is mutated in place; the block row is re-filled only when the
            // window really moved, because that is when `db-row-start` changed.
            if s.db_watch(block, top_in_view) {
                db_refill_row(&s, block);
            }
        });
    }

    // ---- database rules (SPEC §三十九 「操作」, D4) ----
    //
    // The same two rules the cell callbacks carry, with one addition: the
    // panel stays open across its own edits, so every accepted edit re-pushes
    // the panel's rows (`db_push_filter`) the way the columns popup re-pushes
    // its toggles. An edit that Rust refuses (a value of the wrong shape, a
    // tree the panel cannot represent) writes nothing, and the panel shows
    // the rules that are still in force.
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_filter_opened(move |block, x, y| {
            let g = gw.upgrade().unwrap();
            // the panel's value boxes are inputs: the block editor must not
            // also be up
            flush_pending_edit(&g, &s);
            g.set_editing_id(-1);
            db_push_filter(&g, &s, block);
            g.set_db_filter_panel(0);
            g.set_db_filter_block(block);
            // anchored under the button (window coordinates, the columns
            // popup's convention) and clamped to the rules state's height —
            // the tallest the panel gets
            let rows = state.db_filter_panel(block).1.len() as f32;
            let popup_h = 40.0 + rows * 76.0 + 36.0 + 8.0;
            let y = y.clamp(48.0, (g.get_window_h() - popup_h - 8.0).max(48.0));
            let x = x.clamp(8.0, (g.get_window_w() - 348.0 - 8.0).max(8.0));
            g.set_db_filter_x(x);
            g.set_db_filter_y(y);
            g.set_db_filter_open(true);
        });
    }
    {
        let gw = gw.clone();
        ui.global::<UIState>().on_db_filter_closed(move || {
            let g = gw.upgrade().unwrap();
            g.set_db_filter_open(false);
        });
    }
    {
        let gw = gw.clone();
        ui.global::<UIState>().on_db_filter_panel_set(move |panel| {
            let g = gw.upgrade().unwrap();
            g.set_db_filter_panel(panel);
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_filter_match_set(move |block, any| {
            let g = gw.upgrade().unwrap();
            if s.db_filter_set_match(block, any) {
                db_refill_row(&s, block);
                db_push_filter(&g, &s, block);
            }
        });
    }
    {
        let gw = gw.clone();
        ui.global::<UIState>().on_db_filter_add_rule(move |_block| {
            let g = gw.upgrade().unwrap();
            // the chooser's list is already pushed; the panel just turns a page
            g.set_db_filter_panel(1);
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_filter_column_picked(move |block, property| {
            let g = gw.upgrade().unwrap();
            if s.db_filter_add_clause(block, property) {
                db_refill_row(&s, block);
                db_push_filter(&g, &s, block);
                g.set_db_filter_panel(0);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_filter_row_removed(move |block, index| {
            let g = gw.upgrade().unwrap();
            if s.db_filter_remove_clause(block, index as usize) {
                db_refill_row(&s, block);
                db_push_filter(&g, &s, block);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_filter_op_menu(move |block, index| {
            let g = gw.upgrade().unwrap();
            db_push_filter_ops(&g, &s, block, index as usize);
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_filter_op_picked(move |block, index, op| {
            let g = gw.upgrade().unwrap();
            // switching the comparison resets the value (the old shape is not
            // the new one's), so the row comes back unfilled
            if s.db_filter_set_op(block, index as usize, op as usize) {
                db_refill_row(&s, block);
                db_push_filter(&g, &s, block);
                g.set_db_filter_panel(0);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_filter_text_set(move |block, index, text| {
            let g = gw.upgrade().unwrap();
            // a refused value (not a number, not a stored date) writes nothing
            // and the panel keeps the text for the user to fix
            if s.db_filter_set_text(block, index as usize, &text) {
                db_refill_row(&s, block);
                db_push_filter(&g, &s, block);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_filter_flag_set(move |block, index, checked| {
            let g = gw.upgrade().unwrap();
            if s.db_filter_set_flag(block, index as usize, checked) {
                db_refill_row(&s, block);
                db_push_filter(&g, &s, block);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_filter_option_menu(move |block, index| {
            let g = gw.upgrade().unwrap();
            db_push_filter_options(&g, &s, block, index as usize);
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_filter_option_picked(move |block, index, option| {
            let g = gw.upgrade().unwrap();
            // `is`: picking the held option clears the rule; `any of`: the
            // pick toggles membership — either way the panel goes home
            if s.db_filter_toggle_option(block, index as usize, &option) {
                db_refill_row(&s, block);
                db_push_filter(&g, &s, block);
                g.set_db_filter_panel(0);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_filter_not_toggled(move |block, index| {
            let g = gw.upgrade().unwrap();
            if s.db_filter_toggle_not(block, index as usize) {
                db_refill_row(&s, block);
                db_push_filter(&g, &s, block);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_filter_cleared(move |block| {
            let g = gw.upgrade().unwrap();
            if s.db_filter_clear(block) {
                db_refill_row(&s, block);
                db_push_filter(&g, &s, block);
            }
        });
    }
    {
        let s = state.clone();
        ui.global::<UIState>().on_db_sort_cycled(move |block, property| {
            // the header cycles none → asc → desc → none; a kind with no
            // order refuses in Rust and the header stays as it was
            if s.db_sort_cycle(block, property) {
                db_refill_row(&s, block);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_group_opened(move |block, x, y| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            db_push_group(&g, &s, block);
            g.set_db_group_block(block);
            let rows = state.db_group_choices(block).len() as f32 + 1.0;
            let popup_h = rows * 30.0 + 8.0;
            let y = y.clamp(48.0, (g.get_window_h() - popup_h - 8.0).max(48.0));
            let x = x.clamp(8.0, (g.get_window_w() - 224.0 - 8.0).max(8.0));
            g.set_db_group_x(x);
            g.set_db_group_y(y);
            g.set_db_group_open(true);
        });
    }
    {
        let gw = gw.clone();
        ui.global::<UIState>().on_db_group_closed(move || {
            let g = gw.upgrade().unwrap();
            g.set_db_group_open(false);
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_group_picked(move |block, property| {
            let g = gw.upgrade().unwrap();
            if s.db_group_pick(block, property) {
                db_refill_row(&s, block);
            }
            // the pick is the whole interaction: the picker closes whether it
            // changed anything or not (a refusal said its own word)
            g.set_db_group_open(false);
        });
    }

    // ---- database view family (SPEC §三十九 「视图」, D5) ----
    //
    // The switcher's `+` (one callback per picked layout, `ViewLayout::ALL`'s
    // order), the card click (open the record's page, minting it when the
    // record is bare — ADR-0063's lazy page finally getting its trigger), the
    // calendar's ‹ ›, the gallery's reported shape, and the form's three
    // callbacks. The same two rules the D4 bindings carry: every accepted edit
    // re-fills the one block row (the layout's own payload rides on it), and a
    // write Rust refused changed nothing on screen.
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_view_added(move |block, layout| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            // `db_add_view` also switches to the new view — creating and not
            // looking would be half a gesture — so one refill carries the
            // switch's own re-read
            if s.db_add_view(block, layout) {
                db_refill_row(&s, block);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_open_record(move |block, record| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            if let Some(page) = s.db_open_record(block, record as i64) {
                // the record may have just gained its page: the block row's
                // own shape (board cards' Open targets) follows on the refill,
                // and the navigation below is the sidebar's usual dance
                db_refill_row(&s, block);
                open(&g, &s, page);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_cal_month(move |block, delta| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            if s.db_cal_shift(block, delta) {
                db_refill_row(&s, block);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_gallery_shaped(move |block, per_row| {
            let g = gw.upgrade().unwrap();
            // the grid was resized: the card slice is rows × per_row, so a new
            // shape is a re-read (and a no-op when the clamp keeps the number)
            if s.db_gallery_set_per_row(block, per_row) {
                db_refill_row(&s, block);
            }
        });
    }
    {
        let s = state.clone();
        ui.global::<UIState>().on_db_form_text(move |block, property, text| {
            // the draft only: no refill (the live input must not be rebuilt
            // under the keystroke), no SQL (nothing exists until Submit)
            s.db_form_set_text(block, property, &text);
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_form_submitted(move |block| {
            let g = gw.upgrade().unwrap();
            flush_pending_edit(&g, &s);
            // a refused submit said its own word (`db-notice`); a successful
            // one cleared the draft, and the refill is what blanks the fields
            if s.db_form_submit(block) {
                db_refill_row(&s, block);
            }
        });
    }
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_db_form_cleared(move |block| {
            let g = gw.upgrade().unwrap();
            s.db_form_clear(block);
            db_refill_row(&s, block);
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
            // An attachment is not a text style: it needs a file before the
            // block can exist, so the menu closes and the picker decides. "/"
            // means "this line becomes the picture" (the line is empty once
            // the /filter is stripped); the "+" menu means "a picture appears
            // here", so the line it was opened on keeps whatever it holds.
            if matches!(
                kind,
                crate::core::BlockKind::Image | crate::core::BlockKind::File
            ) {
                g.set_slash_open(false);
                g.set_slash_insert(false);
                g.set_editing_id(-1);
                if insert_mode {
                    pick_attachment(&g, &s, id, false, kind);
                } else {
                    let _ = s.exec_on_open_page(Command::ReplaceText {
                        id: BlockId(id as u64),
                        text: cleaned.clone(),
                    });
                    pick_attachment(&g, &s, id, true, kind);
                }
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
            // SPEC §三十九: "Table view" turns the line into a database — the
            // block, the entity, its title column and its first view in one
            // batch (`make_database`; the line's own words go with the same
            // undo). `SetBlockType` refuses a Database kind by design: the
            // entity is not the plan layer's to allocate. A database draws no
            // line input of its own, so the caret leaves the row entirely.
            if kind == crate::core::BlockKind::Database {
                s.make_database(id);
                g.set_slash_open(false);
                g.set_slash_insert(false);
                g.set_editing_text("".into());
                g.set_editing_id(-1);
                return;
            }
            let _ = s.exec_on_open_page(Command::ReplaceText {
                id: BlockId(id as u64),
                text: cleaned.clone(),
            });
            let changes = s.exec_on_open_page(Command::SetBlockType {
                id: BlockId(id as u64),
                kind,
            });
            g.set_slash_open(false);
            g.set_slash_insert(false);
            // a grid and a layout draw no row input of their own, so the caret
            // cannot stay on the row that just became one: it goes into the
            // first cell or the first line, which is where the words moved
            let target = match kind {
                crate::core::BlockKind::Table => {
                    changes.as_deref().and_then(find_inserted_cell_id)
                }
                crate::core::BlockKind::Columns => {
                    changes.as_deref().and_then(find_inserted_line_id)
                }
                _ => None,
            };
            match target {
                Some(target) => focus_block(&g, &s, target, i32::MAX),
                None => {
                    g.set_editing_text(cleaned.clone().into());
                    g.set_pending_caret(cleaned.len() as i32);
                    g.set_editing_id(id);
                }
            }
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
                s.fill_block_menu(id);
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
                s.fill_block_menu(id);
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
            if action == 13 {
                s.fill_block_menu_image_width(id);
                return;
            }
            if action == 14 {
                s.fill_block_menu_code_lang(id);
                return;
            }
            // color picks stay open (Notion-style live preview); everything
            // else closes first
            let color_pick = (AppState::COLOR_TEXT_BASE..AppState::COLOR_BG_BASE + 100)
                .contains(&action);
            let width_pick = (AppState::IMAGE_WIDTH_BASE
                ..AppState::IMAGE_WIDTH_BASE + 200)
                .contains(&action);
            let lang_pick = (AppState::CODE_LANG_BASE
                ..AppState::CODE_LANG_BASE + Lang::ALL.len() as i32)
                .contains(&action);
            if !(color_pick || width_pick || lang_pick) {
                g.set_block_menu_open_id(-1);
            }
            if width_pick {
                s.set_image_width(id, action - AppState::IMAGE_WIDTH_BASE);
                // refill so the current-tier check follows the pick
                s.fill_block_menu_image_width(id);
                return;
            }
            if lang_pick {
                let lang = Lang::ALL[(action - AppState::CODE_LANG_BASE) as usize];
                s.set_code_lang(id, lang);
                s.fill_block_menu_code_lang(id);
                return;
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
                    let new_kind = kind_from_int(a - 100);
                    // SPEC §三十九: Turn-into reaches the same one-batch path
                    // the "/" and "+" menus do. `SetBlockType` refuses a
                    // Database kind — the entity, its title column and its
                    // first view are not the plan layer's to allocate — so
                    // `make_database` is the only door, and the line's words
                    // leave with it (one Ctrl+Z restores the line whole).
                    if new_kind == crate::core::BlockKind::Database {
                        s.make_database(id);
                        if g.get_editing_id() == id {
                            g.set_editing_text("".into());
                            g.set_editing_id(-1);
                        }
                        return;
                    }
                    // A picture is not a style: it takes a file first, and the
                    // block only converts once the user has chosen one.
                    if matches!(
                        new_kind,
                        crate::core::BlockKind::Image | crate::core::BlockKind::File
                    ) {
                        pick_attachment(&g, &s, id, true, new_kind);
                        return;
                    }
                    let _ = s.exec_on_open_page(Command::SetBlockType {
                        id: BlockId(id as u64),
                        kind: new_kind,
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
                    4 => Some(crate::core::MarkKind::Math),
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
            // Windows shell open; cfg-gated so other targets simply no-op.
            // Only an address a browser understands gets this far: a link's
            // target is data that arrives from a file, and `start` will run a
            // path as readily as it opens a url — so a local path, a share or
            // an unknown protocol is the document's own business, not the
            // command line's. (SPEC §三十七 批次 C, ADR-0040; the embed card
            // is what made this worth fixing, since it puts a button on it.)
            if !crate::core::embed::is_openable(&url) {
                return;
            }
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
            // Enter from the title of an empty page lands the caret in the
            // body — which on such a page means making it exist first.
            if s.blocks.row_count() == 0 {
                if let Some(id) = s.start_page() {
                    focus_block(&g, &s, id, 0);
                }
            }
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_empty_page_started(move || {
            let g = gw.upgrade().unwrap();
            if let Some(id) = s.start_page() {
                focus_block(&g, &s, id, 0);
            }
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

/// Restart the 300 ms typing-commit timer. A prose row arms it after its
/// markdown and slash work; a table cell arms it alone, since a cell is a grid
/// slot rather than a line.
fn debounce_arm(
    t: &'static slint::Timer,
    gw: &slint::Weak<UIState<'static>>,
    s: &Rc<AppState>,
) {
    let gw = gw.clone();
    let s = s.clone();
    t.start(
        slint::TimerMode::SingleShot,
        std::time::Duration::from_millis(300),
        move || {
            if let Some(g) = gw.upgrade() {
                flush_pending_edit(&g, &s);
            }
        },
    );
}

fn db_debounce_arm(
    t: &'static slint::Timer,
    gw: &slint::Weak<UIState<'static>>,
    s: &Rc<AppState>,
) {
    let gw = gw.clone();
    let s = s.clone();
    t.start(
        slint::TimerMode::SingleShot,
        std::time::Duration::from_millis(300),
        move || {
            if let Some(g) = gw.upgrade() {
                let record = g.get_db_editing_record();
                let property = g.get_db_editing_property();
                // not a live cell, or the block editor took the keyboard back:
                // neither state belongs to this commit
                if record < 0 || property < 0 || g.get_editing_id() >= 0 {
                    return;
                }
                let block = g.get_db_editing_block();
                if block < 0 {
                    return;
                }
                let text = g.get_editing_text().to_string();
                let _ = s.db_set_cell_text(block, record as i64, property, &text);
            }
        },
    );
}

/// Commit the live database cell synchronously — the `db-cell-closed` path
/// (Enter / Escape / Tab). The debounce cannot be trusted here: closing resets
/// `db-editing-record`/`-property`, and the debounced commit reads those back,
/// so a text typed in the last 300 ms would be silently dropped. A refused
/// parse closes anyway: the editor going away is what repaints the stored
/// value, which is the honest answer to "that did not take".
fn db_commit_cell(g: &UIState<'_>, state: &Rc<AppState>) {
    let record = g.get_db_editing_record();
    let property = g.get_db_editing_property();
    if record < 0 || property < 0 || g.get_db_editing_block() < 0 {
        return;
    }
    let block = g.get_db_editing_block();
    let text = g.get_editing_text().to_string();
    let _ = state.db_set_cell_text(block, record as i64, property, &text);
}

/// Re-fill one projected block row's §三十九 fields in place — the one-row
/// version of the page projection's `db_fill_row` pass. A window read mutates
/// the rows model in place, but the *shape* the delegate lays out (row count,
/// window start, columns, tabs) rides on the `BlockRow`, and every structural
/// database change (row added or removed, column toggled or resized, view
/// picked) moves that shape. Walking the model here keeps the cost one row
/// instead of one page — the same reasoning the projection's targeted patch
/// (`set_row_data` on the todo toggle) already follows.
fn db_refill_row(state: &Rc<AppState>, block: i32) {
    let mut i = 0;
    while let Some(mut row) = state.blocks.row_data(i) {
        if row.id == block {
            state.db_fill_row(&mut row);
            state.blocks.set_row_data(i, row);
            return;
        }
        i += 1;
    }
}

/// (Re)build the columns popup's rows from the view's own document. Two
/// callers, one truth: opening the popup, and the moment after a toggle — the
/// popup stays open across the click, so its rows are stale the instant the
/// toggle lands, and the same builder is what un-stales them.
fn db_push_columns(g: &UIState<'_>, state: &Rc<AppState>, block: i32) {
    let rows: Vec<crate::DbColumnToggle> = state
        .db_column_toggles(block)
        .into_iter()
        .map(|t| crate::DbColumnToggle {
            property: t.property,
            name: t.name.into(),
            kind: t.kind.into(),
            visible: t.visible,
            locked: t.locked,
        })
        .collect();
    g.set_db_columns_rows(Rc::new(slint::VecModel::from(rows)).into());
}

/// Push the filter panel's whole state (D4): the rule rows, the root's
/// flavour, and the column chooser's list. Called on open and after every
/// accepted edit, so the panel a user is looking at is always the document's
/// current rules — the model is derived, never a second copy of the filter.
fn db_push_filter(g: &UIState<'_>, state: &Rc<AppState>, block: i32) {
    let (any, rows) = state.db_filter_panel(block);
    g.set_db_filter_match_any(any);
    let rows: Vec<crate::DbFilterRow> = rows
        .into_iter()
        .map(|r| crate::DbFilterRow {
            property: r.property,
            name: r.name.into(),
            kind: r.kind,
            op: r.op,
            op_name: r.op_name.into(),
            value: r.value.into(),
            has_value: r.has_value,
            invert: r.invert,
        })
        .collect();
    g.set_db_filter_rows(Rc::new(slint::VecModel::from(rows)).into());
    // The chooser lists every property of the database (the columns popup's
    // own model shape, so a name and a type word are already what it draws).
    let choices: Vec<crate::DbColumnToggle> = state
        .db_column_toggles(block)
        .into_iter()
        .map(|t| crate::DbColumnToggle {
            property: t.property,
            name: t.name.into(),
            kind: t.kind.into(),
            visible: t.visible,
            locked: t.locked,
        })
        .collect();
    g.set_db_filter_columns(Rc::new(slint::VecModel::from(choices)).into());
}

/// Push the comparison chooser for one clause: the comparisons its column's
/// kind has — the same list the parser accepts, so nothing the menu offers can
/// be refused later. The panel switches to state 2.
fn db_push_filter_ops(g: &UIState<'_>, state: &Rc<AppState>, block: i32, index: usize) {
    let rows = state.db_filter_panel(block).1;
    let Some(row) = rows.get(index) else {
        return;
    };
    let ops: Vec<crate::DbFilterOp> = state
        .db_ops_for_kind(row.kind)
        .into_iter()
        .map(|(op, name)| crate::DbFilterOp {
            op,
            name: name.into(),
        })
        .collect();
    g.set_db_filter_ops(Rc::new(slint::VecModel::from(ops)).into());
    g.set_db_filter_edit_row(index as i32);
    g.set_db_filter_panel(2);
}

/// Push the value chooser for one clause: the column's options (an id and the
/// name the config gives it). The panel switches to state 3.
fn db_push_filter_options(g: &UIState<'_>, state: &Rc<AppState>, block: i32, index: usize) {
    let rows = state.db_filter_panel(block).1;
    let Some(row) = rows.get(index) else {
        return;
    };
    let options: Vec<crate::DbOption> = state
        .db_property_options(row.property)
        .into_iter()
        .map(|(id, name, color)| crate::DbOption {
            id: id.into(),
            name: name.into(),
            color: color.into(),
        })
        .collect();
    g.set_db_filter_options(Rc::new(slint::VecModel::from(options)).into());
    g.set_db_filter_edit_row(index as i32);
    g.set_db_filter_panel(3);
}

/// Push the group picker's rows (D4): the option-bounded columns, plus the
/// active one so a row can mark itself.
fn db_push_group(g: &UIState<'_>, state: &Rc<AppState>, block: i32) {
    let rows: Vec<crate::DbColumnToggle> = state
        .db_group_choices(block)
        .into_iter()
        .map(|t| crate::DbColumnToggle {
            property: t.property,
            name: t.name.into(),
            kind: t.kind.into(),
            visible: t.visible,
            locked: t.locked,
        })
        .collect();
    g.set_db_group_rows(Rc::new(slint::VecModel::from(rows)).into());
    g.set_db_group_current(state.db_group_current(block));
}

/// Where the caret belongs after a row or column delete: `at` was the focused
/// cell's row-major slot, so the same slot of the smaller grid is the cell
/// nearest to it. `None` means the caret was never in this grid — the delete
/// took a row from under someone else, so it keeps its own focus.
fn table_refocus(g: &UIState<'_>, s: &Rc<AppState>, table: i32, at: Option<usize>) {
    match at.and_then(|at| s.table_cell_at(table, at)) {
        Some(cell) => focus_block(g, s, cell, i32::MAX),
        None => refresh_focused_text(g, s),
    }
}

/// Adding or removing a box rebuilds the layout's row, and the item's input
/// is created with the focus — so it would come back with the caret at the
/// start of the line. Hand it back where the reader left it.
fn columns_refocus(g: &UIState<'_>, s: &Rc<AppState>) {
    let id = g.get_editing_id();
    if id > 0 && s.is_column_item(id) {
        focus_block(g, s, id, i32::MAX);
    }
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
        if state.is_table_cell(editing) {
            // a cell has no row of its own: its text rides on the table's row
            state.sync_cell_text(editing, &text);
            return;
        }
        if state.is_column_item(editing) {
            // ... and so does a block inside a columns box
            state.sync_column_text(editing, &text);
            return;
        }
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
            font: crate::core::PageFont::default(),
            full_width: false,
            small_text: false,
            icon: String::new(),
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

/// One picture round-trip: the native file dialog, then store the bytes beside
/// the database and put the block in the page. `convert` picks between the two
/// image commands — this block becomes the picture, or a picture block appears
/// below it.
///
/// The dialog blocks, deliberately, exactly like the import/export buttons do:
/// it runs on the click that opened it, before the editor takes another event,
/// so there is no half-inserted state to reconcile. A cancelled dialog changes
/// nothing; a failed import says why in the notice bar.
fn pick_attachment(
    g: &UIState<'_>,
    s: &Rc<AppState>,
    anchor: i32,
    convert: bool,
    kind: crate::core::BlockKind,
) {
    let pictures = kind == crate::core::BlockKind::Image;
    let picker = rfd::FileDialog::new().set_title(if pictures {
        "Insert a picture"
    } else {
        "Attach a file"
    });
    // The picture picker constrains the choice, because the decoder reads four
    // formats — png, jpeg, bmp, gif — and nothing else. The file one must not:
    // naming any type is the whole point of the kind.
    let picker = if pictures {
        picker.add_filter("Pictures", &["png", "jpg", "jpeg", "bmp", "gif"])
    } else {
        picker.add_filter("All files", &["*"])
    };
    let Some(path) = picker.pick_file() else {
        return;
    };
    let id = s.claim_attachment_id();
    let imported = if pictures {
        s.store.import_file(id, &path)
    } else {
        s.store.import_any_file(id, &path)
    };
    let placed = match imported {
        Ok(att) => {
            if convert {
                s.set_block_attachment(anchor, att, kind)
            } else {
                s.insert_attachment(anchor, att, kind)
            }
        }
        Err(e) => {
            g.set_db_notice(e.to_string().into());
            return;
        }
    };
    if !placed {
        g.set_db_notice(
            if pictures {
                "That picture did not fit into the page."
            } else {
                "That file did not fit into the page."
            }
            .into(),
        );
    }
}

/// Hand an attachment's stored bytes to whatever the system opens this type
/// with, or copy them out to a path the user picks. Both read the `attachments`
/// row rather than the block, so the name and the extension agree (SPEC
/// §三十七 批次 A).
fn attachment_action(g: &UIState<'_>, s: &Rc<AppState>, id: i32, save: bool) {
    let Some(att) = s.attachment_row(id) else {
        g.set_db_notice("That attachment belongs to another library.".into());
        return;
    };
    let label = att.name.clone();
    let path = s.store.stored_path(&att);
    if save {
        let Some(target) = rfd::FileDialog::new()
            .set_title("Save attachment as")
            .set_file_name(s.store.save_name(&att))
            .save_file()
        else {
            return;
        };
        let notice = match s.store.export_to(&att, &target) {
            Ok(()) => format!("Saved {label}."),
            Err(e) => e.to_string(),
        };
        g.set_db_notice(notice.into());
        return;
    }
    if !path.is_file() {
        g.set_db_notice(format!("{label}: the stored file is gone.").into());
        return;
    }
    if !crate::platform::open_with_default(&path) {
        g.set_db_notice(format!("Nothing here opens {label}.").into());
    }
}

/// Export the open page's blocks to a .md file via the native save dialog.
/// "Copy Page as Markdown" (palette): the page through the exporter onto
/// the clipboard. The FFI write path is mandatory here — a markdown page
/// routinely carries CJK, which clip.exe's OEM stdin garbles (ADR-0025).
fn copy_current_page_markdown(g: &UIState<'_>, s: &Rc<AppState>) {
    let page = s.open_page.get();
    let md = {
        let d = s.doc.borrow();
        crate::services::export_service::export_page_with(
            d.page_blocks(core_page_id(page)),
            // ADR-0065: the caller that can read the database pre-renders
            // the table (current view → GFM); a clipboard copy is that caller.
            &|id| s.db_markdown_table(id.as_u64() as i32),
        )
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
        crate::services::export_service::export_page_with(
            d.page_blocks(core_page_id(page)),
            // ADR-0065: the caller that can read the database pre-renders
            // the table (current view → GFM); the .md export is that caller.
            &|id| state.db_markdown_table(id.as_u64() as i32),
        )
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
        font: crate::core::PageFont::default(),
        full_width: false,
        small_text: false,
        icon: String::new(),
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

/// Seed the database-table scene (SPEC §三十九): one table on the demo page
/// with its title column and three more kinds the inline editors carry —
/// number, checkbox, date — and five rows of fixed values, because a shot has
/// to look the same tomorrow (the date is a literal, exactly as the `@date`
/// scene's is). Every step goes through the write paths the UI uses
/// (`make_database` → `db_add_column` → `db_add_record` → `db_set_cell_text`),
/// so the scene is a fixture of the *pipeline* and not of the model: if the
/// command layer cannot build a database, the scene must not pretend one.
///
/// A select/status column is deliberately absent: D3 has no option editor
/// (D5's), so a fresh select column has an empty option list and the shot
/// would be about "No options yet" rather than about the table.
fn seed_database_table(state: &Rc<AppState>) -> Option<i32> {
    use crate::core::database::{CellValue, PropertyKind};
    use crate::core::BlockKind;

    // the line the database grows from: an empty paragraph after the first
    // line of prose (the same anchor every near-top seed uses)
    let after = {
        let d = state.doc.borrow();
        d.page_blocks(core_page_id(state.open_page.get()))
            .iter()
            .find(|b| b.kind == BlockKind::Paragraph && !b.text.is_empty())
            .map(|b| b.id.0 as i32)
    }?;
    let id = state
        .exec_on_open_page(Command::InsertBlockAfter {
            id: BlockId(after as u64),
            kind: BlockKind::Paragraph,
            text: String::new(),
        })
        .as_deref()
        .and_then(find_inserted_id)?;
    if !state.make_database(id) {
        return None;
    }
    let points = state.db_add_column(id, "Points", PropertyKind::Number);
    let done = state.db_add_column(id, "Done", PropertyKind::Checkbox);
    let due = state.db_add_column(id, "Due", PropertyKind::Date);
    let (Some(points), Some(done), Some(due)) = (points, done, due) else {
        return None;
    };
    // The title column's id: the one `MakeDatabase` created first and locked
    // (ADR-0061), which the popup lists as non-toggleable.
    let Some(title) = state.db_column_toggles(id).iter().find(|t| t.locked).map(|t| t.property)
    else {
        return None;
    };
    // (name, points, done, due) — the values are literals, not `now()`/`today()`:
    // a sweep that runs tomorrow must photograph the same table.
    let rows: [(&str, &str, bool, &str); 5] = [
        ("Spec the window", "31", true, "2026-09-22"),
        ("Sort in SQL", "10", false, "2026-09-23"),
        ("Write the switcher", "7", true, "2026-09-25"),
        ("Measure a scroll", "39", false, "2026-10-01"),
        ("Light the menus", "3", true, "2026-10-04"),
    ];
    for (name, pts, checked, day) in rows {
        let Some(record) = state.db_add_record(id, state.db_next_row_ord(id)) else {
            continue;
        };
        state.db_set_cell_text(id, record, title, name);
        state.db_set_cell_text(id, record, points, pts);
        state.db_set_cell_text(id, record, due, day);
        // the box carries a typed value, not a parse: the cell's own toggle
        // path is `Flag(!shown)`, and the scene uses the same door
        state.db_set_cell_value(id, record, done, CellValue::Flag(checked));
    }
    // the last write's window is current, but the block row's *shape* (row
    // count) is only rebuilt by `db_fill_row` — the same call every structural
    // change in the wire section follows its write with
    db_refill_row(state, id);
    Some(id)
}

/// Seed the database-filter scene (SPEC §三十九 「操作」, D4): the table seed plus
/// one number rule — Points > 5 — written through the **same helpers the filter
/// panel drives**, so the shot shows the real write path's result, not a
/// hand-built fixture: 3 of the 5 rows, the header chip reading "Filter 1",
/// and the rules persisted in the view's document (`{"and":[{…,"op":"gt",
/// "value":5}]}`). The values are literals: a sweep that runs tomorrow must
/// photograph the same table.
fn seed_database_filter(state: &Rc<AppState>) -> Option<i32> {
    let id = seed_database_table(state)?;
    let points = state
        .db_column_toggles(id)
        .into_iter()
        .find(|t| t.name == "Points")
        .map(|t| t.property)?;
    if !state.db_filter_add_clause(id, points) {
        return None;
    }
    // `gt` is FILTER_OPS[3] (contains, eq, ne, gt, …), and the value goes
    // through the kind's own validation — the same door a user's keystroke
    // would.
    if !state.db_filter_set_op(id, 0, 3) || !state.db_filter_set_text(id, 0, "5") {
        return None;
    }
    db_refill_row(state, id);
    Some(id)
}

/// Seed one of D5's view-family scenes: the table seed plus a second view of
/// the layout the scene names, added **through the same path the switcher's
/// `+` drives** (`db_add_view`, which also switches to the new view), plus the
/// layout's own rule through the same helpers the UI drives:
///
/// * board — groups by the seed's `Done` checkbox (D4's `db_group_pick`, the
///   same key the Group picker writes). A checkbox rather than a select: an
///   option editor is still D5's undone half, so a seeded select column would
///   have an empty option list and the shot would be about that.
/// * calendar — the month is **pinned** (`db_calendar_month_set`, 2026-09), not
///   defaulted to now: a sweep that runs tomorrow must photograph the same
///   grid. The records carry literal dates (`2026-09-22` …), so the month's
///   counts and the day peeks answer to the same query the view runs.
/// * timeline / gallery / list / form — the layout itself, no extra rule: the
///   date column resolves by the schema's own order (`Due` is the seed's only
///   date), the gallery reports its own shape on first layout, and the form is
///   its field list.
///
/// The values are literals throughout: a sweep that runs tomorrow must
/// photograph the same view.
fn seed_database_view(state: &Rc<AppState>, layout: crate::core::database::ViewLayout) -> Option<i32> {
    let id = seed_database_table(state)?;
    let index = crate::core::database::ViewLayout::ALL
        .iter()
        .position(|l| *l == layout)? as i32;
    if !state.db_add_view(id, index) {
        return None;
    }
    if layout == crate::core::database::ViewLayout::Board {
        let done = state
            .db_column_toggles(id)
            .into_iter()
            .find(|t| t.name == "Done")
            .map(|t| t.property)?;
        if !state.db_group_pick(id, done) {
            return None;
        }
    }
    if layout == crate::core::database::ViewLayout::Calendar {
        state.db_calendar_month_set(id, 2026, 9);
    }
    db_refill_row(state, id);
    Some(id)
}

/// The first cell a grid-building command created — the cell a table's caret
/// starts in. `apply` inserts cells in row-major order, so this is the
/// top-left one, which is where the converted line's words went.
fn find_inserted_cell_id(changes: &[Change]) -> Option<i32> {
    changes.iter().find_map(|c| match c {
        Change::BlockInserted(b) if b.kind == crate::core::BlockKind::TableCell => {
            Some(b.id.0 as i32)
        }
        _ => None,
    })
}

/// The first line a layout-building command created — where a columns caret
/// starts, since that is where the converted line's words moved. The command
/// inserts a box and its first line as a pair, so this is the first box.
fn find_inserted_line_id(changes: &[Change]) -> Option<i32> {
    changes.iter().find_map(|c| match c {
        Change::BlockInserted(b) if b.kind == crate::core::BlockKind::Paragraph => {
            Some(b.id.0 as i32)
        }
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
    if matches!(
        current,
        Some(BlockKind::Code) | Some(BlockKind::Divider) | Some(BlockKind::Math)
    ) {
        // a formula is source: `> x` inside `\begin{...}` is content, not a
        // quote marker
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
    } else if let Some(rest) = text.strip_prefix("$$ ") {
        conv(BlockKind::Math, rest)
    } else if text == "$$" {
        conv(BlockKind::Math, "")
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
    // a picture from the page you just left must not keep covering the one
    // you arrived at
    g.set_preview_attachment(0);
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
        "dark-code-hl" => {
            g.set_dark(true);
            apply_scene(ui, state, "code-hl");
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
        "dark-database-table" => {
            g.set_dark(true);
            apply_scene(ui, state, "database-table");
        }
        "dark-database-filter" => {
            g.set_dark(true);
            apply_scene(ui, state, "database-filter");
        }
        "dark-database-board" => {
            g.set_dark(true);
            apply_scene(ui, state, "database-board");
        }
        "dark-database-list" => {
            g.set_dark(true);
            apply_scene(ui, state, "database-list");
        }
        "dark-database-calendar" => {
            g.set_dark(true);
            apply_scene(ui, state, "database-calendar");
        }
        "dark-database-gallery" => {
            g.set_dark(true);
            apply_scene(ui, state, "database-gallery");
        }
        "dark-database-timeline" => {
            g.set_dark(true);
            apply_scene(ui, state, "database-timeline");
        }
        "dark-database-form" => {
            g.set_dark(true);
            apply_scene(ui, state, "database-form");
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

        // SPEC §三十九: the table view. A real database through the real write
        // paths — nothing here hand-builds a fixture the UI could not make.
        "database-table" => {
            seed_database_table(state);
        }
        // SPEC §三十九 「操作」(D4): the same table with one rule compiled into
        // SQL — Points > 5 keeps 3 of the 5 rows. The shot shows the red
        // line's visible half: the count and the rows answer to the rules,
        // the chip says "Filter 1", and nothing filtered happens anywhere but
        // in the statement.
        "database-filter" => {
            seed_database_filter(state);
        }

        // SPEC §三十九 「视图」(D5): the family. Each scene is the table seed
        // plus a second view of the named layout, added and switched to through
        // the switcher's own write path — so every shot is the real read
        // pipeline's answer (group counts, month counts, per-day peeks, card
        // slices), never a hand-built fixture.
        "database-board" => {
            seed_database_view(state, crate::core::database::ViewLayout::Board);
        }
        "database-list" => {
            seed_database_view(state, crate::core::database::ViewLayout::List);
        }
        "database-calendar" => {
            seed_database_view(state, crate::core::database::ViewLayout::Calendar);
        }
        "database-gallery" => {
            seed_database_view(state, crate::core::database::ViewLayout::Gallery);
        }
        "database-timeline" => {
            seed_database_view(state, crate::core::database::ViewLayout::Timeline);
        }
        "database-form" => {
            seed_database_view(state, crate::core::database::ViewLayout::Form);
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
        // SPEC §三十七: a collapsible section. `toggle` is the open shape
        // (chevron down, child visible) and `toggle-fold` closes it, so the
        // two captures must differ by exactly the child's row.
        "toggle" | "toggle-fold" => {
            let page = core_page_id(state.open_page.get());
            let bullets = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .filter(|b| b.kind == crate::core::BlockKind::Bullet)
                    .take(2)
                    .map(|b| b.id)
                    .collect::<Vec<_>>()
            };
            if bullets.len() == 2 {
                let (parent, child) = (bullets[0], bullets[1]);
                let _ = state.exec_on_open_page(Command::IndentList { id: child });
                let _ = state.exec_on_open_page(Command::SetBlockType {
                    id: parent,
                    kind: crate::core::BlockKind::Toggle,
                });
                if scene == "toggle-fold" {
                    let _ = state.exec_on_open_page(Command::ToggleFold { id: parent });
                }
            }
        }
        // SPEC §三十七 批次 A: a picture sitting in the page. `image-half` is
        // the same block at the 50 % tier, so the pair shows the width setting
        // really does drive the row geometry. The fixture is generated rather
        // than picked — the native file dialog is the one part of this feature
        // a headless scene cannot reach.
        "image" | "image-half" => {
            let page = core_page_id(state.open_page.get());
            let after = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .find(|b| b.kind == crate::core::BlockKind::Paragraph && !b.text.is_empty())
                    .map(|b| b.id.0 as i32)
            };
            let aid = state.claim_attachment_id();
            let (Some(after), Some(att)) = (after, state.store.create_fixture(aid, 640, 400))
            else {
                return;
            };
            if !state.insert_attachment(after, att, crate::core::BlockKind::Image)
                || scene != "image-half"
            {
                return;
            }
            let pic = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .find(|b| b.kind == crate::core::BlockKind::Image)
                    .map(|b| b.id.0 as i32)
            };
            if let Some(id) = pic {
                state.set_image_width(id, 50);
            }
        }
        // The other half of 批次 A: bytes the editor names but never opens. The
        // payload has to be a fixed length or the size label — the one number
        // this row paints — would differ between sweeps.
        "file" => {
            let page = core_page_id(state.open_page.get());
            let after = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .find(|b| b.kind == crate::core::BlockKind::Paragraph && !b.text.is_empty())
                    .map(|b| b.id.0 as i32)
            };
            let aid = state.claim_attachment_id();
            let payload = vec![0xA5u8; 1_842_000];
            let (Some(after), Some(att)) = (
                after,
                state
                    .store
                    .create_file_fixture(aid, "quarterly-report.pdf", &payload),
            ) else {
                return;
            };
            let _ = state.insert_attachment(after, att, crate::core::BlockKind::File);
        }
        // SPEC §三十七 批次 B: a grid sitting in the page. `table-edit` is the
        // same table with the caret in one cell, so the pair proves both the
        // resting shape and that a cell can take the editor at all — the
        // second is the one a static model could fake. `table-marks` is the
        // third question: a marked cell, which is drawn by a different row.
        "table" | "table-edit" | "table-marks" => {
            let page = core_page_id(state.open_page.get());
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .find(|b| b.kind == crate::core::BlockKind::Paragraph && !b.text.is_empty())
                    .map(|b| b.id)
            };
            let Some(line) = target else { return };
            // built the way the menu builds one: the line's own words move into
            // the top-left cell, which is where a scene like this starts
            let _ = state.exec_on_open_page(Command::SetBlockType {
                id: line,
                kind: crate::core::BlockKind::Table,
            });
            let cells = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .filter(|b| b.kind == crate::core::BlockKind::TableCell)
                    .map(|b| b.id)
                    .collect::<Vec<_>>()
            };
            for (cell, word) in cells.iter().zip(["North", "South", "East", "West", "Up"]) {
                let _ = state.exec_editor(Command::ReplaceText {
                    id: *cell,
                    text: (*word).into(),
                });
            }
            if scene == "table-marks" {
                // one cell whose words cannot fit its column: a marked cell is
                // drawn by the run row, and whether that row wraps is a
                // different question from whether a paragraph's does
                // (ADR-0041)
                let Some(cell) = cells.first().copied() else { return };
                let text = "Revenue grew because the team shipped the second half of the plan.";
                let _ = state.exec_editor(Command::ReplaceText {
                    id: cell,
                    text: text.into(),
                });
                let marks = vec![crate::core::Mark {
                    start: text.find("the team").expect("needle present"),
                    end: text.find("the team").expect("needle present") + "the team".len(),
                    kind: crate::core::MarkKind::Bold,
                    url: String::new(),
                }];
                state.doc.borrow_mut().apply(&[crate::core::Change::BlockMarksSet {
                    id: cell,
                    marks,
                }]);
            }
            state.reproject_blocks();
            if scene == "table-edit" {
                if let Some(cell) = cells.get(3) {
                    focus_block(&g, state, cell.as_u64() as i32, i32::MAX);
                }
            }
        }
        // SPEC §三十七 批次 B: a layout tiling the page. `columns-3` is the same
        // layout after the strip's "Add column", so the pair proves the boxes
        // really re-tile — a static model could show a count that never moved
        // the widths. `columns-marks` puts an over-long marked line in the
        // first box, which is the third place runs are drawn.
        "columns" | "columns-3" | "columns-marks" => {
            let page = core_page_id(state.open_page.get());
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .find(|b| b.kind == crate::core::BlockKind::Paragraph && !b.text.is_empty())
                    .map(|b| b.id)
            };
            let Some(line) = target else { return };
            // built the way the menu builds one, so the words move into the
            // first box and the scene shows a layout with content in it
            let _ = state.exec_on_open_page(Command::SetBlockType {
                id: line,
                kind: crate::core::BlockKind::Columns,
            });
            if scene == "columns-3" {
                let layout = {
                    let d = state.doc.borrow();
                    d.page_blocks(page)
                        .iter()
                        .find(|b| b.kind == crate::core::BlockKind::Columns)
                        .map(|b| b.id.0 as i32)
                };
                if let Some(id) = layout {
                    state.column_add(id);
                }
            }
            let boxes = {
                let d = state.doc.borrow();
                let mut boxes = d
                    .page_blocks(page)
                    .iter()
                    .filter(|b| b.kind == crate::core::BlockKind::Column)
                    .map(|b| (b.order, b.id))
                    .collect::<Vec<_>>();
                // left to right, whatever order the page slice happens to hold
                boxes.sort();
                boxes.into_iter().map(|(_, id)| id).collect::<Vec<_>>()
            };
            for (n, bx) in boxes.iter().enumerate() {
                // one word per box, so an uneven render is visible at a glance
                let first = {
                    let d = state.doc.borrow();
                    d.page_blocks(page)
                        .iter()
                        .filter(|b| b.parent == Some(*bx))
                        .min_by_key(|b| b.order)
                        .map(|b| b.id)
                };
                if let Some(id) = first {
                    let _ = state.exec_editor(Command::ReplaceText {
                        id,
                        text: format!("Column {}", n + 1).into(),
                    });
                }
            }
            if scene == "columns-marks" {
                // the third run consumer: a line inside a box, at half the page
                // width, with more words than fit (ADR-0041)
                let first_line = {
                    let d = state.doc.borrow();
                    d.page_blocks(page)
                        .iter()
                        .filter(|b| b.parent == boxes.first().copied())
                        .min_by_key(|b| b.order)
                        .map(|b| b.id)
                };
                let Some(id) = first_line else { return };
                let text = "Two boxes, and a sentence that has to break inside one of them.";
                let _ = state.exec_editor(Command::ReplaceText { id, text: text.into() });
                let marks = vec![crate::core::Mark {
                    start: text.find("has to break").expect("needle present"),
                    end: text.find("has to break").expect("needle present") + "has to break".len(),
                    kind: crate::core::MarkKind::Bold,
                    url: String::new(),
                }];
                state.doc.borrow_mut().apply(&[crate::core::Change::BlockMarksSet {
                    id,
                    marks,
                }]);
            }
            state.reproject_blocks();
        }
        // SPEC §三十七 批次 C: a formula, built the way the slash menu builds
        // one and then filled with source that touches every shape the
        // renderer knows — fraction, radical, relation, script, spacing escape.
        "math" => {
            let page = core_page_id(state.open_page.get());
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .find(|b| b.kind == crate::core::BlockKind::Paragraph && !b.text.is_empty())
                    .map(|b| b.id)
            };
            let Some(id) = target else { return };
            let _ = state.exec_on_open_page(Command::SetBlockType {
                id,
                kind: crate::core::BlockKind::Math,
            });
            let _ = state.exec_editor(Command::ReplaceText {
                id,
                text: r"\frac{a+b}{2} \leq \sqrt{ab} \ne 0 \quad \int_0^1 x^2 \,dx".into(),
            });
            state.reproject_blocks();
        }
        // …and the same renderer inside a sentence: the mark holds the source,
        // the run shows the glyphs, and the words around it are untouched.
        "math-inline" => {
            let page = core_page_id(state.open_page.get());
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .find(|b| b.kind == crate::core::BlockKind::Paragraph && !b.text.is_empty())
                    .map(|b| b.id)
            };
            let Some(id) = target else { return };
            let text = r"The rest energy is E = mc^2 for any mass.";
            let source = r"E = mc^2";
            let _ = state.exec_editor(Command::ReplaceText {
                id,
                text: text.into(),
            });
            if let Some(start) = text.find(source) {
                let marks = vec![crate::core::Mark {
                    start,
                    end: start + source.len(),
                    kind: crate::core::MarkKind::Math,
                    url: String::new(),
                }];
                state
                    .doc
                    .borrow_mut()
                    .apply(&[crate::core::Change::BlockMarksSet { id, marks }]);
            }
            state.reproject_blocks();
        }
        // SPEC §三十七 批次 C: the page's own contents. Built the way the
        // insert menu builds one — the intro line becomes the block, so the
        // list sits above every heading it lists, and the headings are the
        // fixture's own (three H2s and an H3, so the indent has two steps).
        "toc" => {
            let page = core_page_id(state.open_page.get());
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .find(|b| b.kind == crate::core::BlockKind::Paragraph && !b.text.is_empty())
                    .map(|b| b.id)
            };
            let Some(id) = target else { return };
            let _ = state.exec_on_open_page(Command::SetBlockType {
                id,
                kind: crate::core::BlockKind::Toc,
            });
            state.reproject_blocks();
        }
        // SPEC §三十七 批次 C: a link as a card. The intro line becomes the
        // block and carries an address with a `www.`, a query and a fragment, so
        // both lines the card paints are derived from something shaped like a
        // real link — and the headline is a provider the card knows.
        "embed" => {
            let page = core_page_id(state.open_page.get());
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .find(|b| b.kind == crate::core::BlockKind::Paragraph && !b.text.is_empty())
                    .map(|b| b.id)
            };
            let Some(id) = target else { return };
            let _ = state.exec_on_open_page(Command::SetBlockType {
                id,
                kind: crate::core::BlockKind::Embed,
            });
            let _ = state.exec_editor(Command::ReplaceText {
                id,
                text: "https://www.youtube.com/watch?v=dQw4w9WgXcQ#t=1s".into(),
            });
            state.reproject_blocks();
        }
        // …and the card before anything is typed into it. This is the state
        // every embed starts in, and the only one with no address to lay out,
        // so it is where a card that collapses to nothing would show up.
        "embed-empty" => {
            let page = core_page_id(state.open_page.get());
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .find(|b| b.kind == crate::core::BlockKind::Paragraph && !b.text.is_empty())
                    .map(|b| b.id)
            };
            let Some(id) = target else { return };
            let _ = state.exec_on_open_page(Command::SetBlockType {
                id,
                kind: crate::core::BlockKind::Embed,
            });
            let _ = state.exec_editor(Command::ReplaceText { id, text: "".into() });
            state.reproject_blocks();
        }
        // SPEC §三十七 批次 C: the same block the memory gate scrolls, painted.
        // The fixture carries a comment, a string, numbers, a name, CJK inside
        // the comment, and one line long enough to need a hard break, so every
        // rule the layer model has is on the screen at once.
        "code-hl" => {
            let page = core_page_id(state.open_page.get());
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .find(|b| b.kind == crate::core::BlockKind::Paragraph && !b.text.is_empty())
                    .map(|b| b.id)
            };
            let Some(id) = target else { return };
            let _ = state.exec_on_open_page(Command::SetBlockType {
                id,
                kind: crate::core::BlockKind::Code,
            });
            let _ = state.exec_editor(Command::ReplaceText {
                id,
                text: crate::app::state::bench_code_source(),
            });
            let _ = state.exec_on_open_page(Command::SetCodeLang {
                id,
                lang: crate::core::Lang::Rust,
            });
            state.reproject_blocks();
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
        // A4 D7's other two shapes: a match with no row of its own. Each arm
        // turns one paragraph into a container the way the menu does, so the
        // hit has to ride on the row that paints it (ADR-0028) — and each
        // searches a word that only exists inside that container, because a
        // swept scene that paints nothing either way would pass.
        "find-grid" | "find-cols" => {
            let grid = scene == "find-grid";
            apply_scene(ui, state, if grid { "table" } else { "columns" });
            let term = if grid { "North" } else { "Column" };
            g.set_find_open(true);
            g.set_find_term(term.into());
            state.find_start(term);
            g.set_find_label(state.find_label().into());
        }
        // The fourth surface: a callout keeps its text in a tinted box of its
        // own, offset past the emoji, so the runs path needs its frame to mark
        // a match there. The arm turns the page's first paragraph into one and
        // does not step — the *selected* hit belongs to the block being
        // edited, and an editing block answers with a real text selection
        // instead of a cell, which is the right answer and the wrong demo.
        "find-callout" => {
            let page = core_page_id(state.open_page.get());
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .find(|b| {
                        b.kind == crate::core::BlockKind::Paragraph && b.text.contains("Atlas")
                    })
                    .map(|b| b.id)
            };
            let Some(id) = target else { return };
            let _ = state.exec_on_open_page(Command::SetBlockType {
                id,
                kind: crate::core::BlockKind::Callout,
            });
            g.set_find_open(true);
            g.set_find_term("the".into());
            state.find_start("the");
            g.set_find_label(state.find_label().into());
        }
        // SPEC §三十八's page look, one switch per scene so a moved pixel says
        // which token moved it. Each arm goes through the same state methods
        // the Style menu does — including the write to `pages` — because a
        // scene that set the UI global by hand would prove nothing about the
        // column the look is stored in.
        "style-serif" | "style-mono" | "style-small" | "style-full" | "style-tight" => {
            let page = state.open_page.get();
            match scene {
                "style-serif" => state.set_page_font(page, crate::core::PageFont::Serif),
                "style-mono" => state.set_page_font(page, crate::core::PageFont::Mono),
                "style-small" => state.toggle_page_small_text(page),
                "style-full" => state.toggle_page_full_width(page),
                // both at once: the tier shrinks AND widens, so a line that
                // used to wrap must say where it stopped
                _ => {
                    state.set_page_font(page, crate::core::PageFont::Serif);
                    state.toggle_page_small_text(page);
                    state.toggle_page_full_width(page);
                }
            }
        }
        "dark-style-serif" => {
            g.set_dark(true);
            apply_scene(ui, state, "style-serif");
        }
        // The page's emoji in the two places it draws: above its own title and
        // in its sidebar row. Set through the state method the picker calls,
        // so the shot proves the write as well as the pixels. The sidebar slot
        // is the interesting one: it carries the emoji into the dark.
        "page-icon" => {
            state.set_page_icon(state.open_page.get(), "\u{1f680}");
        }
        "dark-page-icon" => {
            g.set_dark(true);
            apply_scene(ui, state, "page-icon");
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
        "marks-wrap" => {
            // A marked paragraph long enough to need several lines (ADR-0041):
            // `marks` is one line deep, and the wall this scene watches is the
            // one below the first — where the runs break, and whether the row
            // is tall enough to show all of it.
            let page = core_page_id(state.open_page.get());
            let text = "The landing page is a fixture and not a promise: every line of it is painted by the same delegate that paints a ten thousand line page, so what wraps here wraps there. A bold phrase in the middle of a sentence, an italic word, a code span like cargo build --release, and a link all arrive as runs, and a run is one cell of a layout that breaks between cells.";
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .find(|b| b.kind == crate::core::BlockKind::Paragraph && !b.text.is_empty())
                    .map(|b| b.id)
            };
            if let Some(id) = target {
                // a needle that misses must not silently mark the head of the
                // line — that is how `marks` once demoed the wrong words
                let span = |needle: &str, kind: crate::core::MarkKind| crate::core::Mark {
                    start: text.find(needle).expect("needle present"),
                    end: text.find(needle).expect("needle present") + needle.len(),
                    kind,
                    url: if kind == crate::core::MarkKind::Link {
                        "https://example.com".into()
                    } else {
                        String::new()
                    },
                };
                let marks = vec![
                    span("and not a promise", crate::core::MarkKind::Bold),
                    span("delegate", crate::core::MarkKind::Italic),
                    span("cargo build --release", crate::core::MarkKind::Code),
                    span("breaks between cells", crate::core::MarkKind::Link),
                ];
                state.doc.borrow_mut().apply(&[
                    crate::core::Change::BlockTextSet {
                        id,
                        text: text.into(),
                    },
                    crate::core::Change::BlockMarksSet { id, marks },
                ]);
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
                    // Orange and yellow text on their own tint used to be in
                    // this scene only as a menu dot, which is exactly the pair
                    // the palette's contrast slice moved -- a hex ratio needs a
                    // row of glyphs to be worth reading.
                    let taken: Vec<BlockId> = v.iter().map(|(id, ..)| *id).collect();
                    let mut rest = blocks
                        .iter()
                        .filter(|b| !b.text.is_empty() && !taken.contains(&b.id));
                    for c in [
                        crate::core::ColorKind::Orange,
                        crate::core::ColorKind::Yellow,
                    ] {
                        if let Some(b) = rest.by_ref().next() {
                            v.push((b.id, c, c));
                        }
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
        "page-style" => {
            // The Style submenu itself, with the checks on the row the page
            // stores — the only picture of the menu's other half.
            state.set_page_font(106, crate::core::PageFont::Serif);
            state.toggle_page_small_text(106);
            state.fill_page_menu_style(106);
            g.set_menu_node_id(106);
            let row_y = state.sidebar_row_y(106) as f32;
            g.set_menu_y(TREE_TOP_PX + row_y - 4.0);
            g.set_menu_x(240.0);
            g.set_menu_open(true);
        }
        "page-icon" => {
            // The page's emoji in the two places it draws: above its own
            // title and in its sidebar row. Set through the state method the
            // picker calls, so the shot proves the write as well as pixels.
            let page = state.open_page.get();
            state.set_page_icon(page, "\u{1f680}");
        }
        "icon-picker" => {
            let page = state.open_page.get();
            state.fill_icon_picker(page);
            g.set_icon_picker_x(240.);
            g.set_icon_picker_y(TREE_TOP_PX + state.sidebar_row_y(page) as f32);
            g.set_icon_picker_open(true);
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
                state.fill_block_menu(id);
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
