//! Standard heap allocation backend for streaming formatter.
//!
//! This module provides the default backend implementation using standard
//! Rust collections (`Vec<T>`, `String`) for context tracking and anchor storage.

use std::fmt::Write as FmtWrite;

use saphyr_parser::Parser;

use super::Context;
use super::extract_anchor_names;
use super::formatter::StreamingFormatter;
use super::traits::{AnchorStoreOps, ContextStackOps, FormatterBackend};
use crate::emitter::EmitterConfig;
use crate::error::{EmitError, EmitResult};

/// Standard heap allocation backend.
///
/// Uses `Vec<Context>` for context stack and `Vec<String>` for anchor storage.
/// This is the default allocation strategy with no special feature requirements.
pub(super) struct StdBackend {
    context_stack: Vec<Context>,
    anchor_names: Vec<String>,
}

impl StdBackend {
    /// Creates a new standard backend with pre-allocated capacity.
    ///
    /// # Arguments
    ///
    /// * `context_capacity` - Initial capacity for context stack (typical: 16)
    /// * `anchor_capacity` - Initial capacity for anchor storage (typical: 1-4)
    pub fn new(context_capacity: usize, anchor_capacity: usize) -> Self {
        let mut context_stack = Vec::with_capacity(context_capacity);
        context_stack.push(Context::Root);

        Self {
            context_stack,
            anchor_names: Vec::with_capacity(anchor_capacity),
        }
    }
}

// Implementation of ContextStackOps for Vec<Context>
#[allow(clippy::use_self)] // Vec method names match trait names
impl ContextStackOps for Vec<Context> {
    #[inline]
    fn push(&mut self, ctx: Context) {
        Vec::push(self, ctx);
    }

    #[inline]
    fn pop(&mut self) -> Option<Context> {
        Vec::pop(self)
    }

    #[inline]
    fn last(&self) -> Option<&Context> {
        <[Context]>::last(self)
    }

    #[inline]
    fn last_mut(&mut self) -> Option<&mut Context> {
        <[Context]>::last_mut(self)
    }

    #[inline]
    fn len(&self) -> usize {
        Vec::len(self)
    }
}

// Implementation of AnchorStoreOps for Vec<String>
#[allow(clippy::use_self)] // Vec/slice method names used
impl AnchorStoreOps for Vec<String> {
    fn ensure_capacity(&mut self, anchor_id: usize) {
        if self.len() <= anchor_id {
            self.resize(anchor_id + 1, String::new());
        }
    }

    fn get(&self, anchor_id: usize) -> Option<&str> {
        <[String]>::get(self, anchor_id)
            .filter(|s| !s.is_empty())
            .map(String::as_str)
    }

    fn set_if_empty(&mut self, anchor_id: usize) -> &str {
        if self[anchor_id].is_empty() {
            // Generate name directly into storage (single allocation)
            let _ = write!(self[anchor_id], "anchor{anchor_id}");
        }
        &self[anchor_id]
    }
}

// Implementation of FormatterBackend for StdBackend
impl FormatterBackend for StdBackend {
    type ContextStack = Vec<Context>;
    type AnchorStore = Vec<String>;

    #[inline]
    fn context_stack(&self) -> &Self::ContextStack {
        &self.context_stack
    }

    #[inline]
    fn context_stack_mut(&mut self) -> &mut Self::ContextStack {
        &mut self.context_stack
    }

    #[inline]
    fn anchor_store(&self) -> &Self::AnchorStore {
        &self.anchor_names
    }

    #[inline]
    fn anchor_store_mut(&mut self) -> &mut Self::AnchorStore {
        &mut self.anchor_names
    }
}

/// Format YAML using streaming parser events with standard heap allocation.
///
/// This function bypasses DOM construction for better performance on large files.
/// It processes parser events directly, maintaining O(1) memory complexity
/// relative to the portion of the file being processed.
///
/// # Errors
///
/// Returns `EmitError::Emit` if the parser encounters invalid YAML.
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "streaming")]
/// # {
/// use rs_yaml::streaming::format_streaming;
/// use rs_yaml::EmitterConfig;
///
/// let yaml = "key: value\nlist:\n  - item1\n  - item2\n";
/// let config = EmitterConfig::default();
/// let formatted = format_streaming(yaml, &config).unwrap();
/// assert!(formatted.contains("key:"));
/// # }
/// ```
pub fn format_streaming(input: &str, config: &EmitterConfig) -> EmitResult<String> {
    let parser = Parser::new_from_str(input);

    // Output is typically 10-20% larger than input due to formatting
    let output_capacity = input.len() + (input.len() / 5);

    // Pre-allocate for typical nesting depth (16 levels handles 99% of cases)
    let context_capacity = 16;

    // Pre-allocate for a reasonable number of anchors (~4 anchors per KB)
    let anchor_capacity = input.len().min(1024) / 256;

    let anchor_names = extract_anchor_names(input);
    let mut backend = StdBackend::new(context_capacity, anchor_capacity.max(1));
    *backend.anchor_store_mut() = anchor_names;
    let mut formatter = StreamingFormatter::new(config, output_capacity, backend);

    for result in parser {
        let (event, span) = result.map_err(|e| EmitError::Emit(e.to_string()))?;
        formatter.format_event(event, span);
    }

    Ok(formatter.finish())
}
