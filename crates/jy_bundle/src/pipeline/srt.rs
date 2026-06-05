use anyhow::{bail, Context, Result};
use camino::Utf8Path;

#[derive(Debug, Clone)]
pub(crate) struct SrtCue {
    pub(crate) start: f64,
    pub(crate) end: f64,
    pub(crate) text: String,
}

pub(crate) fn parse_srt_file(path: &Utf8Path) -> Result<Vec<SrtCue>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read subtitle file: {path}"))?;
    parse_srt_content(&content)
}

fn parse_srt_content(content: &str) -> Result<Vec<SrtCue>> {
    let normalized = content
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut cues = Vec::new();

    for (block_index, block) in normalized.split("\n\n").enumerate() {
        let lines = block
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>();
        if lines.is_empty() {
            continue;
        }

        let time_line_index = if lines[0].contains("-->") { 0 } else { 1 };
        if time_line_index >= lines.len() {
            bail!("srt block {} is missing time range", block_index + 1);
        }
        let text_start_index = time_line_index + 1;
        if text_start_index >= lines.len() {
            bail!("srt block {} is missing text", block_index + 1);
        }

        let (start, end) = parse_srt_time_range(lines[time_line_index])
            .with_context(|| format!("invalid srt block {} time range", block_index + 1))?;
        if end <= start {
            bail!(
                "srt block {} end must be greater than start",
                block_index + 1
            );
        }

        let text = lines[text_start_index..].join("\n");
        cues.push(SrtCue { start, end, text });
    }

    if cues.is_empty() {
        bail!("srt file must contain at least one subtitle cue");
    }
    Ok(cues)
}

fn parse_srt_time_range(line: &str) -> Result<(f64, f64)> {
    let Some((start, end)) = line.split_once("-->") else {
        bail!("srt time range must contain -->");
    };
    Ok((parse_srt_time(start.trim())?, parse_srt_time(end.trim())?))
}

fn parse_srt_time(value: &str) -> Result<f64> {
    let Some((hour_text, rest)) = value.split_once(':') else {
        bail!("invalid srt time: {value}");
    };
    let Some((minute_text, rest)) = rest.split_once(':') else {
        bail!("invalid srt time: {value}");
    };
    let Some((second_text, millis_text)) = rest.split_once(',').or_else(|| rest.split_once('.'))
    else {
        bail!("invalid srt time: {value}");
    };

    let hours = hour_text
        .parse::<u64>()
        .with_context(|| format!("invalid srt hour: {value}"))?;
    let minutes = minute_text
        .parse::<u64>()
        .with_context(|| format!("invalid srt minute: {value}"))?;
    let seconds = second_text
        .parse::<u64>()
        .with_context(|| format!("invalid srt second: {value}"))?;
    let millis = millis_text
        .parse::<u64>()
        .with_context(|| format!("invalid srt millis: {value}"))?;
    if minutes >= 60 || seconds >= 60 || millis >= 1000 {
        bail!("invalid srt time component: {value}");
    }

    Ok((hours * 3600 + minutes * 60 + seconds) as f64 + millis as f64 / 1000.0)
}
