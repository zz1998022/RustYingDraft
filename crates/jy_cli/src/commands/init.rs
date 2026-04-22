use anyhow::Result;
use camino::Utf8Path;
use serde_json::json;

use crate::output;

/// 生成一个最小可用的 project manifest。
///
/// 这个命令主要用于调试或手工维护 manifest，再交给 `generate` 命令生成草稿。
pub fn run(name: &str, width: u32, height: u32, fps: u32, output: &Utf8Path) -> Result<()> {
    let manifest = json!({
        "name": name,
        "canvas": {
            "width": width,
            "height": height,
            "fps": fps,
        },
        "maintrack_adsorb": true,
        "tracks": [],
        "video_materials": [],
        "audio_materials": [],
    });

    let content = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(output, content)?;
    output::emit_result(
        "init",
        &format!("Created project manifest: {output}"),
        json!({
            "manifest_path": output.as_str(),
            "name": name,
            "canvas": {
                "width": width,
                "height": height,
                "fps": fps,
            }
        }),
    );
    Ok(())
}
