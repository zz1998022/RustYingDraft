use anyhow::{bail, Result};
use jy_schema::{TextAlign, TextStyle, Transform};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SimpleSubtitleStyle {
    #[serde(default = "default_simple_subtitle_font_size")]
    pub(crate) font_size: f64,
    #[serde(default = "default_simple_subtitle_x")]
    pub(crate) x: f64,
    #[serde(default = "default_simple_subtitle_y")]
    pub(crate) y: f64,
}

impl Default for SimpleSubtitleStyle {
    fn default() -> Self {
        Self {
            font_size: default_simple_subtitle_font_size(),
            x: default_simple_subtitle_x(),
            y: default_simple_subtitle_y(),
        }
    }
}

fn default_simple_subtitle_font_size() -> f64 {
    8.0
}

fn default_simple_subtitle_x() -> f64 {
    0.5
}

fn default_simple_subtitle_y() -> f64 {
    0.82
}

pub(crate) fn validate_simple_subtitle_style(style: &SimpleSubtitleStyle) -> Result<()> {
    if !style.font_size.is_finite() || style.font_size <= 0.0 {
        bail!("subtitle_style.font_size must be greater than 0");
    }
    if !is_normalized_position(style.x) {
        bail!("subtitle_style.x must be between 0.0 and 1.0");
    }
    if !is_normalized_position(style.y) {
        bail!("subtitle_style.y must be between 0.0 and 1.0");
    }
    Ok(())
}

pub(crate) fn build_simple_subtitle_style(style: &SimpleSubtitleStyle) -> Result<TextStyle> {
    validate_simple_subtitle_style(style)?;
    Ok(TextStyle {
        size: style.font_size,
        align: TextAlign::Center,
        auto_wrapping: true,
        ..Default::default()
    })
}

pub(crate) fn build_simple_subtitle_transform(style: &SimpleSubtitleStyle) -> Result<Transform> {
    validate_simple_subtitle_style(style)?;
    Ok(Transform {
        x: style.x,
        y: style.y,
        ..Default::default()
    })
}

fn is_normalized_position(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}
