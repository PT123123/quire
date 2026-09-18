// quire-shot — headless visual regression tool.
//
// Renders the real AppWindow through Slint's software renderer into an
// offscreen buffer and writes a BMP (PNG encoding would need an extra
// dependency; the capture script converts to PNG). No window is created,
// so shots are deterministic and immune to foreground/z-order issues —
// see docs/UI_ARCHITECTURE.md "Visual regression".
//
// Build: cargo build --features software --bin quire-shot
// Usage: quire-shot --out shot.bmp [--scene menu] [--w 1280] [--h 800]

use quire::app::controller;
use quire::app::state::{AppState, HandleArgs};
use quire::AppWindow;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, PlatformError, WindowAdapter};
use slint::{ComponentHandle, PhysicalSize, Rgb8Pixel, SharedPixelBuffer};
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static WINDOW: RefCell<Option<Rc<MinimalSoftwareWindow>>> = const { RefCell::new(None) };
}

struct HeadlessPlatform;

impl Platform for HeadlessPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        WINDOW.with(|slot| *slot.borrow_mut() = Some(window.clone()));
        Ok(window)
    }
}

/// Minimal 24-bit BMP writer (bottom-up rows, 4-byte padding).
fn write_bmp(path: &str, buffer: &SharedPixelBuffer<Rgb8Pixel>) -> Result<(), String> {
    let w = buffer.width() as usize;
    let h = buffer.height() as usize;
    let stride = (w * 3 + 3) & !3;
    let data_size = (stride * h + 54) as u32;
    let mut out = Vec::with_capacity(data_size as usize);
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&data_size.to_le_bytes());
    out.extend_from_slice(&[0u8; 4]);
    out.extend_from_slice(&54u32.to_le_bytes());
    out.extend_from_slice(&40u32.to_le_bytes()); // BITMAPINFOHEADER
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&24u16.to_le_bytes()); // bpp
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&((stride * h) as u32).to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes()); // 72 dpi
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&[0u8; 8]);
    let src = buffer.as_bytes();
    for y in (0..h).rev() {
        let row = &src[y * w * 3..(y + 1) * w * 3];
        for px in row.chunks_exact(3) {
            out.push(px[2]);
            out.push(px[1]);
            out.push(px[0]);
        }
        out.resize(out.len() + (stride - w * 3), 0);
    }
    std::fs::write(path, out).map_err(|e| e.to_string())
}

fn render(ui: &AppWindow, w: u32, h: u32) -> bool {
    WINDOW.with(|slot| -> bool {
        let window = slot.borrow().clone().expect("window adapter");
        window.draw_if_needed(|renderer| {
            let mut buffer = SharedPixelBuffer::<Rgb8Pixel>::new(w, h);
            let stride = buffer.width() as usize;
            renderer.render(buffer.make_mut_slice(), stride);
            BUFFER.with(|b| *b.borrow_mut() = Some(buffer));
        })
    })
}

fn parse(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() -> Result<(), String> {
    // Same rationale as the GUI binary: Slint's initial binding/layout
    // evaluation is deep; give it headroom (ADR-0009).
    let child = std::thread::Builder::new().stack_size(8 * 1024 * 1024)
        .spawn(|| run()).expect("spawn render thread");
    child.join().unwrap_or_else(|_| Err("render thread panicked".into()))
}

fn run() -> Result<(), String> {
    let argv: Vec<String> = std::env::args().collect();
    let out = parse(&argv, "--out").unwrap_or_else(|| "quire-shot.bmp".into());
    let scene = parse(&argv, "--scene");
    let w: u32 = parse(&argv, "--w").and_then(|v| v.parse().ok()).unwrap_or(1280);
    let h: u32 = parse(&argv, "--h").and_then(|v| v.parse().ok()).unwrap_or(800);

    slint::platform::set_platform(Box::new(HeadlessPlatform))
        .map_err(|e| format!("set_platform: {e:?}"))?;

    let ui = AppWindow::new().map_err(|e| e.to_string())?;
    ui.window().set_size(PhysicalSize::new(w, h));

    let args = HandleArgs { blocks: 0, auto_exit_secs: 0.0, bench_pages: 0 };
    let state = AppState::new(&args, None);
    controller::bind(&ui, &state);
    controller::wire(&ui, &state);
    if let Some(scene) = &scene {
        controller::apply_scene(&ui, &state, scene);
    }
    ui.show().map_err(|e| e.to_string())?;

    let drawn = render(&ui, w, h);
    if !drawn {
        return Err("window never requested a redraw".into());
    }


    if let Some(scene) = &scene {
        controller::apply_scene_overlay(&ui, &state, scene);
    }
    BUFFER.with(|b| *b.borrow_mut() = None);
    WINDOW.with(|slot| {
        if let Some(window) = slot.borrow().clone() {
            // the overlay flags mark popup state, not the window's redraw
            // flag — request the second pass explicitly
            window.request_redraw();
            window.draw_if_needed(|renderer| {
                let mut buffer = SharedPixelBuffer::<Rgb8Pixel>::new(w, h);
                let stride = buffer.width() as usize;
                renderer.render(buffer.make_mut_slice(), stride);
                BUFFER.with(|b| *b.borrow_mut() = Some(buffer));
            });
        }
    });

    let buffer = BUFFER.with(|b| b.borrow_mut().take());
    match buffer {
        Some(buffer) => {
            if let Some(parent) = std::path::Path::new(&out).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
            }
            write_bmp(&out, &buffer)?;
            println!("wrote {out}");
            Ok(())
        }
        None => Err("render produced no buffer".into()),
    }
}

thread_local! {
    static BUFFER: RefCell<Option<SharedPixelBuffer<Rgb8Pixel>>> = const { RefCell::new(None) };
}
