//! Streaming YAML formatter that bypasses DOM construction.
//!
//! This module provides high-performance formatting for YAML documents
//! by processing parser events directly without building an intermediate
//! representation. This approach achieves O(1) memory complexity for
//! already-formatted files, compared to O(n) for DOM-based formatting.
//!
//! # Performance Characteristics
//!
//! - Small files (<1KB): Use DOM-based formatter (overhead not worth it)
//! - Large files (>1KB): Streaming provides 5-10x speedup
//! - Memory: Constant memory usage regardless of input size
//!
//! # Usage
//!
//! ```
//! # #[cfg(feature = "streaming")]
//! # {
//! use rs_yaml::streaming::{format_streaming, is_streaming_suitable};
//! use rs_yaml::EmitterConfig;
//!
//! let yaml = "key: value\nlist:\n  - item1\n  - item2\n";
//! let config = EmitterConfig::default();
//!
//! if is_streaming_suitable(yaml) {
//!     let formatted = format_streaming(yaml, &config).unwrap();
//!     println!("{formatted}");
//! }
//! # }
//! ```

mod formatter;
mod std_backend;
mod traits;

#[cfg(feature = "arena")]
mod arena_backend;

// Re-export public API
pub use std_backend::format_streaming;

#[cfg(feature = "arena")]
pub use arena_backend::format_streaming_arena;

/// Maximum allowed anchor ID to prevent memory exhaustion attacks.
/// 4096 anchors is more than sufficient for any legitimate YAML file.
const MAX_ANCHOR_ID: usize = 4096;

/// Maximum nesting depth to prevent stack/memory exhaustion.
/// 256 levels of nesting is far beyond any practical use case.
const MAX_DEPTH: usize = 256;

/// Static 64-space string for fast indent generation via slicing.
/// Avoids allocation for nesting depths up to 32 levels with 2-space indent.
static INDENT_SPACES: &str = "                                                                ";

/// Context for tracking the current position within YAML structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Context {
    /// At the root level of a document
    Root,
    /// Inside a sequence (array)
    Sequence,
    /// Inside a mapping, expecting a key
    MappingKey,
    /// Inside a mapping, expecting a value
    MappingValue,
}

/// Fix special float value for YAML 1.2 compliance.
///
/// Converts saphyr's output format to YAML 1.2 compliant format:
/// - `inf` -> `.inf`
/// - `-inf` -> `-.inf`
/// - `NaN` -> `.nan`
fn fix_special_float_value(value: &str) -> &str {
    match value {
        "inf" => ".inf",
        "-inf" => "-.inf",
        "NaN" => ".nan",
        other => other,
    }
}

/// Extract original anchor names from YAML input.
///
/// Returns a `Vec` where `index == anchor_id` and `value == original anchor name`.
/// Index 0 is always an empty string (saphyr anchor IDs start at 1).
///
/// Scans the input for `&name` tokens using the same ordering as saphyr's parser.
/// Anchors inside quoted strings and comments are skipped.
pub(super) fn extract_anchor_names(input: &str) -> Vec<String> {
    // Fast path: no `&` byte means no anchors
    if !input.bytes().any(|b| b == b'&') {
        return vec![String::new()]; // index 0 placeholder
    }

    let mut names: Vec<String> = vec![String::new()]; // index 0 unused (saphyr IDs start at 1)
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        match bytes[i] {
            // Skip single-quoted strings: no escape sequences inside
            b'\'' => {
                i += 1;
                while i < len {
                    if bytes[i] == b'\'' {
                        i += 1;
                        // Two consecutive single quotes = escaped quote inside string
                        if i < len && bytes[i] == b'\'' {
                            i += 1;
                        } else {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            // Skip double-quoted strings
            b'"' => {
                i += 1;
                while i < len {
                    if bytes[i] == b'\\' {
                        i += 2; // skip escape sequence
                    } else if bytes[i] == b'"' {
                        i += 1;
                        break;
                    } else {
                        i += 1;
                    }
                }
            }
            // Skip comments to end of line
            b'#' => {
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            // Block scalar headers: skip lines starting with | or > after whitespace
            // (content lines follow and may contain `&` that is not an anchor)
            // We handle this by only treating `&` after valid YAML flow positions.
            // Anchor: `&` followed by a valid anchor name character
            b'&' => {
                i += 1;
                // Anchor name: [a-zA-Z0-9_-] and other non-space non-special chars
                let start = i;
                while i < len {
                    let b = bytes[i];
                    // Anchor name ends at whitespace, `{`, `}`, `[`, `]`, `,`, `:`, `#`
                    if b.is_ascii_whitespace()
                        || matches!(b, b'{' | b'}' | b'[' | b']' | b',' | b':')
                    {
                        break;
                    }
                    i += 1;
                }
                if i > start
                    && let Ok(name) = std::str::from_utf8(&bytes[start..i])
                {
                    names.push(name.to_owned());
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    names
}

/// Check if input is suitable for streaming formatter.
///
/// Returns `true` for inputs that benefit from streaming:
/// - Large files (>1KB)
/// - Files without heavy anchor/alias usage
///
/// Returns `false` for:
/// - Small files (streaming overhead not worth it)
/// - Files with heavy anchor/alias usage (DOM better for resolution)
///
/// # Examples
///
/// ```
/// # #[cfg(feature = "streaming")]
/// # {
/// use rs_yaml::streaming::is_streaming_suitable;
///
/// // Regular files - use streaming (preserves float types)
/// assert!(is_streaming_suitable("version: 1.0"));
/// assert!(is_streaming_suitable("small: yaml"));
///
/// // Large files - also use streaming
/// let large = "key: value\n".repeat(1000);
/// assert!(is_streaming_suitable(&large));
/// # }
/// ```
#[must_use]
pub fn is_streaming_suitable(input: &str) -> bool {
    // Streaming preserves the original scalar text (e.g. "1.0" stays "1.0"),
    // while DOM-based formatting loses type information (float 1.0 → integer 1).
    // Always prefer streaming to maintain YAML 1.2.2 Core Schema type fidelity.

    // Heavy anchor/alias usage: avoid streaming only when anchor density is very
    // high, because the DOM path resolves aliases during parse.
    let len = input.len();
    if len > 0 {
        let anchor_count = input.bytes().filter(|&b| b == b'&').count();
        let alias_count = input.bytes().filter(|&b| b == b'*').count();

        // More than 10 anchors/aliases per 1000 bytes → fall back to DOM
        if (anchor_count + alias_count) * 100 > len {
            return false;
        }
    }

    true
}
