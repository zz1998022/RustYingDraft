use jy_schema::{Clip, SchemaError, Track, TrackKind};
use uuid::Uuid;

/// Builder for constructing a track with validated clips.
pub struct TrackBuilder {
    track: Track,
}

impl TrackBuilder {
    pub fn new(kind: TrackKind, name: &str, render_index: i32) -> Self {
        Self {
            track: Track::new(
                Uuid::new_v4().as_simple().to_string(),
                kind,
                name.to_string(),
                render_index,
            ),
        }
    }

    pub fn mute(mut self, mute: bool) -> Self {
        self.track.mute = mute;
        self
    }

    /// Add a clip to the track. Validates type compatibility and time overlap.
    pub fn add_clip(mut self, clip: Clip) -> Result<Self, SchemaError> {
        // Type check
        if !self.track.kind.accepts_clip(&clip) {
            return Err(SchemaError::ClipTypeMismatch {
                clip_type: clip_name(&clip),
                track_kind: self.track.kind.to_str().to_string(),
            });
        }

        // Overlap check
        let new_range = clip.target_timerange();
        for existing in &self.track.clips {
            if existing.target_timerange().overlaps(new_range) {
                return Err(SchemaError::SegmentOverlap {
                    start: new_range.start,
                    end: new_range.end(),
                });
            }
        }

        self.track.clips.push(clip);
        Ok(self)
    }

    pub fn build(self) -> Track {
        self.track
    }
}

fn clip_name(clip: &Clip) -> String {
    match clip {
        Clip::Video(_) => "video".into(),
        Clip::Audio(_) => "audio".into(),
        Clip::Text(_) => "text".into(),
        Clip::Image(_) => "image".into(),
    }
}
