//! Earth Studio: an editor for the Earth programming language with a file
//! explorer, a tabbed code editor and a terminal that runs programs.
//!
//! `earth-studio [file-or-folder]` opens the given file or folder; this is
//! also how Windows opens a double-clicked `.earth` file.

// A window application: no console window behind it on Windows.
#![cfg_attr(not(test), windows_subsystem = "windows")]

mod app;
mod files;
mod highlight;
mod runner;
mod texts;
mod update;
mod workspace;

use std::path::PathBuf;

use eframe::egui;

/// Set to `glow` to draw with OpenGL instead of wgpu (DirectX, Vulkan or
/// Metal). Studio sets it itself when wgpu cannot open a window.
const RENDERER_VARIABLE: &str = "EARTH_STUDIO_RENDERER";

fn main() -> eframe::Result {
    let start = std::env::args_os().nth(1).map(PathBuf::from);
    let use_glow = std::env::var(RENDERER_VARIABLE).is_ok_and(|value| value == "glow");

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Earth Studio")
        .with_inner_size([1200.0, 780.0])
        .with_min_inner_size([640.0, 420.0]);
    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../../windows/earth.png")) {
        viewport = viewport.with_icon(icon);
    }
    let options = eframe::NativeOptions {
        viewport,
        renderer: if use_glow {
            eframe::Renderer::Glow
        } else {
            eframe::Renderer::Wgpu
        },
        ..Default::default()
    };

    let result = eframe::run_native(
        "Earth Studio",
        options,
        Box::new(move |context| Ok(Box::new(app::StudioApp::new(context, start)))),
    );

    // A window system can only be started once per process, so fall back to
    // OpenGL by starting Studio again rather than retrying here.
    if result.is_err() && !use_glow {
        if let Ok(executable) = std::env::current_exe() {
            let restarted = std::process::Command::new(executable)
                .args(std::env::args_os().skip(1))
                .env(RENDERER_VARIABLE, "glow")
                .status();
            if restarted.is_ok() {
                return Ok(());
            }
        }
    }
    result
}
