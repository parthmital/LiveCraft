"""Apply the non-mechanical part of the egui 0.37 migration to crates/livecraft-gui.

Purpose: eframe 0.37 replaced `App::update(ctx)` with `App::ui(ui)` and turned screenshots into a
callback (`Context::request_screenshot`). This script rewrites the head of `impl eframe::App` in
app.rs and switches lib.rs to the wgpu renderer with DX12 as the Windows default.
Validated on 2026-10-27 together with egui_037_migrate.py: clippy clean, tests pass, and
`livecraft gui --screenshot` works for both views.
Inputs: the crate directory (crates/livecraft-gui).
Outputs: rewrites src/app.rs and src/lib.rs in place.
Run (from the repository root, after egui_037_migrate.py):
    ./.venv/Scripts/python plans/livecraft-1.0/tools/egui_037_finish.py crates/livecraft-gui
"""

import os
import sys

APP_HEAD = """impl eframe::App for LiveCraftApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        self.frame_counter += 1;
        if let Some(path) = self.screenshot_path.clone() {
            ctx.request_repaint();
            if self.frame_counter == 5 {
                ctx.request_screenshot(move |image| save_screenshot_and_exit(&path, &image));
            }
            if self.frame_counter > 50 {
                log::error!("Screenshot timeout");
                std::process::exit(1);
            }
        }

"""

SAVE_FN = """
/// Writes an egui screenshot to `path` as PNG and exits the process (0 on success, 1 on failure).
fn save_screenshot_and_exit(path: &std::path::Path, image: &egui::ColorImage) -> ! {
    let [width, height] = image.size;
    let rgba: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match image::save_buffer(path, &rgba, width as u32, height as u32, image::ColorType::Rgba8) {
        Ok(()) => {
            println!("Screenshot successfully saved to: {}", path.display());
            std::process::exit(0);
        }
        Err(e) => {
            log::error!("Failed to save screenshot: {e}");
            std::process::exit(1);
        }
    }
}
"""

WGPU_FN = """
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
"""


def rewrite_app(path):
    with open(path, encoding="utf-8") as handle:
        text = handle.read()
    start_marker = "impl eframe::App for LiveCraftApp {"
    end_marker = "        apply_live_style(ctx, &self.palette);"
    start = text.find(start_marker)
    end = text.find(end_marker)
    if start < 0 or end < 0 or end < start:
        sys.exit(f"{path}: markers not found; log a blocker")
    if "fn save_screenshot_and_exit" in text:
        sys.exit(f"{path}: already migrated")
    text = text[:start] + APP_HEAD + text[end:]
    text = text.rstrip() + "\n" + SAVE_FN
    with open(path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)
    print(f"rewrote App::ui head in {path}")


def rewrite_lib(path):
    with open(path, encoding="utf-8") as handle:
        text = handle.read()
    old = "        renderer: eframe::Renderer::Glow,\n"
    if old not in text:
        sys.exit(f"{path}: renderer line not found; log a blocker")
    text = text.replace(
        old,
        "        renderer: eframe::Renderer::Wgpu,\n        wgpu_options: wgpu_options(),\n",
        1,
    )
    text = text.rstrip() + "\n" + WGPU_FN
    with open(path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)
    print(f"switched {path} to wgpu with DX12 default on Windows")


def main():
    if len(sys.argv) != 2:
        sys.exit("usage: egui_037_finish.py <crates/livecraft-gui>")
    crate = sys.argv[1]
    rewrite_app(os.path.join(crate, "src", "app.rs"))
    rewrite_lib(os.path.join(crate, "src", "lib.rs"))


if __name__ == "__main__":
    main()
