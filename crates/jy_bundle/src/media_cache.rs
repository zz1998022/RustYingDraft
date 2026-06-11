use std::collections::HashMap;
use std::thread;

use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use jy_media::material::{create_audio_material_from_info, create_video_material_from_info};
use jy_media::probe::MediaInfo;
use jy_schema::{AudioMaterialRef, VideoMaterialRef};

use crate::fs_util::utf8_path_buf;

#[derive(Debug, Clone)]
pub(crate) struct MediaProbeRequest {
    pub(crate) path: Utf8PathBuf,
    pub(crate) label: String,
}

#[derive(Debug, Default)]
pub(crate) struct MediaMaterialCache {
    infos: HashMap<Utf8PathBuf, MediaInfo>,
}

impl MediaMaterialCache {
    pub(crate) fn preload<I, F, C>(requests: I, mut on_loaded: F, is_cancelled: C) -> Result<Self>
    where
        I: IntoIterator<Item = MediaProbeRequest>,
        F: FnMut(usize, usize, &MediaProbeRequest),
        C: Fn() -> bool,
    {
        Self::preload_with_probe(requests, &mut on_loaded, is_cancelled, |path| {
            Ok(MediaInfo::from_path(path)?)
        })
    }

    fn preload_with_probe<I, F, C, P>(
        requests: I,
        on_loaded: &mut F,
        is_cancelled: C,
        probe: P,
    ) -> Result<Self>
    where
        I: IntoIterator<Item = MediaProbeRequest>,
        F: FnMut(usize, usize, &MediaProbeRequest),
        C: Fn() -> bool,
        P: Fn(&Utf8Path) -> Result<MediaInfo> + Send + Sync,
    {
        let mut unique = Vec::<MediaProbeRequest>::new();
        let mut seen = HashMap::<Utf8PathBuf, usize>::new();
        for request in requests {
            let absolute = canonicalize_utf8(&request.path)
                .with_context(|| format!("failed to canonicalize media path: {}", request.path))?;
            if seen.contains_key(&absolute) {
                continue;
            }
            seen.insert(absolute.clone(), unique.len());
            unique.push(MediaProbeRequest {
                path: absolute,
                label: request.label,
            });
        }

        let total = unique.len();
        let parallelism = std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(1)
            .clamp(1, 4);
        let mut infos = HashMap::with_capacity(total);
        let mut loaded = 0_usize;

        for chunk in unique.chunks(parallelism) {
            if is_cancelled() {
                anyhow::bail!("import cancelled");
            }
            let results = thread::scope(|scope| {
                let probe = &probe;
                let handles = chunk
                    .iter()
                    .cloned()
                    .map(|request| {
                        scope.spawn(move || {
                            let info = probe(&request.path).with_context(|| request.label.clone());
                            (request, info)
                        })
                    })
                    .collect::<Vec<_>>();

                handles
                    .into_iter()
                    .map(|handle| {
                        handle
                            .join()
                            .map_err(|_| anyhow!("media probe worker panicked"))
                    })
                    .collect::<Result<Vec<_>>>()
            })?;

            for (request, info) in results {
                let info = info?;
                loaded += 1;
                on_loaded(loaded, total, &request);
                infos.insert(request.path, info);
            }
            if is_cancelled() {
                anyhow::bail!("import cancelled");
            }
        }

        Ok(Self { infos })
    }

    pub(crate) fn create_video_material(
        &self,
        path: &Utf8Path,
        name: Option<&str>,
    ) -> Result<VideoMaterialRef> {
        let absolute = canonicalize_utf8(path)?;
        let info = self
            .infos
            .get(&absolute)
            .ok_or_else(|| anyhow!("media info not loaded: {absolute}"))?;
        Ok(create_video_material_from_info(&absolute, name, info)?)
    }

    pub(crate) fn create_audio_material(
        &self,
        path: &Utf8Path,
        name: Option<&str>,
    ) -> Result<AudioMaterialRef> {
        let absolute = canonicalize_utf8(path)?;
        let info = self
            .infos
            .get(&absolute)
            .ok_or_else(|| anyhow!("media info not loaded: {absolute}"))?;
        Ok(create_audio_material_from_info(&absolute, name, info)?)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.infos.len()
    }
}

fn canonicalize_utf8(path: &Utf8Path) -> Result<Utf8PathBuf> {
    utf8_path_buf(std::fs::canonicalize(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_file(name: &str) -> Utf8PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("jy_bundle_cache_test_{unique}"));
        fs::create_dir_all(&dir).unwrap();
        let path = Utf8PathBuf::from_path_buf(dir.join(name)).unwrap();
        fs::write(&path, b"fake").unwrap();
        path
    }

    #[test]
    fn preload_deduplicates_same_path() -> Result<()> {
        let path = temp_file("clip.mp4");
        let mut loaded = Vec::new();
        let probe_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let probe_count_for_probe = std::sync::Arc::clone(&probe_count);
        let cache = MediaMaterialCache::preload_with_probe(
            [
                MediaProbeRequest {
                    path: path.clone(),
                    label: "first".to_string(),
                },
                MediaProbeRequest {
                    path,
                    label: "second".to_string(),
                },
            ],
            &mut |current, total, request| loaded.push((current, total, request.label.clone())),
            || false,
            |_| {
                probe_count_for_probe.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(MediaInfo {
                    kind: jy_media::probe::MediaKind::Video,
                    duration_us: Some(1_000_000),
                    width: Some(16),
                    height: Some(16),
                    sample_rate: None,
                })
            },
        )?;

        assert_eq!(cache.len(), 1);
        assert_eq!(probe_count.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].0, 1);
        assert_eq!(loaded[0].1, 1);
        Ok(())
    }
}
