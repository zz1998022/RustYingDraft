use anyhow::Result;
use camino::Utf8Path;
use jy_template::TemplateDraft;

/// 输出草稿中的基础结构信息和模板可替换资源信息。
///
/// 这个命令很适合在做模板二开前先摸清楚某个草稿里有哪些轨道、素材和可复用效果。
pub fn run(draft: &Utf8Path) -> Result<()> {
    let content = std::fs::read_to_string(draft)?;
    let json: serde_json::Value = serde_json::from_str(&content)?;

    // 先打印草稿的基础信息。
    if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
        println!("Draft name: {}", name);
    }
    if let Some(duration) = json.get("duration").and_then(|v| v.as_u64()) {
        println!("Duration: {:.3}s", duration as f64 / 1_000_000.0);
    }
    if let Some(fps) = json.get("fps").and_then(|v| v.as_f64()) {
        println!("FPS: {}", fps);
    }

    // 再列出轨道信息，方便快速确认结构。
    if let Some(tracks) = json.get("tracks").and_then(|v| v.as_array()) {
        println!("\nTracks ({}):", tracks.len());
        for track in tracks {
            let track_type = track.get("type").and_then(|v| v.as_str()).unwrap_or("?");
            let name = track.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let segments = track
                .get("segments")
                .and_then(|v| v.as_array())
                .map(|s| s.len())
                .unwrap_or(0);
            println!("  {} ({}) - {} segments", track_type, name, segments);
        }
    }

    // 最后列出基础素材数量。
    if let Some(materials) = json.get("materials") {
        let videos = materials
            .get("videos")
            .and_then(|v| v.as_array())
            .map(|s| s.len())
            .unwrap_or(0);
        let audios = materials
            .get("audios")
            .and_then(|v| v.as_array())
            .map(|s| s.len())
            .unwrap_or(0);
        let texts = materials
            .get("texts")
            .and_then(|v| v.as_array())
            .map(|s| s.len())
            .unwrap_or(0);
        println!(
            "\nMaterials: {} videos, {} audios, {} texts",
            videos, audios, texts
        );

        // 贴纸素材单独列一下，因为模板场景里常需要手工确认 resource_id。
        let empty_vec = Vec::new();
        let stickers = materials
            .get("stickers")
            .and_then(|v| v.as_array())
            .unwrap_or(&empty_vec);
        if !stickers.is_empty() {
            println!("\nStickers:");
            for s in stickers {
                let rid = s.get("resource_id").and_then(|v| v.as_str()).unwrap_or("?");
                println!("  Resource id: {}", rid);
            }
        }
    }

    // 使用模板层做更深一层的资源检查，比如文字气泡和文字特效。
    let template = TemplateDraft::load(draft)?;
    let inspection = template.inspect_material();
    if !inspection.stickers.is_empty() {
        println!("\nTemplate stickers:");
        for sticker in inspection.stickers {
            if let Some(name) = sticker.name {
                println!("  Resource id: {} '{}'", sticker.resource_id, name);
            } else {
                println!("  Resource id: {}", sticker.resource_id);
            }
        }
    }
    if !inspection.text_bubbles.is_empty() {
        println!("\nText bubbles:");
        for bubble in inspection.text_bubbles {
            if let Some(name) = bubble.name {
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
    if !inspection.text_effects.is_empty() {
        println!("\nText effects:");
        for effect in inspection.text_effects {
            if let Some(name) = effect.name {
                println!("  Resource id: {} '{}'", effect.resource_id, name);
            } else {
                println!("  Resource id: {}", effect.resource_id);
            }
        }
    }

    Ok(())
}
