use crate::load_guard::{self, Documents};
use crate::{EmitError, EmitResult, ParseError, ParseResult};
use saphyr_parser::{BufferedInput, Event, Parser, Tag};
use serde_core::{Deserialize, Serialize};
use serde_saphyr::Error as DeError;

/// Deserialize one YAML document. Duplicate keys, a second document and tags outside
/// the core schema are errors; booleans are `true`/`false` only (no `yes`/`on`).
/// Diagnostics can name an unknown mapping key and expected types. Input values are redacted.
pub fn from_str<'de, T: Deserialize<'de>>(input: &'de str) -> ParseResult<T> {
    load_guard::load(input, Documents::Single, |event, span| {
        let tag = match &event {
            Event::Scalar(_, _, _, tag)
            | Event::MappingStart(_, tag)
            | Event::SequenceStart(_, tag) => tag.as_deref(),
            _ => None,
        };
        if tag.is_some_and(|tag| !is_core_tag(tag)) {
            return Err(ParseError::UnsupportedTag {
                line: span.start.line(),
                column: span.start.col() + 1,
            });
        }
        Ok(())
    })?;
    let options = serde_saphyr::options! {
        with_snippet: false,
        strict_booleans: true,
    };
    serde_saphyr::from_str_with_options(input, options).map_err(|error| {
        let message = describe(&error, input);
        match error.location() {
            Some(location) => ParseError::Syntax {
                line: location.line() as usize,
                column: location.column() as usize,
                message,
            },
            None => ParseError::Deserialize,
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

/// A custom visitor can use a rejected scalar as the field argument to
/// unknown_field. Only quote names observed as scalar keys in the validated input.
fn contains_field(input: &str, field: &str) -> bool {
    let mut parents = Vec::new();
    for item in Parser::new(BufferedInput::new(input.chars())) {
        let Ok((event, _)) = item else { return false };
        let collection = match &event {
            Event::MappingEnd | Event::SequenceEnd => {
                parents.pop();
                continue;
            }
            Event::MappingStart(..) => Some(Some(true)),
            Event::SequenceStart(..) => Some(None),
            Event::Scalar(..) | Event::Alias(_) => None,
            _ => continue,
        };
        if let Some(Some(next_is_key)) = parents.last_mut() {
            if *next_is_key && matches!(event, Event::Scalar(ref text, ..) if text == field) {
                return true;
            }
            *next_is_key = !*next_is_key;
        }
        if let Some(parent) = collection {
            parents.push(parent);
        }
    }
    false
}

fn describe(error: &DeError, input: &str) -> String {
    match error {
        DeError::SerdeUnknownField {
            field, expected, ..
        } => {
            if contains_field(input, field) {
                format!(
                    "unknown field `{}`, expected {}",
                    field.escape_debug(),
                    one_of(expected)
                )
            } else {
                format!("unknown field, expected {}", one_of(expected))
            }
        }
        DeError::SerdeMissingField { field, .. } => format!("missing field `{field}`"),
        DeError::SerdeInvalidType {
            unexpected,
            expected,
            ..
        } => {
            format!(
                "invalid type: found {}, expected {}",
                kind(unexpected),
                expected_type(expected)
            )
        }
        DeError::SerdeInvalidValue {
            unexpected,
            expected,
            ..
        } => {
            format!(
                "invalid value: found {}, expected {}",
                kind(unexpected),
                expected_type(expected)
            )
        }
        DeError::SerdeInvalidLength { len, expected, .. } => {
            format!("invalid length {len}, expected {}", expected_type(expected))
        }
        DeError::SerdeUnknownVariant { expected, .. } => {
            format!("unknown variant, expected {}", one_of(expected))
        }
        DeError::InvalidBooleanStrict { .. } => "invalid boolean: write true or false".into(),
        DeError::InvalidScalar { ty, .. } => format!("invalid value for {ty}"),
        DeError::NullIntoString { .. } => "null where a string is expected".into(),
        DeError::Eof { .. } => "unexpected end of input".into(),
        DeError::Unexpected { expected, .. } => format!("expected {expected}"),
        DeError::Budget { .. } => "YAML resource limit exceeded".into(),
        _ => "cannot deserialize YAML into the requested type".into(),
    }
}

// Serde's Expected formatter can be implemented by the caller and can include input.
fn expected_type(description: &str) -> &str {
    match description {
        "bool" | "a boolean" | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16"
        | "u32" | "u64" | "u128" | "usize" | "f32" | "f64" | "a string" | "a borrowed string"
        | "a character" | "a sequence" | "a map" | "unit" | "option" => description,
        _ => "the requested type",
    }
}

/// The kind word of serde's `Unexpected` display (`string "…"` → `string`).
fn kind(unexpected: &str) -> &str {
    let word = unexpected.split([' ', '"', '`', '\'']).next().unwrap_or("");
    match word {
        "boolean" | "integer" | "floating" | "char" | "string" | "byte" | "unit" | "Option"
        | "newtype" | "sequence" | "map" | "enum" | "null" => word,
        _ => "value",
    }
}

fn one_of(expected: &[&'static str]) -> String {
    match expected {
        [] => "no fields".into(),
        [one] => format!("`{one}`"),
        many => {
            let list: Vec<String> = many.iter().map(|e| format!("`{e}`")).collect();
            format!("one of {}", list.join(", "))
        }
    }
}

/// Serialize one YAML document. The same key, document and resource checks apply to
/// the resulting string, including mappings emitted by custom serializers.
pub fn to_string<T: Serialize + ?Sized>(value: &T) -> EmitResult<String> {
    let yaml = serde_saphyr::to_string(&value)
        .map_err(|_| EmitError::Emit("could not serialize value as YAML".into()))?;
    load_guard::validate(&yaml, Documents::Single)
        .map_err(|error| EmitError::Emit(error.to_string()))?;
    Ok(yaml)
}
