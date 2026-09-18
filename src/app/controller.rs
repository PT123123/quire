// UI callbacks land here. The controller owns *when* something happens;
// state.rs owns *what* the data looks like; nothing here touches DB or disk.
//
// 1.18 global-handle pattern: `ui.global::<UIState>()` borrows the handle, so
// 'static callbacks capture a Weak and upgrade() it at fire time.

use crate::app::state::AppState;
use crate::{AppWindow, SidebarNode, UIState};
use slint::{ComponentHandle, Global, Model};
use std::rc::Rc;

pub fn bind(ui: &AppWindow, state: &Rc<AppState>) {
    let g = ui.global::<UIState>();
    g.set_sidebar_rows(state.sidebar_model());
    g.set_blocks(state.blocks_model());
    g.set_commands(state.commands_model());
    g.set_page_title("Project Atlas".into());
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
                2 => g.set_sidebar_open(!g.get_sidebar_open()),
                3 => g.set_dark(!g.get_dark()),
                other if other >= 50 => {
                    if let Some(title) = s.page_titles.get((other - 50) as usize) {
                        open_page(&g, &s, title);
                    }
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

    {
        let gw = gw.clone();
        let s = state.clone();
        ui.global::<UIState>().on_node_activated(move || {
            let g = gw.upgrade().unwrap();
            let id = g.get_sidebar_selected_id();
            let rows: Vec<SidebarNode> = s.sidebar.iter().collect();
            if let Some(row) = rows.iter().find(|r| r.id == id) {
                if matches!(row.kind.as_str(), "page" | "favorite" | "recent") {
                    let title = row.label.to_string();
                    open_page(&g, &s, &title);
                }
            }
        });
    }

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

fn open_page(g: &UIState<'static>, state: &Rc<AppState>, title: &str) {
    g.set_page_title(title.into());
    state.load_page(title);
    let mut rows: Vec<SidebarNode> = state.sidebar.iter().collect();
    for r in rows.iter_mut() {
        r.selected = r.label == title;
    }
    state.set_sidebar_rows(rows);
}
