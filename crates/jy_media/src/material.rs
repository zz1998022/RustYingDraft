use crate::error::MediaError;
use crate::probe::{MediaInfo, MediaKind};
use camino::{Utf8Path, Utf8PathBuf};
use jy_schema::{AudioMaterialRef, CropSettings, MaterialKind, VideoMaterialRef};
use uuid::Uuid;

/// 将输入路径规范化为绝对路径。
///
/// 剪映草稿最终写入的是本机绝对路径，因此在素材探测阶段就统一做绝对化，
/// 可以减少后续“草稿能打开但素材丢失”的问题。
fn absolutize_path(path: &Utf8Path) -> Result<Utf8PathBuf, MediaError> {
    let absolute = std::fs::canonicalize(path)?;
    Utf8PathBuf::from_path_buf(absolute).map_err(|pb| {
        MediaError::IoError(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("non-utf8 path: {}", pb.display()),
        ))
    })
}

/// 通过媒体探测结果创建视频/图片素材引用。
pub fn create_video_material(
    path: &Utf8Path,
    name: Option<&str>,
) -> Result<VideoMaterialRef, MediaError> {
    let absolute_path = absolutize_path(path)?;
    let info = MediaInfo::from_path(&absolute_path)?;
    let material_name = name
        .map(|s| s.to_string())
        .or_else(|| absolute_path.file_name().map(|s| s.to_string()))
        .unwrap_or_default();

    let kind = match info.kind {
        MediaKind::Video => MaterialKind::Video,
        MediaKind::Photo => MaterialKind::Photo,
        MediaKind::Audio => {
            return Err(MediaError::NoVideoStream {
                path: path.to_string(),
            })
        }
    };

    let duration = info.duration_us.unwrap_or(0);

    Ok(VideoMaterialRef {
        // 这里的 ID 只是工程内部引用 ID，不是素材文件的稳定 ID。
        id: Uuid::new_v4().as_simple().to_string(),
        path: absolute_path,
        duration,
        width: info.width.unwrap_or(0),
        height: info.height.unwrap_or(0),
        kind,
        crop: CropSettings::default(),
        name: material_name,
    })
}

/// 通过媒体探测结果创建音频素材引用。
pub fn create_audio_material(
    path: &Utf8Path,
    name: Option<&str>,
) -> Result<AudioMaterialRef, MediaError> {
    let absolute_path = absolutize_path(path)?;
    let info = MediaInfo::from_path(&absolute_path)?;
    let material_name = name
        .map(|s| s.to_string())
        .or_else(|| absolute_path.file_name().map(|s| s.to_string()))
        .unwrap_or_default();

    if info.kind != MediaKind::Audio {
        return Err(MediaError::NoAudioStream {
            path: path.to_string(),
        });
    }

    let duration = info.duration_us.unwrap_or(0);

    Ok(AudioMaterialRef {
        id: Uuid::new_v4().as_simple().to_string(),
        path: absolute_path,
        duration,
        name: material_name,
    })
}
