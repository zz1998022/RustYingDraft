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
    create_video_material_from_info(&absolute_path, name, &info)
}

/// 使用已探测的媒体信息创建视频/图片素材引用。
///
/// 调用方已经拿到 `MediaInfo` 时使用这个函数，避免同一个素材在一次导入中被重复 ffprobe。
pub fn create_video_material_from_info(
    path: &Utf8Path,
    name: Option<&str>,
    info: &MediaInfo,
) -> Result<VideoMaterialRef, MediaError> {
    let absolute_path = absolutize_path(path)?;
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
    create_audio_material_from_info(&absolute_path, name, &info)
}

/// 使用已探测的媒体信息创建音频素材引用。
pub fn create_audio_material_from_info(
    path: &Utf8Path,
    name: Option<&str>,
    info: &MediaInfo,
) -> Result<AudioMaterialRef, MediaError> {
    let absolute_path = absolutize_path(path)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::{MediaInfo, MediaKind};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_media_path(file_name: &str) -> Utf8PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("jy_media_material_test_{unique}"));
        fs::create_dir_all(&dir).unwrap();
        Utf8PathBuf::from_path_buf(dir.join(file_name)).unwrap()
    }

    #[test]
    fn creates_video_material_from_declared_media_info_without_probe() {
        let path = temp_media_path("clip.mp4");
        fs::write(&path, b"not a real mp4").unwrap();

        let material = create_video_material_from_info(
            &path,
            Some("clip"),
            &MediaInfo {
                kind: MediaKind::Video,
                duration_us: Some(1_500_000),
                width: Some(1920),
                height: Some(1080),
                sample_rate: None,
            },
        )
        .unwrap();

        assert_eq!(material.path, path.canonicalize_utf8().unwrap());
        assert_eq!(material.name, "clip");
        assert_eq!(material.duration, 1_500_000);
        assert_eq!(material.width, 1920);
        assert_eq!(material.height, 1080);
        assert_eq!(material.kind, MaterialKind::Video);
    }

    #[test]
    fn creates_audio_material_from_declared_media_info_without_probe() {
        let path = temp_media_path("narration.wav");
        fs::write(&path, b"not a real wav").unwrap();

        let material = create_audio_material_from_info(
            &path,
            None,
            &MediaInfo {
                kind: MediaKind::Audio,
                duration_us: Some(900_000),
                width: None,
                height: None,
                sample_rate: Some(44_100),
            },
        )
        .unwrap();

        assert_eq!(material.path, path.canonicalize_utf8().unwrap());
        assert_eq!(material.name, "narration.wav");
        assert_eq!(material.duration, 900_000);
    }
}
