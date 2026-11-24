//! `rs-yaml` is the workspace YAML parser and emitter crate.
//!
//! The public surface stays intentionally small:
//! - value-first parsing
//! - configurable emission
//! - typed `thiserror` errors
//! - additive streaming formatting behind a feature
//!
//! # YAML 1.2.2 Compliance
//!
//! This library implements the YAML 1.2.2 specification with the Core Schema:
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
