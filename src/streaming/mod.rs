//! Event-based formatting. Anchors, explicit tags and block scalar styles are preserved.
//! The returned string owns the complete output.

use crate::{EmitError, EmitResult, EmitterConfig};

/// Format YAML using the guarded heap-based event formatter.
pub fn format_streaming(input: &str, config: &EmitterConfig) -> EmitResult<String> {
    check_input_size(input)?;
    fast_yaml_core::streaming::format_streaming(input, &config.upstream()?)
        .map_err(|error| EmitError::Emit(error.to_string()))
}

/// Format YAML with an arena for temporary parser state.
#[cfg(feature = "arena")]
pub fn format_streaming_arena(input: &str, config: &EmitterConfig) -> EmitResult<String> {
    check_input_size(input)?;
    fast_yaml_core::streaming::format_streaming_arena(input, &config.upstream()?)
        .map_err(|error| EmitError::Emit(error.to_string()))
}

/// Historical heuristic for callers choosing between streaming and DOM processing.
/// Returns false when more than one in a hundred bytes is an anchor or alias indicator.
#[must_use]
pub fn is_streaming_suitable(input: &str) -> bool {
    input.bytes().filter(|b| matches!(b, b'&' | b'*')).count() <= input.len() / 100
}

fn check_input_size(input: &str) -> EmitResult<()> {
    if input.len() > crate::load_guard::MAX_INPUT_BYTES {
        return Err(EmitError::Emit(
            "input exceeds compatibility formatter byte limit".into(),
        ));
    }
    Ok(())
}
