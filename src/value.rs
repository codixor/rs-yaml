pub use saphyr::MappingOwned as Map;
pub use saphyr::MarkedYamlOwned as MarkedValue;
pub use saphyr::ScalarOwned;
pub use saphyr::YamlDataOwned;
/// Wrapper around saphyr's `YamlOwned` type for consistent API.
///
/// This re-exports the saphyr types to provide a stable API
/// that can be extended in the future without breaking changes.
/// We use `YamlOwned` instead of `Yaml` to avoid lifetime parameters.
pub use saphyr::YamlOwned as Value;
pub use saphyr_parser::{Marker, Span};

/// Re-export `OrderedFloat` for users working with YAML float values.
///
/// This is used internally by saphyr for float comparison in mappings.
pub use ordered_float::OrderedFloat;

/// Type alias for YAML arrays.
pub type Array = Vec<Value>;
