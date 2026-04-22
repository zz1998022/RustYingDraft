use thiserror::Error;

#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("segment overlap: new segment [{start}, {end}) overlaps existing")]
    SegmentOverlap { start: u64, end: u64 },

    #[error("track not found: {name}")]
    TrackNotFound { name: String },

    #[error("ambiguous track: multiple tracks match '{name}'")]
    AmbiguousTrack { name: String },

    #[error("source timerange ({source_end}us) exceeds material duration ({material_duration}us)")]
    SourceRangeExceedsDuration {
        source_end: u64,
        material_duration: u64,
    },

    #[error("material not found: {name}")]
    MaterialNotFound { name: String },

    #[error("clip type mismatch: {clip_type} cannot be added to {track_kind} track")]
    ClipTypeMismatch {
        clip_type: String,
        track_kind: String,
    },
}
