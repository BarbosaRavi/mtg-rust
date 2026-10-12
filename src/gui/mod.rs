mod app;
mod card_detail;
mod card_grid;
mod cards_tab;
mod collection_panel;
mod collection_sync;
mod deck_filters;
mod decks_tab;
mod images;
mod settings;
mod startup;

use anyhow::{Result, anyhow};
use card_detail::CardDetail;
use collection_panel::CollectionAction;
use eframe::egui;
use rust_mtg::config::Config;
use std::path::PathBuf;

#[derive(Default)]
struct TabOutput {
    detail: Option<CardDetail>,
    action: Option<CollectionAction>,
}

pub fn run(config: Config, log: Option<PathBuf>, raw_dir: Option<PathBuf>) -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("rust-mtg")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0]),
        ..Default::default()
    };

    eframe::run_native(
        "rust-mtg",
        options,
        Box::new(|creation| {
            egui_extras::install_image_loaders(&creation.egui_ctx);
            let ctx = &creation.egui_ctx;
            Ok(Box::new(startup::Startup::new(ctx, config, log, raw_dir)))
        }),
    )
    .map_err(|error| anyhow!("erro na interface gráfica: {error}"))
}
