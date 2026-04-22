use anyhow::Result;
use camino::Utf8Path;
use jy_template::TemplateDraft;
use serde::Serialize;
use serde_json::Value;

use crate::output;

/// `inspect` 的结构化输出。
///
/// 这份摘要既用于 JSON 模式下给后端返回稳定数据，也用于 text 模式下统一渲染，
/// 这样可以避免“人类输出”和“机器输出”走两套不同逻辑，后续更容易维护。
#[derive(Debug, Serialize)]
struct InspectSummary {
    draft_path: String,
    name: Option<String>,
    duration_us: Option<u64>,
    fps: Option<f64>,
    tracks: Vec<TrackSummary>,
    materials: MaterialSummary,
    stickers: Vec<NamedResourceSummary>,
    text_bubbles: Vec<TextBubbleSummary>,
    text_effects: Vec<NamedResourceSummary>,
}

#[derive(Debug, Serialize)]
struct TrackSummary {
    track_type: String,
    name: String,
    segment_count: usize,
}

#[derive(Debug, Serialize)]
struct MaterialSummary {
    videos: usize,
    audios: usize,
    texts: usize,
    stickers: usize,
}

#[derive(Debug, Serialize)]
struct NamedResourceSummary {
    resource_id: String,
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct TextBubbleSummary {
    effect_id: String,
    resource_id: String,
    name: Option<String>,
}

/// 输出草稿中的基础结构信息和模板可替换资源信息。
///
/// 这个命令很适合在做模板二开前先摸清楚某个草稿里有哪些轨道、素材和可复用效果。
pub fn run(draft: &Utf8Path) -> Result<()> {
    let summary = build_summary(draft)?;

    if output::is_json() {
        output::emit_result(
            "inspect",
            &format!("Inspected draft: {draft}"),
            &summary,
        );
        return Ok(());
    }

    render_text_summary(&summary);
    Ok(())
}

fn build_summary(draft: &Utf8Path) -> Result<InspectSummary> {
    let content = std::fs::read_to_string(draft)?;
    let json: Value = serde_json::from_str(&content)?;

    let tracks = json
        .get("tracks")
        .and_then(|v| v.as_array())
        .map(|tracks| {
            tracks
                .iter()
                .map(|track| TrackSummary {
                    track_type: track
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                        .to_string(),
                    name: track
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    segment_count: track
                        .get("segments")
                        .and_then(|v| v.as_array())
                        .map(|s| s.len())
                        .unwrap_or(0),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let materials = json.get("materials");
    let template = TemplateDraft::load(draft)?;
    let inspection = template.inspect_material();

    Ok(InspectSummary {
        draft_path: draft.as_str().to_string(),
        name: json
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        duration_us: json.get("duration").and_then(|v| v.as_u64()),
        fps: json.get("fps").and_then(|v| v.as_f64()),
        tracks,
        materials: MaterialSummary {
            videos: materials
                .and_then(|m| m.get("videos"))
                .and_then(|v| v.as_array())
                .map(|s| s.len())
                .unwrap_or(0),
            audios: materials
                .and_then(|m| m.get("audios"))
                .and_then(|v| v.as_array())
                .map(|s| s.len())
                .unwrap_or(0),
            texts: materials
                .and_then(|m| m.get("texts"))
                .and_then(|v| v.as_array())
                .map(|s| s.len())
                .unwrap_or(0),
            stickers: materials
                .and_then(|m| m.get("stickers"))
                .and_then(|v| v.as_array())
                .map(|s| s.len())
                .unwrap_or(0),
        },
        stickers: inspection
            .stickers
            .into_iter()
            .map(|item| NamedResourceSummary {
                resource_id: item.resource_id,
                name: item.name,
            })
            .collect(),
        text_bubbles: inspection
            .text_bubbles
            .into_iter()
            .map(|item| TextBubbleSummary {
                effect_id: item.effect_id,
                resource_id: item.resource_id,
                name: item.name,
            })
            .collect(),
        text_effects: inspection
            .text_effects
            .into_iter()
            .map(|item| NamedResourceSummary {
                resource_id: item.resource_id,
                name: item.name,
            })
            .collect(),
    })
}

/// text 模式仍然保留一份适合终端阅读的排版。
fn render_text_summary(summary: &InspectSummary) {
    if let Some(name) = &summary.name {
        println!("Draft name: {name}");
    }
    if let Some(duration) = summary.duration_us {
        println!("Duration: {:.3}s", duration as f64 / 1_000_000.0);
    }
    if let Some(fps) = summary.fps {
        println!("FPS: {fps}");
    }

    println!("\nTracks ({}):", summary.tracks.len());
    for track in &summary.tracks {
        println!(
            "  {} ({}) - {} segments",
            track.track_type, track.name, track.segment_count
        );
    }

    println!(
        "\nMaterials: {} videos, {} audios, {} texts",
        summary.materials.videos, summary.materials.audios, summary.materials.texts
    );

    if summary.materials.stickers > 0 {
        println!("\nStickers in draft JSON: {}", summary.materials.stickers);
    }

    if !summary.stickers.is_empty() {
        println!("\nTemplate stickers:");
        for sticker in &summary.stickers {
            if let Some(name) = &sticker.name {
                println!("  Resource id: {} '{}'", sticker.resource_id, name);
            } else {
                println!("  Resource id: {}", sticker.resource_id);
            }
        }
    }

    if !summary.text_bubbles.is_empty() {
        println!("\nText bubbles:");
        for bubble in &summary.text_bubbles {
            if let Some(name) = &bubble.name {
                println!(
                    "  Effect id: {} ,Resource id: {} '{}'",
                    bubble.effect_id, bubble.resource_id, name
                );
            } else {
                println!(
                    "  Effect id: {} ,Resource id: {}",
                    bubble.effect_id, bubble.resource_id
                );
            }
        }
    }

    if !summary.text_effects.is_empty() {
        println!("\nText effects:");
        for effect in &summary.text_effects {
            if let Some(name) = &effect.name {
                println!("  Resource id: {} '{}'", effect.resource_id, name);
            } else {
                println!("  Resource id: {}", effect.resource_id);
            }
        }
    }
}
