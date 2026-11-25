use crate::{Category, FormatError, Position};
use saphyr_parser::Marker;
use std::{fmt, num::NonZeroU32};
use thiserror::Error;

/// Known source coordinates, with zero-based offsets and one-based byte columns.
#[derive(Debug, Copy, Clone)]
pub struct Location(pub Position);

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line) = self.0.line {
            write!(f, "line {line}")?;
            if let Some(column) = self.0.column {
                write!(f, ", column {column}")?;
            }
            Ok(())
        } else if let Some(offset) = self.0.offset {
            write!(f, "byte {offset}")
        } else {
            f.write_str("unknown location")
        }
    }
}

impl Location {
    pub(crate) fn at(input: &str, offset: usize) -> Self {
        let bytes = input.as_bytes();
        let mut start = 0;
        let mut line = 1u32;
        for (i, &b) in bytes[..offset].iter().enumerate() {
            if b == b'\n' || (b == b'\r' && bytes.get(i + 1) != Some(&b'\n')) {
                line += 1;
                start = i + 1;
            }
        }
        Self(Position {
            offset: Some(offset as u64),
            line: NonZeroU32::new(line),
            column: u32::try_from(offset - start + 1)
                .ok()
                .and_then(NonZeroU32::new),
        })
    }

    pub(crate) fn marker(input: &str, marker: Marker) -> Self {
        let offset = crate::source::offset(input, marker);
        Self::at(input, offset)
    }
}

/// Errors returned by guarded YAML loading.
#[derive(Error, Debug)]
pub enum ParseError {
    /// An input, materialization, or backend budget was exceeded.
    #[error("YAML resource limit exceeded: {limit}")]
    ResourceLimit {
        limit: &'static str,
        at: Option<Location>,
    },
    /// The second document in a single-document API.
    #[error("unexpected second YAML document at {at}")]
    MultipleDocuments { at: Location },
    /// The typed loader permits only the seven YAML core tags.
    #[error("unsupported YAML tag at {at}: only core schema tags are accepted")]
    UnsupportedTag { at: Location },
    /// The Serde backend rejected the document or requested type.
    #[cfg(feature = "serde")]
    #[error("{message}")]
    Deserialize {
        category: Category,
        at: Option<Location>,
        message: &'static str,
    },
    /// An alias requires a completed anchor.
    #[error("YAML alias at {at} must refer to a completed anchor in this document")]
    UnresolvedAlias { at: Location },
    /// A scanner diagnostic with its input text discarded.
    #[error("YAML parse error at {at}: {message}")]
    Syntax {
        at: Location,
        category: Category,
        message: &'static str,
    },
    /// Invalid floating-point representation.
    #[error("invalid YAML float")]
    InvalidFloat,
    /// Explicit core tag coercion failed.
    #[error("invalid value for explicit YAML tag {tag}")]
    InvalidTaggedValue { tag: &'static str },
    /// The value of a merge key must contain mappings.
    #[error("invalid YAML merge key value: expected mapping or sequence of mappings, found {kind}")]
    InvalidMergeValue { kind: &'static str },
    /// Distinct source keys collapse to one key in the owned representation.
    #[error("mapping keys resolve to the same YAML value")]
    ConflictingKeys,
    /// A duplicate mapping key and the location of its original definition.
    #[error("duplicate YAML mapping key first defined at {first}, duplicated at {duplicate}")]
    DuplicateKey {
        first: Location,
        duplicate: Location,
    },
}

impl ParseError {
    pub(crate) fn limit(limit: &'static str) -> Self {
        Self::ResourceLimit { limit, at: None }
    }

    pub(crate) fn from_input(error: &saphyr::ScanError, input: &str) -> Self {
        let at = Location::marker(input, *error.marker());
        if at.0.offset == Some(input.len() as u64)
            && matches!(
                error.info(),
                "while parsing a node, did not find expected node content"
                    | "while parsing a flow sequence, expected ',' or ']'"
                    | "while parsing a flow mapping, did not find expected ',' or '}'"
            )
        {
            return Self::Syntax {
                at,
                category: Category::Eof,
                message: "unexpected end of input",
            };
        }
        Self::scanner(error, at)
    }

    fn scanner(error: &saphyr::ScanError, at: Location) -> Self {
        let reason = match error.info() {
            "recursion limit exceeded" => Some("flow collection nesting"),
            "alias expansion exceeded the maximum node budget (possible billion-laughs attack)" => {
                Some("alias expansion node budget")
            }
            _ => None,
        };
        if let Some(limit) = reason {
            return Self::ResourceLimit {
                limit,
                at: Some(at),
            };
        }
        let (category, message) = match error.info() {
            "while scanning a quoted scalar, found unexpected end of stream" => {
                (Category::Eof, "unexpected end of quoted scalar")
            }
            "while parsing node, found unknown anchor" => (Category::Data, "unknown anchor"),
            _ => (Category::Syntax, "invalid YAML syntax"),
        };
        Self::Syntax {
            at,
            category,
            message,
        }
    }
}

impl From<saphyr::ScanError> for ParseError {
    fn from(error: saphyr::ScanError) -> Self {
        // Without input text, a character column cannot be normalized to UTF-8 bytes.
        let line = u32::try_from(error.marker().line())
            .ok()
            .and_then(NonZeroU32::new);
        Self::scanner(
            &error,
            Location(Position {
                line,
                column: None,
                offset: None,
            }),
        )
    }
}

impl FormatError for ParseError {
    fn category(&self) -> Category {
        match self {
            Self::ResourceLimit { .. } => Category::Limit,
            Self::Syntax { category, .. } => *category,
            #[cfg(feature = "serde")]
            Self::Deserialize { category, .. } => *category,
            _ => Category::Data,
        }
    }

    fn position(&self) -> Option<Position> {
        match self {
            Self::ResourceLimit { at, .. } => at.map(|a| a.0),
            #[cfg(feature = "serde")]
            Self::Deserialize { at, .. } => at.map(|a| a.0),
            Self::MultipleDocuments { at }
            | Self::UnresolvedAlias { at }
            | Self::UnsupportedTag { at }
            | Self::Syntax { at, .. } => Some(at.0),
            Self::DuplicateKey { duplicate, .. } => Some(duplicate.0),
            _ => None,
        }
        .filter(|p| p.offset.is_some() || p.line.is_some() || p.column.is_some())
    }
}

/// Errors returned by YAML emission and formatting.
#[derive(Error, Debug)]
pub enum EmitError {
    #[error("failed to emit YAML: {0}")]
    Emit(&'static str),
    #[error("unsupported type for serialization: {0}")]
    UnsupportedType(&'static str),
    #[error("YAML emission limit exceeded: {0}")]
    Limit(&'static str),
    #[error(transparent)]
    Parse(#[from] ParseError),
}

impl FormatError for EmitError {
    fn category(&self) -> Category {
        match self {
            Self::Parse(error) => error.category(),
            Self::Limit(_) => Category::Limit,
            _ => Category::Data,
        }
    }

    fn position(&self) -> Option<Position> {
        if let Self::Parse(error) = self {
            error.position()
        } else {
            None
        }
    }
}

pub type ParseResult<T> = std::result::Result<T, ParseError>;
pub type EmitResult<T> = std::result::Result<T, EmitError>;
