use crate::load_guard::{self, Documents};
use crate::{
    Category, EmitError, EmitResult, Limits, ParseError, ParseResult, YAML_LIMITS, error::Location,
};
use saphyr_parser::{Event, Marker, Tag};
use serde_core::{Deserialize, Serialize};
use serde_saphyr::Error as DeError;

/// Deserialize one YAML document. Duplicate keys, a second document and tags outside
/// the core schema are errors; booleans are `true`/`false` only (no `yes`/`on`).
/// Diagnostics omit input keys and values.
pub fn from_str<'de, T: Deserialize<'de>>(input: &'de str) -> ParseResult<T> {
    from_str_with_limits(input, &YAML_LIMITS)
}

/// Deserialize one YAML document after checking the caller's inclusive bounds.
pub fn from_str_with_limits<'de, T: Deserialize<'de>>(
    input: &'de str,
    limits: &Limits,
) -> ParseResult<T> {
    load_guard::load(input, Documents::Single, limits, |event, span| {
        let tag = match &event {
            Event::Scalar(_, _, _, tag)
            | Event::MappingStart(_, tag)
            | Event::SequenceStart(_, tag) => tag.as_deref(),
            _ => None,
        };
        if tag.is_some_and(|tag| !is_core_tag(tag)) {
            return Err(ParseError::UnsupportedTag {
                at: Location::marker(input, span.start),
            });
        }
        Ok(())
    })?;
    let options = serde_saphyr::options! {
        with_snippet: false,
        strict_booleans: true,
    };
    serde_saphyr::from_str_with_options(input, options).map_err(|error| {
        let (category, message) = describe(&error);
        let at = error.location().and_then(|loc| {
            if let Some(byte) = loc
                .span()
                .byte_offset()
                .and_then(|n| usize::try_from(n).ok())
                && byte <= input.len()
            {
                return Some(Location::at(input, byte));
            }
            let column = usize::try_from(loc.column()).ok()?.checked_sub(1)?;
            let line = usize::try_from(loc.line()).ok()?;
            (line > 0).then(|| Location::marker(input, Marker::new(0, line, column)))
        });
        ParseError::Deserialize {
            category,
            at,
            message,
        }
    })
}

// A %TAG directive or verbatim spelling can split the same URI at a different point.
fn is_core_tag(tag: &Tag) -> bool {
    [
        "tag:yaml.org,2002:null",
        "tag:yaml.org,2002:bool",
        "tag:yaml.org,2002:int",
        "tag:yaml.org,2002:float",
        "tag:yaml.org,2002:str",
        "tag:yaml.org,2002:seq",
        "tag:yaml.org,2002:map",
    ]
    .iter()
    .any(|uri| uri.strip_prefix(&tag.handle) == Some(tag.suffix.as_str()))
}

// All descriptions are static. A custom visitor can populate even schema-field and
// expected-type diagnostics with input-derived text, including leaked &'static str.
fn describe(error: &DeError) -> (Category, &'static str) {
    let message = match error {
        DeError::SerdeUnknownField { .. } => "unknown field",
        DeError::SerdeMissingField { .. } => "missing field",
        DeError::SerdeUnknownVariant { .. } => "unknown variant",
        DeError::SerdeInvalidType { .. } => "invalid type",
        DeError::SerdeInvalidValue { .. } => "invalid value",
        DeError::SerdeInvalidLength { .. } => "invalid length",
        DeError::InvalidBooleanStrict { .. } => "invalid boolean: write true or false",
        DeError::InvalidScalar { ty, .. } => match *ty {
            "bool" => "invalid value for bool",
            "i8" => "invalid value for i8",
            "i16" => "invalid value for i16",
            "i32" => "invalid value for i32",
            "i64" => "invalid value for i64",
            "i128" => "invalid value for i128",
            "u8" => "invalid value for u8",
            "u16" => "invalid value for u16",
            "u32" => "invalid value for u32",
            "u64" => "invalid value for u64",
            "u128" => "invalid value for u128",
            "f32" => "invalid value for f32",
            "f64" => "invalid value for f64",
            _ => "invalid scalar value",
        },
        DeError::NullIntoString { .. } => "null where a string is expected",
        DeError::Eof { .. } => return (Category::Eof, "unexpected end of input"),
        DeError::Budget { .. } => return (Category::Limit, "YAML resource limit exceeded"),
        _ => "cannot deserialize YAML into the requested type",
    };
    (Category::Data, message)
}

/// Serialize one YAML document. The same key, document and resource checks apply to
/// the resulting string, including mappings emitted by custom serializers.
pub fn to_string<T: Serialize + ?Sized>(value: &T) -> EmitResult<String> {
    let yaml = serde_saphyr::to_string(&value)
        .map_err(|_| EmitError::Emit("could not serialize value as YAML"))?;
    load_guard::validate(&yaml, Documents::Single, &YAML_LIMITS)?;
    Ok(yaml)
}
