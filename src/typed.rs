use crate::load_guard::{self, Documents};
use crate::{EmitError, EmitResult, ParseError, ParseResult};
use saphyr_parser::{Event, Tag};
use serde_core::{Deserialize, Serialize};
use serde_saphyr::Error as DeError;

/// Deserialize one YAML document. Duplicate keys, a second document and tags outside
/// the core schema are errors; booleans are `true`/`false` only (no `yes`/`on`).
/// Diagnostics name fields and expected types from the program, never input values.
pub fn from_str<'de, T: Deserialize<'de>>(input: &'de str) -> ParseResult<T> {
    load_guard::load(input, Documents::Single, |event, span| {
        let tag = match &event {
            Event::Scalar(_, _, _, tag)
            | Event::MappingStart(_, tag)
            | Event::SequenceStart(_, tag) => tag.as_deref(),
            _ => None,
        };
        if tag.is_some_and(|tag: &Tag| !tag.is_yaml_core_schema()) {
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
        let message = describe(&error);
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

/// A diagnostic built only from program-side text: field names, expected types and
/// fixed reasons. Unexpected values, unknown variants and custom messages are dropped,
/// since a `deserialize_with` function may format the value it rejected.
fn describe(error: &DeError) -> String {
    match error {
        DeError::SerdeUnknownField {
            field, expected, ..
        } => {
            // Keys are quoted, as rs-toml does: they name the setting the user misspelled.
            format!("unknown field `{field}`, expected {}", one_of(expected))
        }
        DeError::SerdeMissingField { field, .. } => format!("missing field `{field}`"),
        DeError::SerdeInvalidType {
            unexpected,
            expected,
            ..
        } => {
            format!(
                "invalid type: found {}, expected {expected}",
                kind(unexpected)
            )
        }
        DeError::SerdeInvalidValue {
            unexpected,
            expected,
            ..
        } => {
            format!(
                "invalid value: found {}, expected {expected}",
                kind(unexpected)
            )
        }
        DeError::SerdeInvalidLength { len, expected, .. } => {
            format!("invalid length {len}, expected {expected}")
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
