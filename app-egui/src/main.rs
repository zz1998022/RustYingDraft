mod app;
mod fonts;
mod job;
mod platform;
mod theme;
mod util;

use std::sync::Arc;

use eframe::egui;

fn main() -> eframe::Result {
    // 最早一步注入随包 ffprobe 路径，确保后续媒体探测能找到它（详见 platform 模块）。
    platform::configure_bundled_ffprobe();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([720.0, 800.0])
            .with_resizable(false) // 固定窗口尺寸，避免拉伸破坏布局
            .with_icon(Arc::new(theme::app_icon())),
        ..Default::default()
    };

    eframe::run_native(
        "YingDraft 导入器",
        native_options,
        Box::new(|cc| {
            fonts::install_builtin_fonts(&cc.egui_ctx);
            theme::apply(&cc.egui_ctx);
            Ok(Box::new(app::YingDraftApp::new()))
        }),
    )
}
