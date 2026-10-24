pub mod app;
pub mod arrange;
pub mod browser;
pub mod devices;
pub mod mixer;
pub mod pianoroll;
pub mod session;
pub mod style;
pub mod transport;
pub mod widgets;

pub use app::LiveCraftApp;
use std::path::Path;

pub fn run_gui(project_path: Option<&Path>) -> anyhow::Result<()> {
    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0_f32, 800.0_f32])
            .with_min_inner_size([900.0_f32, 600.0_f32])
            .with_title("LiveCraft - Ableton Live Workstation (Rust)"),
        ..Default::default()
    };

    let p_path = project_path.map(|p| p.to_path_buf());

    eframe::run_native(
        "LiveCraft",
        native_options,
        Box::new(move |_cc| Ok(Box::new(LiveCraftApp::new(p_path.as_deref())))),
    )
    .map_err(|e| anyhow::anyhow!("GUI error: {}", e))
}
