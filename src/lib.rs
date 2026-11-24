#![forbid(unsafe_code)]
//! `rs-yaml` parses, emits and formats YAML.
//!
//! The public surface stays intentionally small:
//! - value-first parsing
//! - configurable emission
//! - typed `thiserror` errors
//! - additive streaming formatting behind a feature
//!
//! # YAML 1.2.2 Compliance
//!
//! The parser supports YAML core-schema scalars:
//!
//! - **Null**: `~`, `null`, `Null`, `NULL`, or empty value
//! - **Boolean**: `true`/`false` (case-insensitive) - NOT yes/no/on/off (YAML 1.1)
//! - **Integer**: Decimal, `0o` octal, `0x` hexadecimal
//! - **Float**: Standard notation, `.inf`, `-.inf`, `.nan`
//! - **String**: Plain, single-quoted, double-quoted, literal (`|`), folded (`>`)
//!
//! # Examples
//!
//! Parsing YAML:
//!
//! ```
//! use rs_yaml::Parser;
//!
//! let yaml = "name: test\nvalue: 123";
//! let doc = Parser::parse_str(yaml)?;
//! assert!(doc.is_mapping());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Emitting YAML:
//!
//! ```
//! use rs_yaml::{Emitter, ScalarOwned, Value};
//!
//! let value = Value::Value(ScalarOwned::String("test".to_string()));
//! let yaml = Emitter::emit_str(&value)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod keys;
mod load_guard;
#[cfg(feature = "serde")]
mod typed;
#[cfg(feature = "serde")]
pub use typed::{from_str, to_string};

/// YAML emitter for serializing documents to strings.
pub mod emitter;
/// Error types for parsing and emitting operations.
pub mod error;
/// YAML parser for deserializing strings to documents.
pub mod parser;
/// Value types representing YAML data structures.
pub mod value;

/// Streaming YAML formatter module.
///
/// Provides high-performance formatting by processing parser events directly
/// without building an intermediate DOM representation.
#[cfg(feature = "streaming")]
pub mod streaming;

pub use emitter::{Emitter, EmitterConfig};
pub use error::{EmitError, EmitResult, ParseError, ParseResult};
pub use parser::{Parser, canonicalize, safe_load, safe_load_all};
pub use value::{
    Array, Map, MarkedValue, Marker, OrderedFloat, ScalarOwned, Span, Value, YamlDataOwned,
};

/// Upstream Rust API, including budgeted loading, events, scalar resolution and bounded emission.
/// Its resolved value type is distinct from the span/style-preserving compatibility types.
pub use fast_yaml_core as fast;
pub use fast_yaml_core::{
    MaxScanAhead, ParseLimits, decode_input, decode_input_owned, find_comments, has_comments,
};

/// Source-aware linting, configuration, rules and diagnostic formatters.
#[cfg(feature = "lint")]
pub use fast_yaml_linter as lint;

/// Parallel stream parsing, file processing and atomic file output.
#[cfg(feature = "parallel")]
pub use fast_yaml_parallel as parallel;

/// Emit one compatibility value with default options.
pub fn safe_dump(value: &Value) -> EmitResult<String> {
    Emitter::emit_str(value)
}

/// Emit a stream of compatibility values with document separators.
pub fn safe_dump_all(values: &[Value]) -> EmitResult<String> {
    Emitter::emit_all(values)
}
