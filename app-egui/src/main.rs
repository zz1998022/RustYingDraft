mod app;
mod fonts;
mod job;
mod platform;
mod util;

use eframe::egui;

fn main() -> eframe::Result {
    // 最早一步注入随包 ffprobe 路径，确保后续媒体探测能找到它（详见 platform 模块）。
    platform::configure_bundled_ffprobe();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 520.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };

    eframe::run_native(
        "YingDraft 导入器",
        native_options,
        Box::new(|cc| {
            fonts::install_builtin_fonts(&cc.egui_ctx);
            Ok(Box::new(app::YingDraftApp::new()))
        }),
    )
}
