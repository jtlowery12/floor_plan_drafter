// Don't open a console window next to the app on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod canvas;
mod editor;
mod export;
mod geom;
mod icons;
mod kit;
mod model;
mod paint;
mod panels;
mod render;
mod start;
mod store;
mod symbols;
mod theme;
mod units;

use eframe::egui;
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    if let Ok(p) = std::env::var("DRAFTING_TABLE_DIR") {
        return PathBuf::from(p);
    }
    directories::ProjectDirs::from("", "", "DraftingTable")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("drafting-table-data"))
}

fn main() -> eframe::Result {
    let dir = data_dir();
    let store = match store::Store::open(&dir) {
        Ok(s) => s,
        Err(e) => {
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_title(theme::APP_NAME)
                .set_description(format!("Couldn't open the data folder:\n{}\n\n{e}", dir.display()))
                .show();
            std::process::exit(1);
        }
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(theme::APP_NAME)
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([1020.0, 620.0]),
        ..Default::default()
    };
    eframe::run_native(
        theme::APP_NAME,
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, store)))),
    )
}
