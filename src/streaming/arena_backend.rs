//! Arena allocation backend for streaming formatter.
//!
//! This module provides the arena-based backend implementation using bumpalo
//! for temporary allocations (context stack and anchor storage). The arena
//! is created and destroyed within the public API function, ensuring all
//! temporary allocations are freed at once.

use std::fmt::Write as FmtWrite;

use bumpalo::Bump;
use saphyr_parser::Parser;

use super::Context;
use super::extract_anchor_names;
use super::formatter::StreamingFormatter;
use super::traits::{AnchorStoreOps, ContextStackOps, FormatterBackend};
use crate::emitter::EmitterConfig;
use crate::error::{EmitError, EmitResult};

/// Arena allocation backend.
///
/// Uses `bumpalo::collections::Vec<'bump, Context>` for context stack and
/// `bumpalo::collections::Vec<'bump, String<'bump>>` for anchor storage.
///
/// This backend requires the `arena` feature flag.
pub(super) struct ArenaBackend<'bump> {
    context_stack: bumpalo::collections::Vec<'bump, Context>,
    anchor_names: bumpalo::collections::Vec<'bump, bumpalo::collections::String<'bump>>,
    #[allow(dead_code)] // Keep arena reference for potential future use
    arena: &'bump Bump,
}

impl<'bump> ArenaBackend<'bump> {
    /// Creates a new arena backend with pre-allocated capacity.
    ///
    /// # Arguments
    ///
    /// * `context_capacity` - Initial capacity for context stack (typical: 16)
    /// * `arena` - Arena allocator for temporary allocations
    pub fn new(context_capacity: usize, arena: &'bump Bump) -> Self {
        let mut context_stack =
            bumpalo::collections::Vec::with_capacity_in(context_capacity, arena);
        context_stack.push(Context::Root);

        Self {
            context_stack,
            anchor_names: bumpalo::collections::Vec::new_in(arena),
            arena,
        }
    }
}

// Implementation of ContextStackOps for bumpalo::collections::Vec<'bump, Context>
#[allow(clippy::use_self)] // bumpalo Vec method names match trait names
impl ContextStackOps for bumpalo::collections::Vec<'_, Context> {
    #[inline]
    fn push(&mut self, ctx: Context) {
        bumpalo::collections::Vec::push(self, ctx);
    }

    #[inline]
    fn pop(&mut self) -> Option<Context> {
        bumpalo::collections::Vec::pop(self)
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
        bumpalo::collections::Vec::len(self)
    }
}

// Implementation of AnchorStoreOps for bumpalo::collections::Vec<'bump, String<'bump>>
#[allow(clippy::use_self)] // bumpalo Vec/slice method names used
impl<'bump> AnchorStoreOps
    for bumpalo::collections::Vec<'bump, bumpalo::collections::String<'bump>>
{
    fn ensure_capacity(&mut self, anchor_id: usize) {
        // bumpalo::collections::Vec lacks resize_with, use while loop
        while self.len() <= anchor_id {
            self.push(bumpalo::collections::String::new_in(self.bump()));
        }
    }

    fn get(&self, anchor_id: usize) -> Option<&str> {
        <[bumpalo::collections::String]>::get(self, anchor_id)
            .filter(|s| !s.is_empty())
            .map(bumpalo::collections::String::as_str)
    }

    fn set_if_empty(&mut self, anchor_id: usize) -> &str {
        if self[anchor_id].is_empty() {
            // Generate name directly into storage (single allocation)
            let _ = write!(self[anchor_id], "anchor{anchor_id}");
        }
        self[anchor_id].as_str()
    }
}

// Implementation of FormatterBackend for ArenaBackend
impl<'bump> FormatterBackend for ArenaBackend<'bump> {
    type ContextStack = bumpalo::collections::Vec<'bump, Context>;
    type AnchorStore = bumpalo::collections::Vec<'bump, bumpalo::collections::String<'bump>>;

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

/// Format YAML using streaming parser events with arena allocation.
///
/// This function uses bumpalo arena allocation for temporary structures,
/// reducing heap allocation overhead. The arena is created and destroyed
/// within this function, ensuring all temporary allocations are freed at once.
///
/// # Performance
///
/// Arena allocation provides 5-14% speedup over standard allocation
/// (measured across document sizes from 100 to 50000 lines) by:
/// - Eliminating individual deallocation overhead
/// - Reducing allocator lock contention
/// - Improving memory locality
///
/// Note: Performance gains diminish for larger documents as parser overhead
/// dominates allocation costs.
///
/// # Errors
///
/// Returns `EmitError::Emit` if the parser encounters invalid YAML.
///
/// # Examples
///
/// ```
/// # #[cfg(all(feature = "streaming", feature = "arena"))]
/// # {
/// use rs_yaml::streaming::format_streaming_arena;
/// use rs_yaml::EmitterConfig;
///
/// let yaml = "key: value\nlist:\n  - item1\n  - item2\n";
/// let config = EmitterConfig::default();
/// let formatted = format_streaming_arena(yaml, &config).unwrap();
/// assert!(formatted.contains("key:"));
/// # }
/// ```
pub fn format_streaming_arena(input: &str, config: &EmitterConfig) -> EmitResult<String> {
    // Create arena sized for typical YAML overhead
    // 4KB minimum handles most documents; larger inputs get proportional arenas
    let arena_size = (input.len() / 4).max(4096);
    let arena = Bump::with_capacity(arena_size);

    let parser = Parser::new_from_str(input);

    // Output is typically 10-20% larger than input due to formatting
    let output_capacity = input.len() + (input.len() / 5);

    // Pre-allocate context stack in arena (16 levels handles 99% of cases)
    let context_capacity = 16;

    let anchor_names = extract_anchor_names(input);
    let mut backend = ArenaBackend::new(context_capacity, &arena);
    // Seed anchor store with original names extracted from input.
    // ensure_capacity + direct write mirrors how StdBackend is seeded.
    {
        let store = backend.anchor_store_mut();
        for (id, name) in anchor_names.into_iter().enumerate() {
            store.ensure_capacity(id);
            if !name.is_empty() && store[id].is_empty() {
                let _ = write!(store[id], "{name}");
            }
        }
    }
    let mut formatter = StreamingFormatter::new(config, output_capacity, backend);

    for result in parser {
        let (event, span) = result.map_err(|e| EmitError::Emit(e.to_string()))?;
        formatter.format_event(event, span);
    }

    Ok(formatter.finish())
    // Arena dropped here - all temporary allocations freed at once
}
