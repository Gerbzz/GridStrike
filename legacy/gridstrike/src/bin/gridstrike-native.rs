use std::path::PathBuf;
use std::process::ExitCode;

use eframe::egui;
use gridstrike::native_ui::NativeEditor;

fn main() -> ExitCode {
    let Some(path) = std::env::args_os().nth(1).map(PathBuf::from) else {
        eprintln!("Usage: gridstrike-native FILE.gstrike");
        return ExitCode::FAILURE;
    };
    let editor = match NativeEditor::open_or_create(path) {
        Ok(editor) => editor,
        Err(error) => {
            eprintln!("GridStrike: {error}");
            return ExitCode::FAILURE;
        }
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("GridStrike")
            .with_inner_size([1350.0, 820.0])
            .with_min_inner_size([900.0, 560.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    match eframe::run_native(
        "GridStrike",
        options,
        Box::new(move |_cc| Ok(Box::new(editor))),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("GridStrike window: {error}");
            ExitCode::FAILURE
        }
    }
}
