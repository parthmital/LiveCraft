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
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct GuiOptions {
    pub project_path: Option<PathBuf>,
    pub screenshot_path: Option<PathBuf>,
    pub initial_view: Option<String>,
    pub initial_tab: Option<usize>,
    pub theme: Option<String>,
}

pub fn run_gui(project_path: Option<&Path>) -> anyhow::Result<()> {
    run_gui_with_options(GuiOptions {
        project_path: project_path.map(|p| p.to_path_buf()),
        ..Default::default()
    })
}

pub fn run_gui_with_options(options: GuiOptions) -> anyhow::Result<()> {
    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: wgpu_options(),
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0_f32, 800.0_f32])
            .with_min_inner_size([900.0_f32, 600.0_f32])
            .with_title("LiveCraft"),
        ..Default::default()
    };

    eframe::run_native(
        "LiveCraft",
        native_options,
        Box::new(move |_cc| Ok(Box::new(LiveCraftApp::new_with_options(options)))),
    )
    .map_err(|e| anyhow::anyhow!("GUI error: {}", e))
}

/// wgpu setup: DX12 on Windows unless `WGPU_BACKEND` is set, because the Vulkan
/// backend crashed on the reference Windows machine (2026-10-27).
fn wgpu_options() -> eframe::egui_wgpu::WgpuConfiguration {
    use eframe::egui_wgpu::{WgpuConfiguration, WgpuSetup, WgpuSetupCreateNew};
    let mut setup = WgpuSetupCreateNew::without_display_handle();
    if cfg!(windows) && std::env::var_os("WGPU_BACKEND").is_none() {
        setup.instance_descriptor.backends = eframe::wgpu::Backends::DX12;
    }
    WgpuConfiguration {
        wgpu_setup: WgpuSetup::CreateNew(setup),
        ..Default::default()
    }
}
