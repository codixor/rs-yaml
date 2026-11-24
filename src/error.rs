use thiserror::Error;

/// Errors that can occur during YAML parsing.
#[derive(Error, Debug)]
pub enum ParseError {
    /// The compatibility loader reached a resource bound.
    #[error("YAML resource limit exceeded: {0}")]
    ResourceLimit(&'static str),

    /// The alias refers to an unknown or still-open anchor.
    #[error("YAML alias must refer to a completed anchor in this document")]
    UnresolvedAlias,

    /// YAML syntax error with location information.
    #[error("YAML parse error at line {line}, column {column}: {message}")]
    Syntax {
        /// Line number where the error occurred (1-indexed).
        line: usize,
        /// Column number where the error occurred (1-indexed).
        column: usize,
        /// Description of the syntax error.
        message: String,
    },

    /// Invalid float value encountered.
    #[error("invalid float value '{value}': {source}")]
    InvalidFloat {
        /// The invalid float value string.
        value: String,
        /// The underlying parse error.
        #[source]
        source: std::num::ParseFloatError,
    },

    /// YAML scanner error from saphyr.
    #[error("YAML scanner error: {0}")]
    Scanner(#[from] saphyr::ScanError),

    /// Explicit tag coercion failed.
    #[error("invalid explicit YAML tag {tag} for value {value}")]
    InvalidTaggedValue {
        /// The explicit tag that failed.
        tag: String,
        /// The scalar representation that could not be coerced.
        value: String,
    },

    /// YAML merge key (`<<`) used an unsupported value shape.
    #[error("invalid YAML merge key value: expected mapping or sequence of mappings, found {kind}")]
    InvalidMergeValue {
        /// The merge value kind that was encountered.
        kind: &'static str,
    },

    /// Duplicate mapping key detected.
    #[error(
        "duplicate YAML mapping key '{key}' first defined at line {first_line}, column {first_column}, duplicated at line {duplicate_line}, column {duplicate_column}"
    )]
    DuplicateKey {
        /// Duplicate key text.
        key: String,
        /// First key line.
        first_line: usize,
        /// First key column.
        first_column: usize,
        /// Duplicate key line.
        duplicate_line: usize,
        /// Duplicate key column.
        duplicate_column: usize,
    },
}

/// Errors that can occur during YAML emission.
#[derive(Error, Debug)]
pub enum EmitError {
    /// General emission error.
    #[error("failed to emit YAML: {0}")]
    Emit(String),

    /// Attempted to serialize an unsupported type.
    #[error("unsupported type for serialization: {0}")]
    UnsupportedType(String),
}

/// Result type for parsing operations.
pub type ParseResult<T> = std::result::Result<T, ParseError>;

/// Result type for emission operations.
pub type EmitResult<T> = std::result::Result<T, EmitError>;
