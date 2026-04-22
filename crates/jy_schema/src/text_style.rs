/// Text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TextAlign {
    Left = 0,
    Center = 1,
    Right = 2,
}

impl Default for TextAlign {
    fn default() -> Self {
        Self::Left
    }
}

/// Text font style settings.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TextStyle {
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: (f64, f64, f64),
    pub alpha: f64,
    pub align: TextAlign,
    pub vertical: bool,
    pub letter_spacing: i32,
    pub line_spacing: i32,
    pub auto_wrapping: bool,
    pub max_line_width: f64,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            size: 8.0,
            bold: false,
            italic: false,
            underline: false,
            color: (1.0, 1.0, 1.0),
            alpha: 1.0,
            align: TextAlign::default(),
            vertical: false,
            letter_spacing: 0,
            line_spacing: 0,
            auto_wrapping: false,
            max_line_width: 0.82,
        }
    }
}

/// Text stroke/border settings.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TextBorder {
    pub alpha: f64,
    pub color: (f64, f64, f64),
    /// Internal width value (0.0~0.2). User-facing width is 0~100, mapped as `width / 100.0 * 0.2`.
    pub width: f64,
}

impl Default for TextBorder {
    fn default() -> Self {
        Self {
            alpha: 1.0,
            color: (0.0, 0.0, 0.0),
            width: 0.08, // corresponds to user-facing width of 40
        }
    }
}

/// Text background settings.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TextBackground {
    pub style: u8,
    pub alpha: f64,
    /// Color in "#RRGGBB" format.
    pub color: String,
    pub round_radius: f64,
    pub height: f64,
    pub width: f64,
    pub horizontal_offset: f64,
    pub vertical_offset: f64,
}

impl Default for TextBackground {
    fn default() -> Self {
        Self {
            style: 1,
            alpha: 1.0,
            color: "#000000".into(),
            round_radius: 0.0,
            height: 0.14,
            width: 0.14,
            horizontal_offset: 0.0,
            vertical_offset: 0.0,
        }
    }
}

/// Text shadow settings.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TextShadow {
    pub alpha: f64,
    pub color: (f64, f64, f64),
    /// Spread (0~100 user-facing).
    pub diffuse: f64,
    /// Distance (0~100).
    pub distance: f64,
    /// Angle (-180 to 180 degrees).
    pub angle: f64,
}

impl Default for TextShadow {
    fn default() -> Self {
        Self {
            alpha: 1.0,
            color: (0.0, 0.0, 0.0),
            diffuse: 15.0,
            distance: 5.0,
            angle: -45.0,
        }
    }
}
