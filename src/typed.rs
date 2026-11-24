use crate::load_guard::{self, Documents};
use crate::{EmitError, EmitResult, ParseError, ParseResult};
use serde_core::{Deserialize, Serialize};

/// Deserialize one YAML document. Duplicate keys and a second document are errors.
/// Input text and custom Serde error messages are omitted from diagnostics.
pub fn from_str<'de, T: Deserialize<'de>>(input: &'de str) -> ParseResult<T> {
    load_guard::validate(input, Documents::Single)?;
    let options = serde_saphyr::options! { with_snippet: false };
    serde_saphyr::from_str_with_options(input, options).map_err(|error| {
        if let Some(location) = error.location() {
            ParseError::Syntax {
                line: location.line() as usize,
                column: location.column() as usize,
                message: "cannot deserialize YAML into the requested type".into(),
            }
        } else {
            ParseError::Deserialize
        }
    })
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
