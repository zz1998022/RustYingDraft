use eframe::egui;

const BUILTIN_CJK_FONT_NAME: &str = "noto_sans_cjk_sc_regular";
const BUILTIN_CJK_FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/NotoSansCJKsc-Regular.otf");

pub(crate) fn install_builtin_fonts(ctx: &egui::Context) {
    ctx.set_fonts(builtin_font_definitions());
}

fn builtin_font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        BUILTIN_CJK_FONT_NAME.to_string(),
        egui::FontData::from_static(BUILTIN_CJK_FONT_BYTES).into(),
    );

    // egui 默认字体不含中文，把内置 CJK 字体插到两个字体族首位作为主字体，
    // 保证界面文案和错误详情（等宽框）都能正常显示中文。
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, BUILTIN_CJK_FONT_NAME.to_string());
    }

    fonts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_font_is_registered_for_text_and_error_details() {
        let fonts = builtin_font_definitions();
        let proportional = fonts
            .families
            .get(&egui::FontFamily::Proportional)
            .expect("proportional font family exists");
        let monospace = fonts
            .families
            .get(&egui::FontFamily::Monospace)
            .expect("monospace font family exists");

        assert!(fonts.font_data.contains_key(BUILTIN_CJK_FONT_NAME));
        assert_eq!(
            proportional.first().map(String::as_str),
            Some(BUILTIN_CJK_FONT_NAME)
        );
        assert_eq!(
            monospace.first().map(String::as_str),
            Some(BUILTIN_CJK_FONT_NAME)
        );
    }
}
