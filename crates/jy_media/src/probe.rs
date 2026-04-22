use crate::error::MediaError;
use jy_schema::PHOTO_DURATION_US;
use serde::Deserialize;
use std::process::Command;

/// 媒体种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Video,
    Photo,
    Audio,
}

/// `ffprobe` 归一化后的媒体信息。
#[derive(Debug, Clone)]
pub struct MediaInfo {
    pub kind: MediaKind,
    pub duration_us: Option<u64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub sample_rate: Option<u32>,
}

// `ffprobe` 输出 JSON 的最小映射结构，只保留当前需要的字段。
#[derive(Deserialize)]
struct FfprobeOutput {
    streams: Option<Vec<StreamInfo>>,
    format: Option<FormatInfo>,
}

#[derive(Deserialize)]
struct StreamInfo {
    codec_type: Option<String>,
    duration: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    sample_rate: Option<String>,
    // 用于区分“真正的视频”和“只有一帧的静态图”。
    #[serde(rename = "avg_frame_rate")]
    avg_frame_rate: Option<String>,
}

#[derive(Deserialize)]
struct FormatInfo {
    duration: Option<String>,
}

impl MediaInfo {
    /// 使用 `ffprobe` 探测媒体文件。
    ///
    /// 当前策略：
    /// - 常见图片扩展名直接视为 `Photo`
    /// - 有视频流且不是静态图的视为 `Video`
    /// - 只有音频流的视为 `Audio`
    pub fn from_path(path: &camino::Utf8Path) -> Result<Self, MediaError> {
        if !path.exists() {
            return Err(MediaError::FileNotFound {
                path: path.to_string(),
            });
        }

        let output = Command::new("ffprobe")
            .args([
                "-print_format",
                "json",
                "-show_streams",
                "-show_format",
                "-v",
                "quiet",
            ])
            .arg(path.as_str())
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    MediaError::FfprobeNotFound
                } else {
                    MediaError::FfprobeFailed(e.to_string())
                }
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(MediaError::FfprobeFailed(stderr.to_string()));
        }

        let probe: FfprobeOutput = serde_json::from_slice(&output.stdout)?;

        let streams = probe.streams.unwrap_or_default();

        let has_video = streams
            .iter()
            .any(|s| s.codec_type.as_deref() == Some("video"));
        let has_audio = streams
            .iter()
            .any(|s| s.codec_type.as_deref() == Some("audio"));

        // `avg_frame_rate = 0/0` 时，一般意味着这是静态图而不是真视频。
        let is_image = streams.iter().any(|s| {
            s.codec_type.as_deref() == Some("video") && s.avg_frame_rate.as_deref() == Some("0/0")
        });

        let extension = path.extension().unwrap_or("").to_lowercase();
        let is_gif = extension == "gif";
        let is_image_ext = matches!(
            extension.as_str(),
            "jpg" | "jpeg" | "png" | "bmp" | "webp" | "tiff" | "tif"
        );

        // 对明确的图片扩展名优先按图片处理，避免被 ffprobe 的视频流信息误导。
        if is_image_ext {
            let video_stream = streams
                .iter()
                .find(|s| s.codec_type.as_deref() == Some("video"));
            return Ok(Self {
                kind: MediaKind::Photo,
                duration_us: Some(jy_schema::PHOTO_DURATION_US),
                width: video_stream.and_then(|s| s.width),
                height: video_stream.and_then(|s| s.height),
                sample_rate: None,
            });
        }

        if has_video && !is_image {
            let video_stream = streams
                .iter()
                .find(|s| s.codec_type.as_deref() == Some("video"));

            let duration = video_stream
                .and_then(|s| s.duration.as_ref())
                .and_then(|d| d.parse::<f64>().ok())
                .or_else(|| {
                    probe
                        .format
                        .as_ref()
                        .and_then(|f| f.duration.as_ref())
                        .and_then(|d| d.parse::<f64>().ok())
                })
                .map(|d| (d * 1_000_000.0) as u64);

            Ok(Self {
                kind: if is_gif {
                    MediaKind::Video
                } else {
                    MediaKind::Video
                },
                duration_us: duration,
                width: video_stream.and_then(|s| s.width),
                height: video_stream.and_then(|s| s.height),
                sample_rate: None,
            })
        } else if is_image {
            let video_stream = streams
                .iter()
                .find(|s| s.codec_type.as_deref() == Some("video"));
            Ok(Self {
                kind: MediaKind::Photo,
                duration_us: Some(PHOTO_DURATION_US),
                width: video_stream.and_then(|s| s.width),
                height: video_stream.and_then(|s| s.height),
                sample_rate: None,
            })
        } else if has_audio {
            let audio_stream = streams
                .iter()
                .find(|s| s.codec_type.as_deref() == Some("audio"));

            let duration = audio_stream
                .and_then(|s| s.duration.as_ref())
                .and_then(|d| d.parse::<f64>().ok())
                .or_else(|| {
                    probe
                        .format
                        .as_ref()
                        .and_then(|f| f.duration.as_ref())
                        .and_then(|d| d.parse::<f64>().ok())
                })
                .map(|d| (d * 1_000_000.0) as u64);

            Ok(Self {
                kind: MediaKind::Audio,
                duration_us: duration,
                width: None,
                height: None,
                sample_rate: audio_stream
                    .and_then(|s| s.sample_rate.as_ref())
                    .and_then(|r| r.parse().ok()),
            })
        } else {
            Err(MediaError::UnsupportedFormat {
                path: path.to_string(),
            })
        }
    }
}
