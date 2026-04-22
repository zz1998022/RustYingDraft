/// 画布配置。
///
/// 对应剪映草稿中的宽、高、帧率三元组。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

impl Default for Canvas {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            fps: 30,
        }
    }
}

impl Canvas {
    /// 创建一个新的画布配置。
    pub fn new(width: u32, height: u32, fps: u32) -> Self {
        Self { width, height, fps }
    }
}
