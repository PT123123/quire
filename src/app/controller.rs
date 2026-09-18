// UI callbacks land here. The controller owns *when* something happens;
// state.rs owns *what* the data looks like; workspace.rs owns the tree
// itself. Nothing here touches DB or disk.
//
// 1.18 global-handle pattern: `ui.global::<UIState>()` borrows the handle, so
// 'static callbacks capture a Weak and upgrade() it at fire time.

use crate::app::state::{
    AppState, CMD_PAGE_BASE, MENU_DELETE, MENU_DUPLICATE, MENU_FAVORITE, MENU_NEW_SUBPAGE,
    MENU_RENAME, PAGE_GETTING_STARTED, ROW_NEW_PAGE,
};
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
    let (title, crumb) = state.open_page_info(state.open_page.get());
    g.set_page_title(title.into());
    g.set_page_breadcrumb(crumb.into());
    g.set_renderer_name(renderer_name().into());
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
        ui.global::<UIState>().on_toggle_theme(move || {
            let g = gw.upgrade().unwrap();
            g.set_dark(!g.get_dark());
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
            let mut rows: Vec<_> = s.blocks.iter().collect();
            for r in rows.iter_mut() {
                if r.id == id {
                    r.checked = !r.checked;
                }
            }
            s.blocks.set_vec(rows);
        });
    }
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
        "rename" => {
            g.set_renaming_id(108);
        }
        "settings" => g.set_settings_open(true),
        "dialog" => {
            let (_title, message) = state.delete_dialog_text(105);
            g.set_dialog_title("Delete page?".into());
            g.set_dialog_message(message.into());
            g.set_dialog_open(true);
        }
        "empty" => open(&g, state, 113),
        _ => {}
    }
}
