/// 视觉变换信息。
///
/// 这里使用的是“归一化坐标系”：
///
/// - `x / y` 范围通常是 `0.0 ~ 1.0`
/// - `(0.5, 0.5)` 表示画布中心
/// - 真正写入剪映时，会再换算成“半个画布宽/高”的偏移量
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Transform {
    pub x: f64,
    pub y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub rotation_deg: f64,
    pub opacity: f64,
    pub flip_h: bool,
    pub flip_v: bool,
    pub uniform_scale: bool,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            x: 0.5,
            y: 0.5,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation_deg: 0.0,
            opacity: 1.0,
            flip_h: false,
            flip_v: false,
            uniform_scale: true,
        }
    }
}

impl Transform {
    /// 创建一个默认变换。
    pub fn new() -> Self {
        Self::default()
    }

    /// 将归一化的 `x` 坐标换算为剪映使用的 `transform_x`。
    pub fn to_jy_transform_x(&self) -> f64 {
        (self.x - 0.5) * 2.0
    }

    /// 将归一化的 `y` 坐标换算为剪映使用的 `transform_y`。
    pub fn to_jy_transform_y(&self) -> f64 {
        (self.y - 0.5) * 2.0
    }
}
