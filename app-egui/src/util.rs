use eframe::egui;

/// 一行「标签 + 输入框 + 选择按钮」的通用控件；点击按钮时用 pick 回调填入路径。
pub(crate) fn path_row(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut String,
    button: &str,
    pick: impl FnOnce() -> Option<String>,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.text_edit_singleline(value);
        if ui.button(button).clicked() {
            if let Some(path) = pick() {
                *value = path;
            }
        }
    });
}

/// 把“选文件”选中的路径归一化为底层可用的 source。
///
/// 选中 `bundle.json` 时取其父目录；选中 `.zip` 或其它文件时原样返回，
/// 交给 `jy_bundle` 按 zip 或目录源处理。
pub(crate) fn resolve_picked_source(path: &std::path::Path) -> String {
    if path.file_name().and_then(|name| name.to_str()) == Some("bundle.json") {
        if let Some(parent) = path.parent() {
            return parent.display().to_string();
        }
    }
    path.display().to_string()
}

/// 把用户输入的草稿名清洗成安全的目录名：
/// 替换文件系统非法字符，并把连续空白折叠成下划线；为空时回退到默认名。
pub(crate) fn sanitize_draft_name(value: &str) -> String {
    let sanitized = value
        .trim()
        .chars()
        .map(|ch| match ch {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_");

    if sanitized.is_empty() {
        "imported_bundle".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picked_bundle_json_resolves_to_parent_dir() {
        let source =
            resolve_picked_source(std::path::Path::new(r"C:\packages\case_001\bundle.json"));

        assert_eq!(source, r"C:\packages\case_001");
    }

    #[test]
    fn picked_zip_is_passed_through() {
        let source = resolve_picked_source(std::path::Path::new(r"C:\packages\case_001.zip"));

        assert_eq!(source, r"C:\packages\case_001.zip");
    }
}
