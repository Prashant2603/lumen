use std::sync::Arc;

/// The kind of pipeline layer.
#[derive(Debug, Clone)]
pub enum LayerKind {
    /// Keep (or exclude) lines matching a regex pattern.
    Filter { pattern: String, exclude: bool },
    /// Replace matches of a regex with a replacement string.
    Rewrite { find: String, replacement: String },
    /// Replace matches of a regex with a mask string.
    Mask { pattern: String, mask_with: String },
}

/// A single layer in the transformation pipeline.
#[derive(Debug, Clone)]
pub struct PipelineLayer {
    pub id:      u64,
    pub kind:    LayerKind,
    pub enabled: bool,
}

/// A snapshot of the pipeline configuration, shareable across threads.
pub type PipelineConfig = Arc<Vec<PipelineLayer>>;

/// Response from the pipeline worker.
#[derive(Debug, Clone)]
pub enum PipelineResponse {
    /// All line indices that passed the filter layers.
    Complete(Vec<usize>),
    /// The run was cancelled by a newer config.
    Cancelled,
    /// An error occurred (e.g. bad regex).
    Error(String),
}
