mod constants;
mod error;
mod events;
mod file_handler;
mod file_monitor;
mod markdown_generator;
mod ui_tree_handler;
mod app;

use eframe::NativeOptions;
use log::info;

use app::MarkdownContextBuilderApp;

fn main() -> Result<(), eframe::Error> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();
    
    info!("Starting Context Builder - Rust Edition");

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_title("Context Builder - Rust Edition"),
        ..Default::default()
    };

    eframe::run_native(
        "Context Builder",
        options,
        Box::new(|cc| Box::new(MarkdownContextBuilderApp::new(cc))),
    )
}
