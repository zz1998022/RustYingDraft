use anyhow::Result;
use camino::Utf8Path;
use jy_template::{ReplacementMaterial, TemplateDraft};

/// 按素材名替换模板中的素材。
///
/// 这个入口更适合“模板里素材命名稳定”的场景，例如固定叫 `main_video`、
/// `bgm`、`voice_over` 这类可读名字。
pub fn run(
    draft_path: &Utf8Path,
    target_name: &str,
    media_type: crate::MediaTypeArg,
    source: &Utf8Path,
    material_name: Option<&str>,
    replace_crop: bool,
    output: Option<&Utf8Path>,
) -> Result<()> {
    let mut draft = TemplateDraft::load(draft_path)?;
    // 先把输入文件探测成统一的替换素材对象，再交给模板层处理。
    let material = match media_type {
        crate::MediaTypeArg::Video => ReplacementMaterial::Video(
            jy_media::material::create_video_material(source, material_name)?,
        ),
        crate::MediaTypeArg::Audio => ReplacementMaterial::Audio(
            jy_media::material::create_audio_material(source, material_name)?,
        ),
    };

    draft.replace_material_by_name(target_name, &material, replace_crop)?;
    if let Some(output_path) = output {
        draft.write_to(output_path)?;
        println!("Updated template written to: {}", output_path);
    } else {
        draft.save()?;
        println!("Updated template in place: {}", draft_path);
    }
    Ok(())
}
