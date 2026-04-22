mod error;

use camino::{Utf8Path, Utf8PathBuf};
pub use error::TemplateError;
use jy_schema::{AudioMaterialRef, MaterialKind, TimeRange, TrackKind, VideoMaterialRef};
use serde_json::{json, Value};

/// 替换素材后，如果新素材更短，时间范围该如何收缩。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShrinkMode {
    CutHead,
    CutTail,
    CutTailAlign,
    Shrink,
}

/// 替换素材后，如果新素材更长，时间范围该如何延展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtendMode {
    CutMaterialTail,
    ExtendHead,
    ExtendTail,
    PushTail,
}

/// 模板替换时使用的统一素材类型。
#[derive(Debug, Clone)]
pub enum ReplacementMaterial {
    Video(VideoMaterialRef),
    Audio(AudioMaterialRef),
}

impl ReplacementMaterial {
    fn is_video(&self) -> bool {
        matches!(self, Self::Video(_))
    }

    fn id(&self) -> &str {
        match self {
            Self::Video(mat) => &mat.id,
            Self::Audio(mat) => &mat.id,
        }
    }

    fn duration(&self) -> u64 {
        match self {
            Self::Video(mat) => mat.duration,
            Self::Audio(mat) => mat.duration,
        }
    }

    /// 转换成可以直接写回草稿 `materials` 区域的 JSON 对象。
    fn as_json(&self) -> Value {
        match self {
            Self::Video(mat) => json!({
                "audio_fade": Value::Null,
                "category_id": "",
                "category_name": "local",
                "check_flag": 63487,
                "crop": {
                    "upper_left_x": mat.crop.upper_left_x,
                    "upper_left_y": mat.crop.upper_left_y,
                    "upper_right_x": mat.crop.upper_right_x,
                    "upper_right_y": mat.crop.upper_right_y,
                    "lower_left_x": mat.crop.lower_left_x,
                    "lower_left_y": mat.crop.lower_left_y,
                    "lower_right_x": mat.crop.lower_right_x,
                    "lower_right_y": mat.crop.lower_right_y,
                },
                "crop_ratio": "free",
                "crop_scale": 1.0,
                "duration": mat.duration,
                "height": mat.height,
                "id": mat.id,
                "local_material_id": "",
                "material_id": mat.id,
                "material_name": mat.name,
                "media_path": "",
                "path": mat.path.as_str().replace('\\', "/"),
                "type": match mat.kind {
                    MaterialKind::Video => "video",
                    MaterialKind::Photo => "photo",
                    MaterialKind::Audio => "video",
                },
                "width": mat.width,
            }),
            Self::Audio(mat) => json!({
                "app_id": 0,
                "category_id": "",
                "category_name": "local",
                "check_flag": 3,
                "copyright_limit_type": "none",
                "duration": mat.duration,
                "effect_id": "",
                "formula_id": "",
                "id": mat.id,
                "local_material_id": mat.id,
                "music_id": mat.id,
                "name": mat.name,
                "path": mat.path.as_str().replace('\\', "/"),
                "source_platform": 0,
                "type": "extract_music",
                "wave_points": [],
            }),
        }
    }
}

/// 轨道选择器。
///
/// 支持按名称或按“同类型轨道索引”定位。
#[derive(Debug, Clone, Default)]
pub struct TrackSelector {
    pub name: Option<String>,
    pub index: Option<usize>,
}

/// 现有草稿的模板包装对象。
#[derive(Debug, Clone)]
pub struct TemplateDraft {
    path: Option<Utf8PathBuf>,
    content: Value,
}

impl TemplateDraft {
    /// 从 `draft_content.json` 文件加载模板。
    pub fn load(path: &Utf8Path) -> Result<Self, TemplateError> {
        let content = std::fs::read_to_string(path)?;
        let content = serde_json::from_str(&content)?;
        Ok(Self {
            path: Some(path.to_path_buf()),
            content,
        })
    }

    /// 从剪映草稿目录加载模板。
    pub fn load_from_draft_dir(draft_dir: &Utf8Path) -> Result<Self, TemplateError> {
        Self::load(&draft_dir.join("draft_content.json"))
    }

    /// 复制整个草稿目录，并返回复制后目录对应的模板对象。
    pub fn duplicate_draft_dir(
        template_dir: &Utf8Path,
        new_draft_dir: &Utf8Path,
        allow_replace: bool,
    ) -> Result<Self, TemplateError> {
        if !template_dir.exists() {
            return Err(TemplateError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("template draft directory not found: {template_dir}"),
            )));
        }

        if new_draft_dir.exists() {
            if !allow_replace {
                return Err(TemplateError::Io(std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    format!("draft directory already exists: {new_draft_dir}"),
                )));
            }
            std::fs::remove_dir_all(new_draft_dir)?;
        }

        copy_dir_all(template_dir, new_draft_dir)?;
        Self::load_from_draft_dir(new_draft_dir)
    }

    /// 直接从 JSON 值构造模板对象，适合测试或中间转换使用。
    pub fn from_value(content: Value) -> Self {
        Self {
            path: None,
            content,
        }
    }

    /// 获取底层 JSON 的只读引用。
    pub fn content(&self) -> &Value {
        &self.content
    }

    /// 取出底层 JSON 并消费模板对象。
    pub fn into_content(self) -> Value {
        self.content
    }

    /// 保存回加载时的原始路径。
    pub fn save(&self) -> Result<(), TemplateError> {
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| TemplateError::UnsupportedStructure("no save path configured".into()))?;
        self.write_to(path)
    }

    /// 保存到指定草稿目录。
    pub fn save_to_draft_dir(&self, draft_dir: &Utf8Path) -> Result<(), TemplateError> {
        self.write_to(&draft_dir.join("draft_content.json"))
    }

    /// 保存到指定 JSON 文件。
    pub fn write_to(&self, path: &Utf8Path) -> Result<(), TemplateError> {
        let serialized = serde_json::to_string_pretty(&self.content)?;
        std::fs::write(path, serialized)?;
        Ok(())
    }

    /// 按素材名称替换模板中的视频或音频素材。
    pub fn replace_material_by_name(
        &mut self,
        material_name: &str,
        material: &ReplacementMaterial,
        replace_crop: bool,
    ) -> Result<(), TemplateError> {
        let materials = materials_mut(&mut self.content)?;
        let key = if material.is_video() {
            "videos"
        } else {
            "audios"
        };
        let name_key = if material.is_video() {
            "material_name"
        } else {
            "name"
        };
        let target_list = get_array_mut(materials, key)?;

        let mut matches = target_list
            .iter_mut()
            .filter(|mat| mat.get(name_key).and_then(Value::as_str) == Some(material_name));

        let target = matches
            .next()
            .ok_or_else(|| TemplateError::MaterialNotFound {
                name: material_name.to_string(),
            })?;
        if matches.next().is_some() {
            return Err(TemplateError::AmbiguousMaterial {
                name: material_name.to_string(),
            });
        }

        match material {
            ReplacementMaterial::Video(video) => {
                target["material_name"] = json!(video.name);
                target["path"] = json!(video.path.as_str().replace('\\', "/"));
                target["duration"] = json!(video.duration);
                target["width"] = json!(video.width);
                target["height"] = json!(video.height);
                target["type"] = json!(match video.kind {
                    MaterialKind::Video => "video",
                    MaterialKind::Photo => "photo",
                    MaterialKind::Audio => "video",
                });
                if replace_crop {
                    target["crop"] = material.as_json()["crop"].clone();
                }
            }
            ReplacementMaterial::Audio(audio) => {
                target["name"] = json!(audio.name);
                target["path"] = json!(audio.path.as_str().replace('\\', "/"));
                target["duration"] = json!(audio.duration);
            }
        }

        Ok(())
    }

    /// 替换单段文本。
    pub fn replace_text(
        &mut self,
        selector: &TrackSelector,
        segment_index: usize,
        text: &str,
        recalc_style: bool,
    ) -> Result<(), TemplateError> {
        let replacements = vec![text.to_string()];
        self.replace_texts(selector, segment_index, &replacements, recalc_style)
    }

    /// 统一替换文本，兼容普通文本和多段 `text_template`。
    pub fn replace_texts(
        &mut self,
        selector: &TrackSelector,
        segment_index: usize,
        texts: &[String],
        recalc_style: bool,
    ) -> Result<(), TemplateError> {
        let material_id = {
            let track = select_track_mut(&mut self.content, TrackKind::Text, selector)?;
            let segments = get_value_array_mut(track, "segments")?;
            let segment =
                segments
                    .get(segment_index)
                    .ok_or(TemplateError::SegmentIndexOutOfRange {
                        index: segment_index,
                    })?;
            segment
                .get("material_id")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    TemplateError::UnsupportedStructure("missing text material_id".into())
                })?
                .to_string()
        };

        let materials = materials_mut(&mut self.content)?;

        if texts.len() > 1 && replace_text_template(materials, &material_id, texts, recalc_style)? {
            return Ok(());
        }

        if replace_text_material(materials, &material_id, texts, recalc_style)? {
            return Ok(());
        }

        if replace_text_template(materials, &material_id, texts, recalc_style)? {
            return Ok(());
        }

        Err(TemplateError::MaterialNotFound { name: material_id })
    }

    /// 按轨道和片段位置替换模板素材。
    pub fn replace_material_by_seg(
        &mut self,
        kind: TrackKind,
        selector: &TrackSelector,
        segment_index: usize,
        material: &ReplacementMaterial,
        source_timerange: Option<TimeRange>,
        shrink: ShrinkMode,
        extend: &[ExtendMode],
    ) -> Result<(), TemplateError> {
        match (kind, material) {
            (TrackKind::Video, ReplacementMaterial::Video(_))
            | (TrackKind::Audio, ReplacementMaterial::Audio(_)) => {}
            _ => return Err(TemplateError::MaterialTypeMismatch),
        }

        {
            let track = select_track_mut(&mut self.content, kind, selector)?;
            let segments = get_value_array_mut(track, "segments")?;
            if segment_index >= segments.len() {
                return Err(TemplateError::SegmentIndexOutOfRange {
                    index: segment_index,
                });
            }

            let current_target = parse_timerange(&segments[segment_index]["target_timerange"])?;
            let mut new_source = match (source_timerange, material) {
                (Some(range), _) => range,
                (None, ReplacementMaterial::Video(video)) if video.kind == MaterialKind::Photo => {
                    TimeRange::new(0, current_target.duration)
                }
                (None, _) => TimeRange::new(0, material.duration()),
            };

            process_timerange(segments, segment_index, &mut new_source, shrink, extend)?;
            segments[segment_index]["source_timerange"] = timerange_json(&new_source);
            segments[segment_index]["material_id"] = json!(material.id());
        }

        add_material_if_missing(&mut self.content, material)?;
        Ok(())
    }

    /// 检查模板中可复用的贴纸、文字气泡和文字特效资源。
    pub fn inspect_material(&self) -> TemplateInspection {
        let mut inspection = TemplateInspection::default();
        if let Some(materials) = self.content.get("materials").and_then(Value::as_object) {
            if let Some(stickers) = materials.get("stickers").and_then(Value::as_array) {
                inspection.stickers = stickers
                    .iter()
                    .filter_map(|sticker| {
                        Some(StickerInfo {
                            resource_id: sticker
                                .get("resource_id")
                                .and_then(Value::as_str)?
                                .to_string(),
                            name: sticker
                                .get("name")
                                .and_then(Value::as_str)
                                .map(str::to_string),
                        })
                    })
                    .collect();
            }

            if let Some(effects) = materials.get("effects").and_then(Value::as_array) {
                for effect in effects {
                    match effect.get("type").and_then(Value::as_str) {
                        Some("text_shape") => {
                            if let Some(resource_id) =
                                effect.get("resource_id").and_then(Value::as_str)
                            {
                                inspection.text_bubbles.push(TextBubbleInfo {
                                    effect_id: effect
                                        .get("effect_id")
                                        .and_then(Value::as_str)
                                        .unwrap_or_default()
                                        .to_string(),
                                    resource_id: resource_id.to_string(),
                                    name: effect
                                        .get("name")
                                        .and_then(Value::as_str)
                                        .map(str::to_string),
                                });
                            }
                        }
                        Some("text_effect") => {
                            if let Some(resource_id) =
                                effect.get("resource_id").and_then(Value::as_str)
                            {
                                inspection.text_effects.push(TextEffectInfo {
                                    resource_id: resource_id.to_string(),
                                    name: effect
                                        .get("name")
                                        .and_then(Value::as_str)
                                        .map(str::to_string),
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        inspection
    }
}

/// 模板资源检查结果。
#[derive(Debug, Clone, Default)]
pub struct TemplateInspection {
    pub stickers: Vec<StickerInfo>,
    pub text_bubbles: Vec<TextBubbleInfo>,
    pub text_effects: Vec<TextEffectInfo>,
}

/// 贴纸资源信息。
#[derive(Debug, Clone)]
pub struct StickerInfo {
    pub resource_id: String,
    pub name: Option<String>,
}

/// 文本气泡资源信息。
#[derive(Debug, Clone)]
pub struct TextBubbleInfo {
    pub effect_id: String,
    pub resource_id: String,
    pub name: Option<String>,
}

/// 文本特效资源信息。
#[derive(Debug, Clone)]
pub struct TextEffectInfo {
    pub resource_id: String,
    pub name: Option<String>,
}

/// 取得 `materials` 对象的可变引用。
fn materials_mut(
    content: &mut Value,
) -> Result<&mut serde_json::Map<String, Value>, TemplateError> {
    content
        .get_mut("materials")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| TemplateError::UnsupportedStructure("missing materials".into()))
}

/// 从 JSON 对象中取出指定数组字段。
fn get_array_mut<'a>(
    object: &'a mut serde_json::Map<String, Value>,
    key: &str,
) -> Result<&'a mut Vec<Value>, TemplateError> {
    object
        .get_mut(key)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| TemplateError::UnsupportedStructure(format!("missing array: {key}")))
}

/// 从 `Value` 中取出指定数组字段。
fn get_value_array_mut<'a>(
    value: &'a mut Value,
    key: &str,
) -> Result<&'a mut Vec<Value>, TemplateError> {
    value
        .get_mut(key)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| TemplateError::UnsupportedStructure(format!("missing array: {key}")))
}

/// 根据轨道类型与选择器定位一条轨道。
fn select_track_mut<'a>(
    content: &'a mut Value,
    kind: TrackKind,
    selector: &TrackSelector,
) -> Result<&'a mut Value, TemplateError> {
    let tracks = content
        .get_mut("tracks")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| TemplateError::UnsupportedStructure("missing tracks".into()))?;

    let mut matches: Vec<usize> = tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| track.get("type").and_then(Value::as_str) == Some(kind.to_str()))
        .enumerate()
        .filter(|(same_type_index, (_, track))| {
            selector
                .name
                .as_deref()
                .map(|name| track.get("name").and_then(Value::as_str) == Some(name))
                .unwrap_or(true)
                && selector
                    .index
                    .map(|target| *same_type_index == target)
                    .unwrap_or(true)
        })
        .map(|(_, (actual_index, _))| actual_index)
        .collect();

    if matches.is_empty() {
        return Err(TemplateError::TrackNotFound);
    }
    if matches.len() > 1 {
        return Err(TemplateError::AmbiguousTrack);
    }

    Ok(&mut tracks[matches.remove(0)])
}

/// 按普通文本素材的格式替换文本。
fn replace_text_material(
    materials: &mut serde_json::Map<String, Value>,
    material_id: &str,
    texts: &[String],
    recalc_style: bool,
) -> Result<bool, TemplateError> {
    if texts.len() != 1 {
        return Err(TemplateError::InvalidTextReplacement {
            expected: 1,
            actual: texts.len(),
        });
    }
    let text = &texts[0];
    let texts = get_array_mut(materials, "texts")?;
    for material in texts.iter_mut() {
        if material.get("id").and_then(Value::as_str) != Some(material_id) {
            continue;
        }

        let content = material
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let mut content_json: Value = serde_json::from_str(content)?;
        let old_text = content_json
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if recalc_style {
            recalc_styles(&mut content_json, &old_text, text);
        }
        content_json["text"] = json!(text);
        material["content"] = json!(serde_json::to_string(&content_json)?);
        return Ok(true);
    }
    Ok(false)
}

/// 按 `text_template` 的多子文本格式替换文本。
fn replace_text_template(
    materials: &mut serde_json::Map<String, Value>,
    material_id: &str,
    texts: &[String],
    recalc_style: bool,
) -> Result<bool, TemplateError> {
    let template_ids: Vec<String> = materials
        .get("text_templates")
        .and_then(Value::as_array)
        .and_then(|templates| {
            templates
                .iter()
                .find(|template| template.get("id").and_then(Value::as_str) == Some(material_id))
        })
        .and_then(|template| {
            template
                .get("text_info_resources")
                .and_then(Value::as_array)
                .cloned()
        })
        .map(|resources| {
            resources
                .iter()
                .filter_map(|resource| resource.get("text_material_id").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    if template_ids.is_empty() {
        return Ok(false);
    }

    if texts.len() > template_ids.len() {
        return Err(TemplateError::InvalidTextReplacement {
            expected: template_ids.len(),
            actual: texts.len(),
        });
    }

    for (sub_material_id, text) in template_ids.iter().zip(texts.iter()) {
        let single = vec![text.clone()];
        replace_text_material(materials, sub_material_id, &single, recalc_style)?;
    }

    Ok(true)
}

/// 按新旧文本长度比例重算样式范围。
fn recalc_styles(content_json: &mut Value, old_text: &str, new_text: &str) {
    let old_len = old_text.chars().count().max(1);
    let new_len = new_text.chars().count();

    let Some(styles) = content_json.get_mut("styles").and_then(Value::as_array_mut) else {
        return;
    };

    styles.retain_mut(|style| {
        let Some(range) = style.get_mut("range").and_then(Value::as_array_mut) else {
            return true;
        };
        if range.len() != 2 {
            return true;
        }

        let start = range[0].as_u64().unwrap_or(0) as usize;
        let end = range[1].as_u64().unwrap_or(0) as usize;
        let new_start = ((start as f64 / old_len as f64) * new_len as f64).ceil() as u64;
        let new_end = ((end as f64 / old_len as f64) * new_len as f64).ceil() as u64;
        range[0] = json!(new_start);
        range[1] = json!(new_end);
        new_start != new_end
    });
}

/// 如果新素材尚未存在于草稿 `materials` 中，则自动补进去。
fn add_material_if_missing(
    content: &mut Value,
    material: &ReplacementMaterial,
) -> Result<(), TemplateError> {
    let materials = materials_mut(content)?;
    let key = if material.is_video() {
        "videos"
    } else {
        "audios"
    };
    let entries = get_array_mut(materials, key)?;
    let exists = entries
        .iter()
        .any(|entry| entry.get("id").and_then(Value::as_str) == Some(material.id()));
    if !exists {
        entries.push(material.as_json());
    }
    Ok(())
}

/// 根据缩短/延长策略调整片段的 source/target 时间范围。
fn process_timerange(
    segments: &mut [Value],
    segment_index: usize,
    source_timerange: &mut TimeRange,
    shrink: ShrinkMode,
    extend_modes: &[ExtendMode],
) -> Result<(), TemplateError> {
    let segment = &segments[segment_index];
    let mut target = parse_timerange(&segment["target_timerange"])?;
    let new_duration = source_timerange.duration;

    if new_duration < target.duration {
        let delta = target.duration - new_duration;
        match shrink {
            ShrinkMode::CutHead => {
                target.start += delta;
                target.duration -= delta;
            }
            ShrinkMode::CutTail => {
                target.duration -= delta;
            }
            ShrinkMode::CutTailAlign => {
                target.duration -= delta;
                for next in segments.iter_mut().skip(segment_index + 1) {
                    let mut next_range = parse_timerange(&next["target_timerange"])?;
                    next_range.start = next_range.start.saturating_sub(delta);
                    next["target_timerange"] = timerange_json(&next_range);
                }
            }
            ShrinkMode::Shrink => {
                target.duration -= delta;
                target.start += delta / 2;
            }
        }
    } else if new_duration > target.duration {
        let delta = new_duration - target.duration;
        let prev_end = if segment_index == 0 {
            0
        } else {
            parse_timerange(&segments[segment_index - 1]["target_timerange"])?.end()
        };
        let next_start = if segment_index + 1 >= segments.len() {
            u64::MAX / 4
        } else {
            parse_timerange(&segments[segment_index + 1]["target_timerange"])?.start
        };

        let mut success = false;
        for mode in extend_modes {
            match mode {
                ExtendMode::ExtendHead => {
                    if target.start >= delta && target.start - delta >= prev_end {
                        target.start -= delta;
                        target.duration += delta;
                        success = true;
                    }
                }
                ExtendMode::ExtendTail => {
                    if target.end() + delta <= next_start {
                        target.duration += delta;
                        success = true;
                    }
                }
                ExtendMode::PushTail => {
                    let shift = target
                        .end()
                        .saturating_add(delta)
                        .saturating_sub(next_start);
                    target.duration += delta;
                    if shift > 0 {
                        for next in segments.iter_mut().skip(segment_index + 1) {
                            let mut next_range = parse_timerange(&next["target_timerange"])?;
                            next_range.start += shift;
                            next["target_timerange"] = timerange_json(&next_range);
                        }
                    }
                    success = true;
                }
                ExtendMode::CutMaterialTail => {
                    source_timerange.duration = target.duration;
                    success = true;
                }
            }

            if success {
                break;
            }
        }

        if !success {
            return Err(TemplateError::ExtensionFailed);
        }
    }

    segments[segment_index]["target_timerange"] = timerange_json(&target);
    Ok(())
}

/// 解析草稿 JSON 中的 timerange 结构。
fn parse_timerange(value: &Value) -> Result<TimeRange, TemplateError> {
    let start = value
        .get("start")
        .and_then(Value::as_u64)
        .ok_or_else(|| TemplateError::UnsupportedStructure("missing timerange.start".into()))?;
    let duration = value
        .get("duration")
        .and_then(Value::as_u64)
        .ok_or_else(|| TemplateError::UnsupportedStructure("missing timerange.duration".into()))?;
    Ok(TimeRange::new(start, duration))
}

/// 将 `TimeRange` 转成草稿 JSON 结构。
fn timerange_json(timerange: &TimeRange) -> Value {
    json!({
        "start": timerange.start,
        "duration": timerange.duration,
    })
}

/// 递归复制整个草稿目录。
fn copy_dir_all(src: &Utf8Path, dst: &Utf8Path) -> Result<(), TemplateError> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let entry_path = Utf8PathBuf::from_path_buf(entry.path()).map_err(|pb| {
            TemplateError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("non-utf8 path: {}", pb.display()),
            ))
        })?;
        let target_path = dst.join(entry.file_name().to_string_lossy().as_ref());
        if file_type.is_dir() {
            copy_dir_all(&entry_path, &target_path)?;
        } else {
            std::fs::copy(&entry_path, &target_path)?;
        }
    }
    Ok(())
}
