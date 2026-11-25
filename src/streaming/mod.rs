//! Event-based formatting. Anchors, explicit tags and block scalar styles are preserved.
//! The returned string owns the complete output.

use crate::{Category, EmitError, EmitResult, EmitterConfig, ParseError, error::Location};
use fast_yaml_core::{
    EmitError as CoreEmitError, NormalizedInput, ParseError as CoreParseError, SourcePosition,
    position::ByteCursor,
};

/// Format YAML using the guarded heap-based event formatter.
pub fn format_streaming(input: &str, config: &EmitterConfig) -> EmitResult<String> {
    validate_stream(input)?;
    fast_yaml_core::streaming::format_streaming(input, &config.upstream()?)
        .map_err(|error| format_error(input, error))
}

/// Format YAML with an arena for temporary parser state.
#[cfg(feature = "arena")]
pub fn format_streaming_arena(input: &str, config: &EmitterConfig) -> EmitResult<String> {
    validate_stream(input)?;
    fast_yaml_core::streaming::format_streaming_arena(input, &config.upstream()?)
        .map_err(|error| format_error(input, error))
}

fn format_error(input: &str, error: CoreEmitError) -> EmitError {
    match error {
        CoreEmitError::DepthLimitExceeded { .. } => EmitError::Limit("formatter nesting depth"),
        CoreEmitError::AnchorLimitExceeded { .. } => EmitError::Limit("formatter anchor count"),
        CoreEmitError::TagLimitExceeded { .. } => EmitError::Limit("formatter tag bytes"),
        CoreEmitError::Parse(error) => {
            let at = format_location(input, error.position());
            let (category, message) = match error {
                CoreParseError::LimitExceeded { .. } => {
                    return ParseError::ResourceLimit {
                        limit: "formatter input budget",
                        at: Some(at),
                    }
                    .into();
                }
                CoreParseError::Syntax(_) => (Category::Syntax, "invalid YAML syntax"),
                CoreParseError::Merge { .. } => (Category::Data, "invalid YAML merge"),
                CoreParseError::SetValue { .. } => (Category::Data, "invalid YAML set member"),
                CoreParseError::Key { .. } => (Category::Data, "invalid YAML mapping key"),
                _ => (Category::Data, "could not format YAML input"),
            };
            ParseError::Syntax {
                at,
                category,
                message,
            }
            .into()
        }
        CoreEmitError::ComplexFlowKey => {
            EmitError::UnsupportedType("complex mapping key in flow style")
        }
        CoreEmitError::SetAsKey => EmitError::UnsupportedType("set used as a key"),
        _ => EmitError::Emit("could not format YAML stream"),
    }
}

fn format_location(input: &str, position: SourcePosition) -> Location {
    let offset = |text: &str| ByteCursor::new(text).offset(position);
    if !input.contains('\u{feff}') {
        return Location::at(input, offset(input));
    }
    if let Ok(normalized) = NormalizedInput::new(input) {
        return Location::at(
            input,
            normalized.original_offset(offset(normalized.as_str())),
        );
    }
    // Normalization failed at the first non-printable character. Its column
    // excludes stripped prefix BOMs, so the raw column is a lower bound.
    let start = offset(input);
    let extra = input[start..]
        .char_indices()
        .find_map(|(i, c)| {
            NormalizedInput::new(c.encode_utf8(&mut [0; 4]))
                .is_err()
                .then_some(i)
        })
        .unwrap_or(0);
    Location::at(input, start + extra)
}

/// Historical heuristic for callers choosing between streaming and DOM processing.
/// Returns false when more than one in a hundred bytes is an anchor or alias indicator.
#[must_use]
pub fn is_streaming_suitable(input: &str) -> bool {
    input.bytes().filter(|b| matches!(b, b'&' | b'*')).count() <= input.len() / 100
}

fn validate_stream(input: &str) -> EmitResult<()> {
    crate::Parser::validate_no_duplicate_mapping_keys(input).map_err(EmitError::from)
}
