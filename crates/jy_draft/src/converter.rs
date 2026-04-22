use crate::error::DraftError;
use crate::templates::load_content_template;
use jy_schema::{
    AudioClip, AudioMaterialRef, Clip, ImageClip, Project, Speed, TextClip, TimeRange, Track,
    VideoClip, VideoMaterialRef,
};
use serde_json::{json, Value};
use uuid::Uuid;

/// 将统一的 `Project` 领域模型转换为完整的 `draft_content.json`。
///
/// 可以把这个文件理解为：
/// - 上游：`jy_schema / jy_timeline`
/// - 下游：剪映草稿 JSON
///
/// 如果后续出现“草稿能生成但剪映里效果不对”的问题，绝大多数都应该先从这里排查。
pub fn project_to_draft(project: &Project) -> Result<Value, DraftError> {
    let mut draft = load_content_template()?;

    // 先写顶层基础字段。
    draft["duration"] = json!(project.duration);
    draft["fps"] = json!(project.canvas.fps as f64);
    draft["id"] = json!(format!(
        "{{{}}}",
        Uuid::new_v4().as_hyphenated().to_string().to_uppercase()
    ));

    // 写入画布配置。
    draft["canvas_config"] = json!({
        "height": project.canvas.height,
        "ratio": "original",
        "width": project.canvas.width,
    });

    // 写入草稿配置项。
    draft["config"]["maintrack_adsorb"] = json!(project.maintrack_adsorb);

    // 先准备各类素材容器。
    let mut videos: Vec<Value> = Vec::new();
    let mut audios: Vec<Value> = Vec::new();
    let mut texts: Vec<Value> = Vec::new();
    let mut speeds: Vec<Value> = Vec::new();
    let mut audio_effects: Vec<Value> = Vec::new();
    let mut audio_fades: Vec<Value> = Vec::new();
    let mut video_effects: Vec<Value> = Vec::new();
    let mut effects: Vec<Value> = Vec::new(); // filters + mix_modes
    let mut masks: Vec<Value> = Vec::new();
    let mut transitions: Vec<Value> = Vec::new();
    let mut canvases: Vec<Value> = Vec::new();
    let mut animations: Vec<Value> = Vec::new();

    // 注册显式素材列表。
    for mat in &project.video_materials {
        videos.push(video_material_to_json(mat));
    }
    for mat in &project.audio_materials {
        audios.push(audio_material_to_json(mat));
    }

    // 再从所有片段中收集“附属素材”，例如变速、淡入淡出、转场、动画、文字样式等。
    for track in &project.tracks {
        for clip in &track.clips {
            match clip {
                Clip::Video(vc) => {
                    speeds.push(speed_to_json(&vc.speed));
                    if let Some(fade) = &vc.fade {
                        audio_fades.push(audio_fade_to_json(fade));
                    }
                    for eff in &vc.effects {
                        video_effects.push(effect_ref_to_json(eff));
                    }
                    for filt in &vc.filters {
                        effects.push(filter_ref_to_json(filt));
                    }
                    if let Some(mm) = &vc.mix_mode {
                        effects.push(mix_mode_to_json(mm));
                    }
                    if let Some(mask) = &vc.mask {
                        masks.push(mask_to_json(mask));
                    }
                    if let Some(tr) = &vc.transition {
                        transitions.push(transition_to_json(tr));
                    }
                    if let Some(bg) = &vc.background_filling {
                        canvases.push(background_filling_to_json(bg));
                    }
                    if let Some(anim) = &vc.animations {
                        animations.push(segment_animations_to_json(anim));
                    }
                }
                Clip::Audio(ac) => {
                    speeds.push(speed_to_json(&ac.speed));
                    if let Some(fade) = &ac.fade {
                        audio_fades.push(audio_fade_to_json(fade));
                    }
                    for eff in &ac.effects {
                        audio_effects.push(audio_effect_to_json(eff));
                    }
                }
                Clip::Text(tc) => {
                    texts.push(text_material_to_json(tc));
                    if let Some(anim) = &tc.animations {
                        animations.push(segment_animations_to_json(anim));
                    }
                    if let Some(bubble) = &tc.bubble {
                        effects.push(json!({
                            "type": "text_shape",
                            "id": bubble.id,
                            "effect_id": bubble.effect_id,
                            "resource_id": bubble.resource_id,
                            "value": 1.0,
                            "apply_target_type": 0,
                        }));
                    }
                    if let Some(eff) = &tc.effect {
                        effects.push(json!({
                            "type": "text_effect",
                            "id": eff.id,
                            "effect_id": eff.effect_id,
                            "resource_id": eff.resource_id,
                            "value": 1.0,
                            "apply_target_type": 0,
                            "source_platform": 1,
                        }));
                    }
                }
                Clip::Image(ic) => {
                    speeds.push(speed_to_json(&ic.speed));
                    if let Some(bg) = &ic.background_filling {
                        canvases.push(background_filling_to_json(bg));
                    }
                    if let Some(anim) = &ic.animations {
                        animations.push(segment_animations_to_json(anim));
                    }
                }
            }
        }
    }

    // 把所有素材区域回写到草稿 JSON。
    let mats = &mut draft["materials"];
    mats["videos"] = json!(videos);
    mats["audios"] = json!(audios);
    mats["texts"] = json!(texts);
    mats["speeds"] = json!(speeds);
    mats["audio_effects"] = json!(audio_effects);
    mats["audio_fades"] = json!(audio_fades);
    mats["video_effects"] = json!(video_effects);
    mats["effects"] = json!(effects);
    mats["masks"] = json!(masks);
    mats["transitions"] = json!(transitions);
    mats["canvases"] = json!(canvases);
    mats["material_animations"] = json!(animations);

    // 最后构建轨道。
    let mut sorted_tracks: Vec<&Track> = project.tracks.iter().collect();
    sorted_tracks.sort_by_key(|t| t.render_index);

    let track_jsons: Vec<Value> = sorted_tracks.iter().map(|t| track_to_json(t)).collect();
    draft["tracks"] = json!(track_jsons);

    Ok(draft)
}

// ---------------------------------------------------------------------------
// 各类素材的 JSON 转换
// ---------------------------------------------------------------------------

/// 视频/图片素材转换。
fn video_material_to_json(mat: &VideoMaterialRef) -> Value {
    json!({
        "audio_fade": Value::Null,
        "category_id": "",
        "category_name": "local",
        "check_flag": 63487,
        "crop": crop_to_json(&mat.crop),
        "crop_ratio": "free",
        "crop_scale": 1.0,
        "duration": mat.duration,
        "height": mat.height,
        "id": mat.id,
        "local_material_id": "",
        "material_id": mat.id,
        "material_name": mat.name,
        "media_path": "",
        "path": path_to_jy(&mat.path),
        "type": match mat.kind {
            jy_schema::MaterialKind::Video => "video",
            jy_schema::MaterialKind::Photo => "photo",
            jy_schema::MaterialKind::Audio => "video",
        },
        "width": mat.width,
    })
}

/// 音频素材转换。
fn audio_material_to_json(mat: &AudioMaterialRef) -> Value {
    json!({
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
        "path": path_to_jy(&mat.path),
        "source_platform": 0,
        "type": "extract_music",
        "wave_points": [],
    })
}

/// 裁剪配置转换。
fn crop_to_json(crop: &jy_schema::CropSettings) -> Value {
    json!({
        "upper_left_x": crop.upper_left_x,
        "upper_left_y": crop.upper_left_y,
        "upper_right_x": crop.upper_right_x,
        "upper_right_y": crop.upper_right_y,
        "lower_left_x": crop.lower_left_x,
        "lower_left_y": crop.lower_left_y,
        "lower_right_x": crop.lower_right_x,
        "lower_right_y": crop.lower_right_y,
    })
}

/// 将路径转换为剪映偏好的格式。
///
/// 剪映在 Windows 下也习惯使用正斜杠路径，因此这里统一做一层转换。
fn path_to_jy(path: &camino::Utf8Path) -> String {
    let s = path.as_str();
    if cfg!(windows) {
        s.replace('\\', "/")
    } else {
        s.to_string()
    }
}

/// 变速素材转换。
fn speed_to_json(speed: &Speed) -> Value {
    json!({
        "curve_speed": Value::Null,
        "id": speed.id,
        "mode": 0,
        "speed": speed.speed,
        "type": "speed",
    })
}

/// 音频淡入淡出转换。
fn audio_fade_to_json(fade: &jy_schema::AudioFade) -> Value {
    json!({
        "id": fade.id,
        "fade_in_duration": fade.in_duration,
        "fade_out_duration": fade.out_duration,
        "fade_type": 0,
        "type": "audio_fade",
    })
}

/// 音频特效转换。
fn audio_effect_to_json(effect: &jy_schema::AudioEffectRef) -> Value {
    json!( {
        "audio_adjust_params": [],
        "category_id": effect.category_id,
        "category_name": effect.category_name,
        "id": effect.id,
        "is_ugc": false,
        "name": effect.name,
        "production_path": "",
        "resource_id": effect.resource_id,
        "speaker_id": "",
        "sub_type": effect.category_index,
        "time_range": {
            "duration": 0,
            "start": 0
        },
        "type": "audio_effect"
    })
}

/// 视频特效引用转换。
fn effect_ref_to_json(eff: &jy_schema::EffectRef) -> Value {
    json!({
        "id": eff.id,
        "effect_id": eff.effect_id,
        "resource_id": eff.resource_id,
        "type": "video_effect",
        "value": 1.0,
        "apply_target_type": 0,
    })
}

/// 滤镜引用转换。
fn filter_ref_to_json(filt: &jy_schema::FilterRef) -> Value {
    json!({
        "id": filt.id,
        "effect_id": filt.effect_id,
        "resource_id": filt.resource_id,
        "type": "filter",
        "value": filt.intensity,
        "apply_target_type": 0,
    })
}

/// 混合模式转换。
fn mix_mode_to_json(mm: &jy_schema::MixModeRef) -> Value {
    json!({
        "type": "mix_mode",
        "name": mm.name,
        "effect_id": mm.effect_id,
        "resource_id": mm.resource_id,
        "value": 1.0,
        "apply_target_type": 0,
        "platform": "all",
        "source_platform": 0,
        "category_id": "",
        "category_name": "",
        "sub_type": "none",
        "time_range": Value::Null,
        "id": mm.id,
    })
}

/// 蒙版转换。
fn mask_to_json(mask: &jy_schema::MaskRef) -> Value {
    json!( {
        "config": {
            "aspectRatio": mask.aspect_ratio,
            "centerX": mask.center_x,
            "centerY": mask.center_y,
            "feather": mask.feather,
            "height": mask.height,
            "invert": mask.invert,
            "rotation": mask.rotation,
            "roundCorner": mask.round_corner,
            "width": mask.width
        },
        "id": mask.id,
        "name": mask.name,
        "platform": "all",
        "position_info": mask.position_info,
        "resource_type": mask.resource_type,
        "resource_id": mask.resource_id,
        "type": "mask"
    })
}

/// 转场转换。
fn transition_to_json(transition: &jy_schema::TransitionRef) -> Value {
    json!( {
        "category_id": "",
        "category_name": "",
        "duration": transition.duration,
        "effect_id": transition.effect_id,
        "id": transition.id,
        "is_overlap": transition.is_overlap,
        "name": transition.name,
        "platform": "all",
        "resource_id": transition.resource_id,
        "type": "transition"
    })
}

/// 背景填充转换。
fn background_filling_to_json(bg: &jy_schema::BackgroundFillingRef) -> Value {
    json!({
        "id": bg.id,
        "type": match bg.fill_type {
            jy_schema::BackgroundFillType::Blur => "canvas_blur",
            jy_schema::BackgroundFillType::Color => "canvas_color",
        },
        "blur": bg.blur,
        "color": bg.color,
        "source_platform": 0,
    })
}

/// 动画集合转换。
fn segment_animations_to_json(animation_ref: &jy_schema::AnimationRef) -> Value {
    let animations: Vec<Value> = animation_ref
        .animations
        .iter()
        .map(|animation| {
            json!( {
                "anim_adjust_params": Value::Null,
                "platform": "all",
                "panel": if animation.is_video_animation { "video" } else { "" },
                "material_type": if animation.is_video_animation { "video" } else { "sticker" },
                "name": animation.name,
                "id": animation.effect_id,
                "type": animation.animation_type,
                "resource_id": animation.resource_id,
                "start": animation.start,
                "duration": animation.duration
            })
        })
        .collect();

    json!( {
        "id": animation_ref.id,
        "type": "sticker_animation",
        "multi_language_current": "none",
        "animations": animations
    })
}

// ---------------------------------------------------------------------------
// 文本素材转换（最复杂的一部分）
// ---------------------------------------------------------------------------

/// 文本片段转换为 `materials.texts` 中的文本素材对象。
///
/// 这里之所以复杂，是因为剪映文本素材的 `content` 字段本身还是一段序列化后的 JSON 字符串。
fn text_material_to_json(tc: &TextClip) -> Value {
    let text_len = tc.text.chars().count();
    let mut check_flag: u32 = 7;
    let mut strokes = Vec::new();
    if let Some(border) = &tc.border {
        check_flag |= 8;
        strokes.push(json!({
            "content": {
                "solid": {
                    "alpha": border.alpha,
                    "color": [border.color.0, border.color.1, border.color.2]
                }
            },
            "width": border.width
        }));
    }

    let mut bg_json = json!({});
    if let Some(bg) = &tc.background {
        check_flag |= 16;
        bg_json = json!({
            "background_style": bg.style,
            "background_color": bg.color,
            "background_alpha": bg.alpha,
            "background_round_radius": bg.round_radius,
            "background_height": bg.height,
            "background_width": bg.width,
            "background_horizontal_offset": bg.horizontal_offset,
            "background_vertical_offset": bg.vertical_offset,
        });
    }

    let mut shadow_json = Vec::new();
    if let Some(shadow) = &tc.shadow {
        check_flag |= 32;
        shadow_json.push(json!({
            "diffuse": shadow.diffuse / 100.0 / 6.0,
            "alpha": shadow.alpha,
            "distance": shadow.distance,
            "content": {
                "solid": {
                    "color": [shadow.color.0, shadow.color.1, shadow.color.2]
                }
            },
            "angle": shadow.angle,
        }));
    }

    let mut style_obj = json!({
        "fill": {
            "alpha": 1.0,
            "content": {
                "render_type": "solid",
                "solid": {
                    "alpha": 1.0,
                    "color": [tc.style.color.0, tc.style.color.1, tc.style.color.2]
                }
            }
        },
        "range": [0, text_len],
        "size": tc.style.size,
        "bold": tc.style.bold,
        "italic": tc.style.italic,
        "underline": tc.style.underline,
        "strokes": strokes,
    });

    if let Some(font) = &tc.font {
        style_obj["font"] = json!({
            "id": font.resource_id,
            "path": "D:"
        });
    }

    if let Some(eff) = &tc.effect {
        style_obj["effectStyle"] = json!({
            "id": eff.effect_id,
            "path": "C:"
        });
    }

    if !shadow_json.is_empty() {
        style_obj["shadows"] = json!(shadow_json);
    }

    let content = json!({
        "styles": [style_obj],
        "text": tc.text,
    });

    let content_str = serde_json::to_string(&content).unwrap_or_default();

    let mut mat = json!({
        "id": tc.material_id,
        "content": content_str,
        "typesetting": tc.style.vertical as u32,
        "alignment": tc.style.align as u32,
        "letter_spacing": (tc.style.letter_spacing as f64) * 0.05,
        "line_spacing": 0.02 + (tc.style.line_spacing as f64) * 0.05,
        "line_feed": 1,
        "line_max_width": tc.style.max_line_width,
        "force_apply_line_max_width": false,
        "check_flag": check_flag,
        "type": if tc.style.auto_wrapping { "subtitle" } else { "text" },
        "global_alpha": tc.style.alpha,
    });

    // 背景字段在剪映里直接平铺在文本素材对象上，这里做一次合并。
    if tc.background.is_some() {
        mat.as_object_mut()
            .unwrap()
            .extend(bg_json.as_object().unwrap().clone());
    }

    mat
}

// ---------------------------------------------------------------------------
// 轨道和片段转换
// ---------------------------------------------------------------------------

/// 轨道转换。
fn track_to_json(track: &Track) -> Value {
    let segments: Vec<Value> = track
        .clips
        .iter()
        .map(|c| clip_to_segment_json(c, track.render_index))
        .collect();

    json!({
        "attribute": if track.mute { 1 } else { 0 },
        "flag": 0,
        "id": track.id,
        "is_default_name": track.name.is_empty(),
        "name": track.name,
        "segments": segments,
        "type": track.kind.to_str(),
    })
}

fn clip_to_segment_json(clip: &Clip, render_index: i32) -> Value {
    match clip {
        Clip::Video(vc) => video_segment_json(vc, render_index),
        Clip::Audio(ac) => audio_segment_json(ac, render_index),
        Clip::Text(tc) => text_segment_json(tc, render_index),
        Clip::Image(ic) => image_segment_json(ic, render_index),
    }
}

fn base_segment_json(id: &str, material_id: &str, target: &TimeRange) -> Value {
    json!({
        "enable_adjust": true,
        "enable_color_correct_adjust": false,
        "enable_color_curves": true,
        "enable_color_match_adjust": false,
        "enable_color_wheels": true,
        "enable_lut": true,
        "enable_smart_color_adjust": false,
        "last_nonzero_volume": 1.0,
        "reverse": false,
        "track_attribute": 0,
        "track_render_index": 0,
        "visible": true,
        "id": id,
        "material_id": material_id,
        "target_timerange": timerange_to_json(target),
        "common_keyframes": [],
        "keyframe_refs": [],
    })
}

fn video_segment_json(vc: &VideoClip, render_index: i32) -> Value {
    let mut seg = base_segment_json(&vc.id, &vc.material_id, &vc.target_timerange);

    // MediaSegment fields
    let mut extra_refs = vec![vc.speed.id.clone()];
    if let Some(fade) = &vc.fade {
        extra_refs.push(fade.id.clone());
    }
    for eff in &vc.effects {
        extra_refs.push(eff.id.clone());
    }
    for filt in &vc.filters {
        extra_refs.push(filt.id.clone());
    }
    if let Some(mm) = &vc.mix_mode {
        extra_refs.push(mm.id.clone());
    }
    if let Some(mask) = &vc.mask {
        extra_refs.push(mask.id.clone());
    }
    if let Some(tr) = &vc.transition {
        extra_refs.push(tr.id.clone());
    }
    if let Some(bg) = &vc.background_filling {
        extra_refs.push(bg.id.clone());
    }
    if let Some(anim) = &vc.animations {
        extra_refs.push(anim.id.clone());
    }

    let obj = seg.as_object_mut().unwrap();
    obj.insert(
        "source_timerange".into(),
        timerange_to_json_opt(vc.source_timerange.as_ref()),
    );
    obj.insert("speed".into(), json!(vc.speed.speed));
    obj.insert("volume".into(), json!(vc.volume));
    obj.insert("extra_material_refs".into(), json!(extra_refs));
    obj.insert("is_tone_modify".into(), json!(vc.change_pitch));

    // VisualSegment fields
    obj.insert("clip".into(), transform_to_clip_json(&vc.transform));
    obj.insert(
        "uniform_scale".into(),
        json!({
            "on": vc.transform.uniform_scale,
            "value": 1.0,
        }),
    );

    // VideoSegment fields
    obj.insert(
        "hdr_settings".into(),
        json!({
            "intensity": 1.0,
            "mode": 1,
            "nits": 1000,
        }),
    );
    obj.insert("render_index".into(), json!(render_index));

    seg
}

fn audio_segment_json(ac: &AudioClip, render_index: i32) -> Value {
    let mut seg = base_segment_json(&ac.id, &ac.material_id, &ac.target_timerange);

    let mut extra_refs = vec![ac.speed.id.clone()];
    if let Some(fade) = &ac.fade {
        extra_refs.push(fade.id.clone());
    }
    for eff in &ac.effects {
        extra_refs.push(eff.id.clone());
    }

    let obj = seg.as_object_mut().unwrap();
    obj.insert(
        "source_timerange".into(),
        timerange_to_json_opt(ac.source_timerange.as_ref()),
    );
    obj.insert("speed".into(), json!(ac.speed.speed));
    obj.insert("volume".into(), json!(ac.volume));
    obj.insert("extra_material_refs".into(), json!(extra_refs));
    obj.insert("is_tone_modify".into(), json!(ac.change_pitch));
    obj.insert("clip".into(), Value::Null);
    obj.insert("hdr_settings".into(), Value::Null);
    obj.insert("render_index".into(), json!(render_index));

    seg
}

fn text_segment_json(tc: &TextClip, render_index: i32) -> Value {
    let mut seg = base_segment_json(&tc.id, &tc.material_id, &tc.target_timerange);

    let mut extra_refs: Vec<String> = Vec::new();
    if let Some(anim) = &tc.animations {
        extra_refs.push(anim.id.clone());
    }
    if let Some(bubble) = &tc.bubble {
        extra_refs.push(bubble.id.clone());
    }
    if let Some(eff) = &tc.effect {
        extra_refs.push(eff.id.clone());
    }

    let obj = seg.as_object_mut().unwrap();
    obj.insert("source_timerange".into(), Value::Null);
    obj.insert("speed".into(), json!(1.0));
    obj.insert("volume".into(), json!(1.0));
    obj.insert("extra_material_refs".into(), json!(extra_refs));
    obj.insert("is_tone_modify".into(), json!(false));
    obj.insert("clip".into(), transform_to_clip_json(&tc.transform));
    obj.insert(
        "uniform_scale".into(),
        json!({
            "on": tc.transform.uniform_scale,
            "value": 1.0,
        }),
    );
    obj.insert(
        "hdr_settings".into(),
        json!({
            "intensity": 1.0,
            "mode": 1,
            "nits": 1000,
        }),
    );
    obj.insert("render_index".into(), json!(render_index));

    seg
}

fn image_segment_json(ic: &ImageClip, render_index: i32) -> Value {
    let mut seg = base_segment_json(&ic.id, &ic.material_id, &ic.target_timerange);

    let mut extra_refs = vec![ic.speed.id.clone()];
    if let Some(bg) = &ic.background_filling {
        extra_refs.push(bg.id.clone());
    }
    if let Some(anim) = &ic.animations {
        extra_refs.push(anim.id.clone());
    }

    let obj = seg.as_object_mut().unwrap();
    obj.insert(
        "source_timerange".into(),
        timerange_to_json_opt(ic.source_timerange.as_ref()),
    );
    obj.insert("speed".into(), json!(ic.speed.speed));
    obj.insert("volume".into(), json!(1.0));
    obj.insert("extra_material_refs".into(), json!(extra_refs));
    obj.insert("is_tone_modify".into(), json!(false));
    obj.insert("clip".into(), transform_to_clip_json(&ic.transform));
    obj.insert(
        "uniform_scale".into(),
        json!({
            "on": ic.transform.uniform_scale,
            "value": 1.0,
        }),
    );
    obj.insert(
        "hdr_settings".into(),
        json!({
            "intensity": 1.0,
            "mode": 1,
            "nits": 1000,
        }),
    );
    obj.insert("render_index".into(), json!(render_index));

    seg
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn timerange_to_json(tr: &TimeRange) -> Value {
    json!({
        "start": tr.start,
        "duration": tr.duration,
    })
}

fn timerange_to_json_opt(tr: Option<&TimeRange>) -> Value {
    match tr {
        Some(t) => timerange_to_json(t),
        None => Value::Null,
    }
}

fn transform_to_clip_json(t: &jy_schema::Transform) -> Value {
    json!({
        "alpha": t.opacity,
        "flip": {
            "horizontal": t.flip_h,
            "vertical": t.flip_v,
        },
        "rotation": t.rotation_deg,
        "scale": {
            "x": t.scale_x,
            "y": t.scale_y,
        },
        "transform": {
            "x": t.to_jy_transform_x(),
            "y": t.to_jy_transform_y(),
        },
    })
}
