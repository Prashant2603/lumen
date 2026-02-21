use std::borrow::Cow;
use std::sync::Arc;

use flash_core::pipeline::{LayerKind, PipelineConfig, PipelineLayer};
use regex::Regex;

/// UI-side state for a single pipeline layer.
pub struct UiLayer {
    pub layer:         PipelineLayer,
    /// Compiled regex for Rewrite/Mask render-time transforms.
    pub compiled_re:   Option<Regex>,
    pub editing:       bool,
    pub draft_pattern: String,
    pub draft_extra:   String,   // replacement or mask_with
    pub parse_error:   Option<String>,
}

impl UiLayer {
    pub fn new_filter(id: u64, pattern: &str, exclude: bool) -> Self {
        Self {
            layer: PipelineLayer {
                id,
                kind: LayerKind::Filter {
                    pattern: pattern.to_string(),
                    exclude,
                },
                enabled: true,
            },
            compiled_re:   None,
            editing:       true,
            draft_pattern: pattern.to_string(),
            draft_extra:   String::new(),
            parse_error:   None,
        }
    }

    pub fn new_rewrite(id: u64, find: &str, replacement: &str) -> Self {
        let compiled_re = Regex::new(find).ok();
        Self {
            layer: PipelineLayer {
                id,
                kind: LayerKind::Rewrite {
                    find:        find.to_string(),
                    replacement: replacement.to_string(),
                },
                enabled: true,
            },
            compiled_re,
            editing:       true,
            draft_pattern: find.to_string(),
            draft_extra:   replacement.to_string(),
            parse_error:   None,
        }
    }

    pub fn new_mask(id: u64, pattern: &str, mask_with: &str) -> Self {
        let compiled_re = Regex::new(pattern).ok();
        Self {
            layer: PipelineLayer {
                id,
                kind: LayerKind::Mask {
                    pattern:   pattern.to_string(),
                    mask_with: mask_with.to_string(),
                },
                enabled: true,
            },
            compiled_re,
            editing:       true,
            draft_pattern: pattern.to_string(),
            draft_extra:   mask_with.to_string(),
            parse_error:   None,
        }
    }

    /// Kind label for display.
    pub fn kind_label(&self) -> &'static str {
        match &self.layer.kind {
            LayerKind::Filter  { .. } => "FILTER",
            LayerKind::Rewrite { .. } => "REWRITE",
            LayerKind::Mask    { .. } => "MASK",
        }
    }

    /// Whether this layer is a Filter (exclude) variant.
    pub fn is_exclude(&self) -> bool {
        matches!(&self.layer.kind, LayerKind::Filter { exclude: true, .. })
    }
}

/// UI-side pipeline state (not sent to the worker).
pub struct TransformPipeline {
    pub layers:  Vec<UiLayer>,
    pub next_id: u64,
}

impl TransformPipeline {
    pub fn new() -> Self {
        Self { layers: Vec::new(), next_id: 1 }
    }

    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// True if any enabled Filter layers exist.
    pub fn has_active_filter_layers(&self) -> bool {
        self.layers.iter().any(|ul| {
            ul.layer.enabled && matches!(ul.layer.kind, LayerKind::Filter { .. })
        })
    }

    /// Build a config snapshot for the worker thread.
    pub fn to_config(&self) -> PipelineConfig {
        Arc::new(
            self.layers
                .iter()
                .filter(|ul| ul.layer.enabled)
                .map(|ul| ul.layer.clone())
                .collect(),
        )
    }

    /// Build a config snapshot including only layers up to and including `id`.
    pub fn to_config_up_to(&self, id: u64) -> PipelineConfig {
        let end = self.layers.iter().position(|ul| ul.layer.id == id)
            .map(|i| i + 1)
            .unwrap_or(self.layers.len());
        Arc::new(
            self.layers[..end]
                .iter()
                .filter(|ul| ul.layer.enabled)
                .map(|ul| ul.layer.clone())
                .collect(),
        )
    }

    /// True if any enabled Filter layers exist up to and including `id`.
    pub fn has_filter_layers_up_to(&self, id: u64) -> bool {
        let end = self.layers.iter().position(|ul| ul.layer.id == id)
            .map(|i| i + 1)
            .unwrap_or(self.layers.len());
        self.layers[..end].iter().any(|ul| {
            ul.layer.enabled && matches!(ul.layer.kind, LayerKind::Filter { .. })
        })
    }

    /// Apply only Rewrite/Mask layers up to and including `id` at render time.
    pub fn apply_text_transforms_up_to<'a>(&self, line: &'a str, id: u64) -> Cow<'a, str> {
        let end = self.layers.iter().position(|ul| ul.layer.id == id)
            .map(|i| i + 1)
            .unwrap_or(self.layers.len());
        let mut result: Cow<str> = Cow::Borrowed(line);
        for ul in &self.layers[..end] {
            if !ul.layer.enabled { continue; }
            match &ul.layer.kind {
                LayerKind::Rewrite { replacement, .. } => {
                    if let Some(re) = &ul.compiled_re {
                        let replaced = re.replace_all(&result, replacement.as_str());
                        if let Cow::Owned(s) = replaced { result = Cow::Owned(s); }
                    }
                }
                LayerKind::Mask { mask_with, .. } => {
                    if let Some(re) = &ul.compiled_re {
                        let replaced = re.replace_all(&result, mask_with.as_str());
                        if let Cow::Owned(s) = replaced { result = Cow::Owned(s); }
                    }
                }
                LayerKind::Filter { .. } => {}
            }
        }
        result
    }

    /// Apply Rewrite and Mask layers to a line of text at render time.
    pub fn apply_text_transforms<'a>(&self, line: &'a str) -> Cow<'a, str> {
        let mut result: Cow<str> = Cow::Borrowed(line);
        for ul in &self.layers {
            if !ul.layer.enabled { continue; }
            match &ul.layer.kind {
                LayerKind::Rewrite { replacement, .. } => {
                    if let Some(re) = &ul.compiled_re {
                        let replaced = re.replace_all(&result, replacement.as_str());
                        if let Cow::Owned(s) = replaced {
                            result = Cow::Owned(s);
                        }
                    }
                }
                LayerKind::Mask { mask_with, .. } => {
                    if let Some(re) = &ul.compiled_re {
                        let replaced = re.replace_all(&result, mask_with.as_str());
                        if let Cow::Owned(s) = replaced {
                            result = Cow::Owned(s);
                        }
                    }
                }
                LayerKind::Filter { .. } => {}
            }
        }
        result
    }
}
