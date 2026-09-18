// UI callbacks land here. The controller owns *when* something happens;
// state.rs owns *what* the data looks like; workspace.rs owns the tree
// itself. Nothing here touches DB or disk.
//
// 1.18 global-handle pattern: `ui.global::<UIState>()` borrows the handle, so
// 'static callbacks capture a Weak and upgrade() it at fire time.

use crate::app::state::{
    core_page_id, AppState, CMD_EXPORT_PAGE, CMD_IMPORT_MD, CMD_PAGE_BASE, MENU_DELETE,
    MENU_DUPLICATE, MENU_FAVORITE, MENU_NEW_SUBPAGE, MENU_RENAME, PAGE_GETTING_STARTED,
    ROW_NEW_PAGE,
};
use crate::core::{BlockId, Change, Command};
use crate::{AppWindow, UIState};
use slint::{ComponentHandle, Global, Model};
use std::rc::Rc;

/// Y offset of the first tree row inside the window: title bar (40) +
/// workspace header (36) + search row (28) + settings row (28) + spacer (8).
pub const TREE_TOP_PX: f32 = 140.0;

pub fn bind(ui: &AppWindow, state: &Rc<AppState>) {
    let g = ui.global::<UIState>();
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

    // ---- shell ----
    {
        let gw = gw.clone();
        ui.global::<UIState>().on_toggle_sidebar(move || {
            let g = gw.upgrade().unwrap();
            g.set_sidebar_open(!g.get_sidebar_open());
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
            s.exec_on_open_page(Command::MoveBlock { id: BlockId(cur as u64), delta });
            refresh_focused_text(&g, &s);
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
            g.set_menu_open(false);
            g.set_menu_node_id(-1);
            match action {
                MENU_NEW_SUBPAGE => {
                    let new_id = s.create_page(Some(id));
                    open(&g, &s, new_id);
                    g.set_renaming_id(new_id);
                }
                MENU_RENAME => {
                    g.set_renaming_id(id);
                }
                MENU_DUPLICATE => {
                    if let Some(new_id) = s.duplicate_page(id) {
                        open(&g, &s, new_id);
                    }
                }
                MENU_FAVORITE => s.toggle_favorite(id),
                MENU_DELETE => {
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
                CMD_EXPORT_PAGE => export_current_page(&g, &s),
                CMD_IMPORT_MD => import_markdown_dialog(&g, &s),
                other if other >= CMD_PAGE_BASE => {
                    open(&g, &s, other - CMD_PAGE_BASE);
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
                Command::ToggleTodoChecked { id: BlockId(id as u64) },
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

    // debounce the typing commit: each keystroke restarts the timer; the
    // Timer must outlive this scope (leaked, like the bench timers)
    {
        let gw = gw.clone();
        let s = state.clone();
        let t: &'static slint::Timer = Box::leak(Box::new(slint::Timer::default()));
        ui.global::<UIState>().on_editing_changed(move |row_y, row_h, content_x| {
            let gw = gw.clone();
            let s = s.clone();
            if let Some(g) = gw.upgrade() {
                // "/" at block start opens the slash menu with the rest of
                // the line as the filter (SPEC §十五); anchored below the
                // editing block
                let text = g.get_editing_text().to_string();
                if let Some(filter) = text.strip_prefix('/') {
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
            let changes = s.exec_on_open_page(Command::MergeBackward { id: BlockId(cur as u64) });
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
                            blocks.get(t as usize).map(|b| {
                                (b.id.0 as i32, b.text.len() as i32, delta < 0)
                            })
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
            let next = (g.get_slash_focus() + delta).clamp(0, count - 1);
            g.set_slash_focus(next);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_slash_apply_selected(move || {
            let g = gw.upgrade().unwrap();
            let focus = g.get_slash_focus();
            let kind = match s.slash_selected_kind(focus) {
                Some(k) => k,
                None => return,
            };
            let text = g.get_editing_text().to_string();
            // strip "/filter" (the menu only triggers on the block's first
            // token; a space closes the menu before this point)
            let filter_len = g.get_slash_filter().len();
            let cleaned = if text.len() >= 1 + filter_len
                && text.is_char_boundary(1 + filter_len)
            {
                text[1 + filter_len..].to_string()
            } else {
                String::new()
            };
            let id = g.get_editing_id();
            if id <= 0 {
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
        ui.global::<UIState>().on_block_menu_opened(move |id, row_y| {
            let g = gw.upgrade().unwrap();
            eprintln!("debug: block-menu row_y={:.1}", (row_y as f32));
            s.fill_block_menu();
            g.set_block_menu_open_id(id);
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_block_menu_action(move |action| {
            let g = gw.upgrade().unwrap();
            let id = g.get_block_menu_open_id();
            g.set_block_menu_open_id(-1);
            if id <= 0 {
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
                    let _ = s.exec_on_open_page(Command::DuplicateBlock { id: BlockId(id as u64) });
                }
                4 => s.copy_block(id),
                5 => {
                    s.paste_below(id);
                }
                6 => {
                    let _ = s.exec_on_open_page(Command::DeleteBlock { id: BlockId(id as u64) });
                    if g.get_editing_id() == id {
                        g.set_editing_id(-1);
                    }
                }
                _ => {}
            }
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_block_plus(move |id| {
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
            }
        });
    }

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_toggle_mark(move |kind, start, end| {
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

    // ---- link dialog (M6) ----
    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_link_apply(move || {
            let g = gw.upgrade().unwrap();
            let url = g.get_link_url().to_string();
            g.set_link_open(false);
            let cur = g.get_editing_id();
            if cur <= 0 {
                return;
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
    let applied = state.exec_editor(Command::ReplaceText { id, text: text.clone() });
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

/// Export the open page's blocks to a .md file via the native save dialog.
fn export_current_page(g: &UIState<'_>, state: &Rc<AppState>) {
    let page = state.open_page.get();
    let title = state.workspace.borrow().title_of(page).unwrap_or("page").to_string();
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
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Markdown", &["md"])
        .pick_file()
    else {
        return;
    };
    let Ok(src) = std::fs::read_to_string(&path) else {
        eprintln!("quire: import failed: cannot read {}", path.display());
        return;
    };
    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Imported".into());

    let new_id = state.create_page(None);
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

/// Open a page, sync the top bar, and highlight it in the tree.
fn open(g: &UIState<'_>, state: &Rc<AppState>, id: i32) {
    if !state.workspace.borrow().contains(id) {
        return;
    }
    state.open_page(id);
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
pub fn bench_switch_next(ui: &AppWindow, state: &Rc<AppState>, bench_ids: &[i32], cursor: &mut usize) {
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

        "rename" => {
            g.set_renaming_id(108);
        }

        "empty" => open(&g, state, 113),
        "marks" => {
            // seed inline marks on the first paragraph (visual test only,
            // applied directly like an editor toggle would)
            let page = core_page_id(state.open_page.get());
            let target = {
                let d = state.doc.borrow();
                d.page_blocks(page)
                    .iter()
                    .find(|b| b.kind == crate::core::BlockKind::Paragraph && !b.text.is_empty())
                    .map(|b| (b.id, b.text.clone()))
            };
            if let Some((id, text)) = target {
                let end = text.len().min(26);
                let marks = vec![
                    crate::core::Mark { start: 0, end, kind: crate::core::MarkKind::Bold, url: String::new() },
                    crate::core::Mark { start: 2, end: 7, kind: crate::core::MarkKind::Italic, url: String::new() },
                    crate::core::Mark { start: 8, end: 12, kind: crate::core::MarkKind::Code, url: String::new() },
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
        "search" => {
            g.set_search_open(true);
        }
        "search-notes" => {
            g.set_search_query("notes".into());
            state.set_search_query("notes");
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
        "dialog" => {
            let (_title, message) = state.delete_dialog_text(105);
            g.set_dialog_title("Delete page?".into());
            g.set_dialog_message(message.into());
            g.set_dialog_open(true);
        }
        "settings" => g.set_settings_open(true),
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
                g.set_block_menu_open(true);
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
