#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod pipeline;
mod theme;
mod views;
mod widgets;

#[cfg(target_os = "windows")]
mod windows_dark;

use app::App;
use iced::window;
use iced::Size;

fn main() -> iced::Result {
    // Spawn a thread to apply dark title bar on Windows after the window is created
    #[cfg(target_os = "windows")]
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(300));
        windows_dark::apply();
    });

    iced::application(App::title, App::update, App::view)
        .theme(App::theme)
        .subscription(App::subscription)
        .window(window::Settings {
            size: Size::new(1200.0, 800.0),
            min_size: Some(Size::new(600.0, 400.0)),
            icon: make_icon(),
            ..Default::default()
        })
        .run_with(App::new)
}

/// Generate a 32×32 RGBA icon: dark background with coloured log-level bars.
fn make_icon() -> Option<window::Icon> {
    const S: usize = 32;
    let mut px = vec![0u8; S * S * 4];

    // Fill background: dark navy (#1E1E2E)
    for chunk in px.chunks_exact_mut(4) {
        chunk[0] = 0x1E; chunk[1] = 0x1E; chunk[2] = 0x2E; chunk[3] = 0xFF;
    }

    // Helper: paint a rectangle (x0..x1, y0..y1) with (r,g,b,a)
    let mut rect = |x0: usize, y0: usize, x1: usize, y1: usize, r: u8, g: u8, b: u8, a: u8| {
        for y in y0..y1.min(S) {
            for x in x0..x1.min(S) {
                let i = (y * S + x) * 4;
                px[i] = r; px[i+1] = g; px[i+2] = b; px[i+3] = a;
            }
        }
    };

    // 5 log rows — each row: coloured level dot (left) + grey text bar (right)
    // (debug, info, info, warn, error)
    let rows: &[(u8, u8, u8, usize)] = &[
        (0x6C, 0x7E, 0xA8, 20), // debug: muted blue,  bar width 20
        (0xA6, 0xE3, 0xA1, 23), // info:  green,        bar width 23
        (0xA6, 0xE3, 0xA1, 18), // info:  green,        bar width 18
        (0xF9, 0xE2, 0xAF, 14), // warn:  yellow,       bar width 14
        (0xF3, 0x8B, 0xA8,  9), // error: red,          bar width  9
    ];

    for (i, &(r, g, b, bar_w)) in rows.iter().enumerate() {
        let y = 3 + i * 5;
        // Level dot: 3×3 pixels at x=2
        rect(2, y, 5, y + 3, r, g, b, 0xFF);
        // Text bar: 2px tall at x=7
        rect(7, y, 7 + bar_w, y + 2, 0x7F, 0x7F, 0x9A, 0xCC);
    }

    window::icon::from_rgba(px, S as u32, S as u32).ok()
}
