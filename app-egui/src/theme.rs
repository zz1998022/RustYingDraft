use eframe::egui::{self, Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle};

// 品牌配色，沿用 Tauri UI 的暖橙主色与浅色调。
pub(crate) const ACCENT: Color32 = Color32::from_rgb(0xEA, 0x58, 0x0C); // 主按钮橙
pub(crate) const ACCENT_HOVER: Color32 = Color32::from_rgb(0xF9, 0x73, 0x16);
pub(crate) const KICKER: Color32 = Color32::from_rgb(0xD9, 0x77, 0x06); // Step 小标题
pub(crate) const TITLE: Color32 = Color32::from_rgb(0x1F, 0x2A, 0x37);
pub(crate) const MUTED: Color32 = Color32::from_rgb(0x64, 0x74, 0x8B);

const PANEL_BG: Color32 = Color32::from_rgb(0xFB, 0xF7, 0xF0); // 暖白背景
const CARD_BG: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);
const CARD_BORDER: Color32 = Color32::from_rgb(0xE6, 0xE8, 0xEC);

/// 应用整体主题：浅色 + 橙色强调 + 更圆润的控件与更舒展的间距。
pub(crate) fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let mut visuals = egui::Visuals::light();

    visuals.panel_fill = PANEL_BG;
    visuals.window_fill = CARD_BG;
    visuals.extreme_bg_color = Color32::from_rgb(0xF6, 0xF8, 0xFA); // 输入框 / 代码区底色
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.28);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.hyperlink_color = ACCENT;

    // 控件圆角加大，靠近原 UI 的圆润观感；hover 时透出一点暖色描边。
    let radius = CornerRadius::same(8);
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = radius;
    }
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT_HOVER.gamma_multiply(0.6));

    style.visuals = visuals;
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional));
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(15.0, FontFamily::Proportional));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::new(15.0, FontFamily::Proportional));

    ctx.set_style(style);
}

/// 分步卡片的外观：白底、细描边、圆角、内边距与一层柔和投影。
pub(crate) fn card_frame() -> egui::Frame {
    egui::Frame::default()
        .fill(CARD_BG)
        .stroke(Stroke::new(1.0, CARD_BORDER))
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(18))
        .shadow(egui::epaint::Shadow {
            offset: [0, 6],
            blur: 18,
            spread: 0,
            color: Color32::from_black_alpha(18),
        })
}

/// 加载随包窗口图标（沿用 Tauri 的 icon.png）。
pub(crate) fn app_icon() -> egui::IconData {
    let bytes = include_bytes!("../assets/icon.png");
    let image = image::load_from_memory(bytes)
        .expect("内置窗口图标应为合法 PNG")
        .into_rgba8();
    let (width, height) = image.dimensions();
    egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    }
}
