use jy_schema::error::SchemaError;
use jy_schema::{
    AudioClip, AudioMaterialRef, Clip, ImageClip, Speed, TextClip, TextStyle, TimeRange, Transform,
    VideoClip, VideoMaterialRef,
};
use uuid::Uuid;

/// 统一生成无连字符 UUID，供片段/速度等内部对象使用。
fn new_id() -> String {
    Uuid::new_v4().as_simple().to_string()
}

/// 创建视频片段，并自动推导时间信息。
///
/// 时间逻辑尽量对齐 Python 版 `VideoSegment`：
///
/// - 同时给 `source` 和 `speed`：
///   - 目标时长 = 源时长 / 播放速度
/// - 只给 `source`：
///   - 自动反推播放速度
/// - 两者都不给：
///   - 默认速度为 1.0
///   - 从素材开头截取与目标时长相同的一段
pub fn make_video_clip(
    material: &VideoMaterialRef,
    target: TimeRange,
    source: Option<TimeRange>,
    speed: Option<f64>,
    volume: f64,
    transform: Option<Transform>,
) -> Result<Clip, SchemaError> {
    let (final_target, final_source, final_speed) =
        compute_time(target, source, speed, material.duration)?;

    Ok(Clip::Video(VideoClip {
        id: new_id(),
        material_id: material.id.clone(),
        target_timerange: final_target,
        source_timerange: Some(final_source),
        speed: Speed {
            id: new_id(),
            speed: final_speed,
        },
        volume,
        change_pitch: false,
        transform: transform.unwrap_or_default(),
        keyframes: Vec::new(),
        fade: None,
        effects: Vec::new(),
        filters: Vec::new(),
        mask: None,
        transition: None,
        background_filling: None,
        animations: None,
        mix_mode: None,
    }))
}

/// 创建音频片段，并自动推导时间信息。
pub fn make_audio_clip(
    material: &AudioMaterialRef,
    target: TimeRange,
    source: Option<TimeRange>,
    speed: Option<f64>,
    volume: f64,
) -> Result<Clip, SchemaError> {
    let (final_target, final_source, final_speed) =
        compute_time(target, source, speed, material.duration)?;

    Ok(Clip::Audio(AudioClip {
        id: new_id(),
        material_id: material.id.clone(),
        target_timerange: final_target,
        source_timerange: Some(final_source),
        speed: Speed {
            id: new_id(),
            speed: final_speed,
        },
        volume,
        change_pitch: false,
        keyframes: Vec::new(),
        fade: None,
        effects: Vec::new(),
    }))
}

/// 创建文本片段。
///
/// 文本片段本身不依赖外部素材文件，因此 `material_id` 直接生成一个新的内部 ID。
pub fn make_text_clip(
    text: &str,
    target: TimeRange,
    style: Option<TextStyle>,
    transform: Option<Transform>,
) -> Clip {
    Clip::Text(TextClip {
        id: new_id(),
        material_id: new_id(),
        target_timerange: target,
        text: text.to_string(),
        font: None,
        style: style.unwrap_or_default(),
        transform: transform.unwrap_or_default(),
        keyframes: Vec::new(),
        border: None,
        background: None,
        shadow: None,
        animations: None,
        bubble: None,
        effect: None,
    })
}

/// 创建图片片段，通常用于水印或静态覆盖层。
pub fn make_image_clip(
    material: &VideoMaterialRef,
    target: TimeRange,
    transform: Option<Transform>,
) -> Clip {
    Clip::Image(ImageClip {
        id: new_id(),
        material_id: material.id.clone(),
        target_timerange: target,
        source_timerange: Some(TimeRange::new(0, target.duration)),
        speed: Speed {
            id: new_id(),
            speed: 1.0,
        },
        transform: transform.unwrap_or_default(),
        keyframes: Vec::new(),
        background_filling: None,
        animations: None,
    })
}

/// 统一处理音视频片段的时间推导逻辑。
///
/// 这是时间轴层最重要的公共逻辑之一。
fn compute_time(
    target: TimeRange,
    source: Option<TimeRange>,
    speed: Option<f64>,
    material_duration: u64,
) -> Result<(TimeRange, TimeRange, f64), SchemaError> {
    match (source, speed) {
        (Some(src), Some(sp)) => {
            let duration = (src.duration as f64 / sp) as u64;
            let final_target = TimeRange::new(target.start, duration);
            validate_source(&src, material_duration)?;
            Ok((final_target, src, sp))
        }
        (Some(src), None) => {
            let sp = src.duration as f64 / target.duration as f64;
            validate_source(&src, material_duration)?;
            Ok((target, src, sp))
        }
        (None, Some(sp)) => {
            let src = TimeRange::new(0, (target.duration as f64 * sp) as u64);
            validate_source(&src, material_duration)?;
            Ok((target, src, sp))
        }
        (None, None) => {
            let src = TimeRange::new(0, target.duration);
            Ok((target, src, 1.0))
        }
    }
}

/// 校验源时间范围是否超出素材真实时长。
fn validate_source(source: &TimeRange, material_duration: u64) -> Result<(), SchemaError> {
    if source.end() > material_duration {
        Err(SchemaError::SourceRangeExceedsDuration {
            source_end: source.end(),
            material_duration,
        })
    } else {
        Ok(())
    }
}
