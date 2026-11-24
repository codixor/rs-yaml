use std::{borrow::Cow, collections::BTreeMap};

use crate::error::{ParseError, ParseResult};
use crate::value::{MarkedValue, Value, YamlDataOwned};
use saphyr::{MappingOwned, ScalarOwned, Tag, YamlLoader, parse_core_schema_fp};
use saphyr_parser::{BufferedInput, Event, Parser as SaphyrParser, Span, SpannedEventReceiver};

/// Parser for YAML documents.
///
/// Wraps saphyr's YAML loading to provide a consistent API.
#[derive(Debug)]
pub struct Parser;

impl Parser {
    /// Parse a single YAML document from a string.
    ///
    /// Returns the first document if multiple are present.
    ///
    /// Truly empty input and comment-only input are treated as valid YAML null
    /// documents and return a YAML null scalar value.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if the YAML syntax is invalid or if an explicit
    /// tag/merge-key coercion fails during normalization.
    ///
    /// # Examples
    ///
    /// ```
    /// use rs_yaml::Parser;
    ///
    /// let result = Parser::parse_str("name: test\nvalue: 123")?;
    /// assert!(result.is_mapping());
    ///
    /// let empty = Parser::parse_str("# comments only")?;
    /// assert!(matches!(empty, rs_yaml::Value::Value(rs_yaml::ScalarOwned::Null)));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn parse_str(input: &str) -> ParseResult<Value> {
        Ok(Self::parse_all(input)?
            .into_iter()
            .next()
            .unwrap_or(Value::Value(ScalarOwned::Null)))
    }

    /// Parse all YAML documents from a string.
    ///
    /// Returns a vector of all documents found in the input.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if the YAML syntax is invalid or if an explicit
    /// tag/merge-key coercion fails during normalization.
    ///
    /// # Examples
    ///
    /// ```
    /// use rs_yaml::Parser;
    ///
    /// let docs = Parser::parse_all("---\nfoo: 1\n---\nbar: 2")?;
    /// assert_eq!(docs.len(), 2);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn parse_all(input: &str) -> ParseResult<Vec<Value>> {
        load_documents_preserving_styles(input)?
            .into_iter()
            .map(normalize_value)
            .collect()
    }

    /// Validates that mappings do not contain duplicate scalar keys.
    ///
    /// This is an explicit validation pass because the normal YAML value tree is
    /// key-indexed and cannot represent duplicate keys after loading.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::DuplicateKey`] with the first and duplicate key
    /// positions when a duplicate scalar mapping key is detected.
    pub fn validate_no_duplicate_mapping_keys(input: &str) -> ParseResult<()> {
        reject_duplicate_mapping_keys(input)
    }

    /// Parse a single YAML document while preserving source spans.
    ///
    /// Returns the first document if multiple are present. Truly empty input
    /// and comment-only input are treated as valid YAML null documents.
    ///
    /// This API intentionally returns the marked parser tree rather than the
    /// normalized [`Value`] tree so diagnostics can point back to the original
    /// source positions.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if the YAML syntax is invalid.
    pub fn parse_marked_str(input: &str) -> ParseResult<MarkedValue> {
        Ok(Self::parse_all_marked(input)?
            .into_iter()
            .next()
            .unwrap_or_else(|| MarkedValue::from(YamlDataOwned::Value(ScalarOwned::Null))))
    }

    /// Parse all YAML documents while preserving source spans.
    ///
    /// The returned nodes preserve scalar representations and their spans.
    /// Use [`parse_all`](Self::parse_all) when canonical scalar conversion is
    /// desired instead.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] if the YAML syntax is invalid.
    pub fn parse_all_marked(input: &str) -> ParseResult<Vec<MarkedValue>> {
        load_marked_documents_preserving_styles(input)
    }

    /// Parse all YAML documents preserving scalar styles (literal `|`, folded `>`).
    ///
    /// Unlike [`parse_all`], this function uses `early_parse = false` in the loader,
    /// which keeps scalars as `Value::Representation` nodes with their original style
    /// information instead of resolving them eagerly.
    ///
    /// This is used by the format pipeline to preserve block scalar styles in output.
    ///
    /// # Errors
    ///
    /// Returns `ParseError::Scanner` if the YAML syntax is invalid.
    ///
    /// [`parse_all`]: Parser::parse_all
    pub fn parse_all_preserving_styles(input: &str) -> ParseResult<Vec<Value>> {
        load_documents_preserving_styles(input)
    }
}

/// Canonicalize mixed-case YAML 1.2.2 bool/null variants that saphyr leaves as strings.
///
/// saphyr handles lowercase `true`, `false`, `null`, `~` natively.
/// This function post-processes the tree to handle `True`, `TRUE`, `False`, `FALSE`, `Null`.
pub fn canonicalize(value: Value) -> Value {
    match value {
        Value::Value(ScalarOwned::String(ref s)) => match s.as_str() {
            "True" | "TRUE" => Value::Value(ScalarOwned::Boolean(true)),
            "False" | "FALSE" => Value::Value(ScalarOwned::Boolean(false)),
            "Null" | "NULL" => Value::Value(ScalarOwned::Null),
            _ => value,
        },
        Value::Sequence(seq) => Value::Sequence(seq.into_iter().map(canonicalize).collect()),
        Value::Mapping(map) => Value::Mapping(
            map.into_iter()
                .map(|(k, v)| (canonicalize(k), canonicalize(v)))
                .collect(),
        ),
        Value::Tagged(tag, inner) => Value::Tagged(tag, Box::new(canonicalize(*inner))),
        other => other,
    }
}

/// Convenience wrapper that mirrors common `safe_load` YAML APIs.
///
/// # Errors
///
/// Returns [`ParseError`] if parsing or normalization fails.
pub fn safe_load(input: &str) -> ParseResult<Value> {
    Parser::parse_str(input)
}

/// Convenience wrapper that mirrors common `safe_load_all` YAML APIs.
///
/// # Errors
///
/// Returns [`ParseError`] if parsing or normalization fails.
pub fn safe_load_all(input: &str) -> ParseResult<Vec<Value>> {
    Parser::parse_all(input)
}

fn normalize_value(value: Value) -> ParseResult<Value> {
    match value {
        Value::Representation(text, style, tag) => normalize_representation(text, style, tag),
        Value::Sequence(seq) => seq
            .into_iter()
            .map(normalize_value)
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Sequence),
        Value::Mapping(map) => normalize_mapping(map),
        Value::Tagged(tag, inner) => normalize_tagged(tag, *inner),
        other => Ok(canonicalize(other)),
    }
}

fn load_documents_preserving_styles(input: &str) -> ParseResult<Vec<Value>> {
    let mut saphyr_parser = SaphyrParser::new(BufferedInput::new(input.chars()));
    let mut loader = YamlLoader::<Value>::default();
    loader.early_parse(false);
    saphyr_parser.load(&mut loader, true)?;
    Ok(loader.into_documents())
}

fn load_marked_documents_preserving_styles(input: &str) -> ParseResult<Vec<MarkedValue>> {
    let mut saphyr_parser = SaphyrParser::new(BufferedInput::new(input.chars()));
    let mut loader = YamlLoader::<MarkedValue>::default();
    loader.early_parse(false);
    saphyr_parser.load(&mut loader, true)?;
    Ok(loader.into_documents())
}

fn reject_duplicate_mapping_keys(input: &str) -> ParseResult<()> {
    let mut saphyr_parser = SaphyrParser::new(BufferedInput::new(input.chars()));
    let mut detector = DuplicateKeyDetector::default();
    saphyr_parser.load(&mut detector, true)?;
    detector.finish()
}

#[derive(Debug, Default)]
struct DuplicateKeyDetector {
    containers: Vec<DuplicateKeyContainer>,
    duplicate: Option<ParseError>,
}

#[derive(Debug)]
enum DuplicateKeyContainer {
    Mapping {
        keys: BTreeMap<String, Span>,
        expecting_key: bool,
        role: Option<ParentRole>,
    },
    Sequence {
        role: Option<ParentRole>,
    },
}

#[derive(Debug, Clone, Copy)]
enum ParentRole {
    Key,
    Value,
}

impl DuplicateKeyDetector {
    fn finish(self) -> ParseResult<()> {
        if let Some(error) = self.duplicate {
            Err(error)
        } else {
            Ok(())
        }
    }

    fn current_parent_role(&self) -> Option<ParentRole> {
        match self.containers.last() {
            Some(DuplicateKeyContainer::Mapping { expecting_key, .. }) => {
                if *expecting_key {
                    Some(ParentRole::Key)
                } else {
                    Some(ParentRole::Value)
                }
            }
            Some(DuplicateKeyContainer::Sequence { .. }) | None => None,
        }
    }

    fn complete_node_in_parent(&mut self, role: Option<ParentRole>) {
        let Some(parent_role) = role else {
            return;
        };
        let Some(DuplicateKeyContainer::Mapping { expecting_key, .. }) = self.containers.last_mut()
        else {
            return;
        };
        *expecting_key = match parent_role {
            ParentRole::Key => false,
            ParentRole::Value => true,
        };
    }

    fn record_scalar(&mut self, value: Cow<'_, str>, span: Span) {
        let Some(DuplicateKeyContainer::Mapping {
            keys,
            expecting_key,
            ..
        }) = self.containers.last_mut()
        else {
            return;
        };

        if *expecting_key {
            if value != "<<" {
                if let Some(first) = keys.get(value.as_ref()) {
                    self.duplicate = Some(ParseError::DuplicateKey {
                        key: value.into_owned(),
                        first_line: first.start.line(),
                        first_column: first.start.col(),
                        duplicate_line: span.start.line(),
                        duplicate_column: span.start.col(),
                    });
                } else {
                    keys.insert(value.into_owned(), span);
                }
            }
            *expecting_key = false;
        } else {
            *expecting_key = true;
        }
    }
}

impl<'input> SpannedEventReceiver<'input> for DuplicateKeyDetector {
    fn on_event(&mut self, ev: Event<'input>, span: Span) {
        if self.duplicate.is_some() {
            return;
        }

        match ev {
            Event::MappingStart(..) => {
                let role = self.current_parent_role();
                self.containers.push(DuplicateKeyContainer::Mapping {
                    keys: BTreeMap::new(),
                    expecting_key: true,
                    role,
                });
            }
            Event::SequenceStart(..) => {
                let role = self.current_parent_role();
                self.containers
                    .push(DuplicateKeyContainer::Sequence { role });
            }
            Event::MappingEnd | Event::SequenceEnd => {
                let role = self.containers.pop().and_then(|container| match container {
                    DuplicateKeyContainer::Mapping { role, .. }
                    | DuplicateKeyContainer::Sequence { role } => role,
                });
                self.complete_node_in_parent(role);
            }
            Event::Scalar(value, ..) => self.record_scalar(value, span),
            Event::Alias(_) => {
                if let Some(role) = self.current_parent_role() {
                    self.complete_node_in_parent(Some(role));
                }
            }
            Event::Nothing
            | Event::StreamStart
            | Event::StreamEnd
            | Event::DocumentStart(_)
            | Event::DocumentEnd => {}
        }
    }
}

fn normalize_representation(
    text: String,
    style: saphyr::ScalarStyle,
    tag: Option<Tag>,
) -> ParseResult<Value> {
    if let Some(tag) = tag {
        if tag.is_yaml_core_schema() {
            return match tag.suffix.as_str() {
                "str" => Ok(Value::Value(ScalarOwned::String(text))),
                "int" => parse_yaml_int(&text)
                    .map(|int| Value::Value(ScalarOwned::Integer(int)))
                    .ok_or_else(|| ParseError::InvalidTaggedValue {
                        tag: tag.to_string(),
                        value: text,
                    }),
                "float" => parse_core_schema_fp(&text)
                    .map(ordered_float::OrderedFloat)
                    .map(ScalarOwned::FloatingPoint)
                    .map(Value::Value)
                    .ok_or_else(|| ParseError::InvalidTaggedValue {
                        tag: tag.to_string(),
                        value: text,
                    }),
                "bool" => match text.as_str() {
                    "true" | "True" | "TRUE" => Ok(Value::Value(ScalarOwned::Boolean(true))),
                    "false" | "False" | "FALSE" => Ok(Value::Value(ScalarOwned::Boolean(false))),
                    _ => Err(ParseError::InvalidTaggedValue {
                        tag: tag.to_string(),
                        value: text,
                    }),
                },
                "null" => match text.as_str() {
                    "" | "~" | "null" | "Null" | "NULL" => Ok(Value::Value(ScalarOwned::Null)),
                    _ => Err(ParseError::InvalidTaggedValue {
                        tag: tag.to_string(),
                        value: text,
                    }),
                },
                "map" | "seq" => Ok(Value::Representation(text, style, Some(tag))),
                _ => Err(ParseError::InvalidTaggedValue {
                    tag: tag.to_string(),
                    value: text,
                }),
            };
        }

        let inner = if style == saphyr::ScalarStyle::Plain {
            canonicalize(Value::Value(ScalarOwned::parse_from_cow(Cow::Owned(text))))
        } else {
            Value::Value(ScalarOwned::String(text))
        };
        return Ok(Value::Tagged(tag, Box::new(inner)));
    }

    if style == saphyr::ScalarStyle::Plain {
        Ok(canonicalize(Value::Value(ScalarOwned::parse_from_cow(
            Cow::Owned(text),
        ))))
    } else {
        Ok(Value::Value(ScalarOwned::String(text)))
    }
}

fn normalize_mapping(map: MappingOwned) -> ParseResult<Value> {
    let mut normalized = MappingOwned::new();

    for (raw_key, raw_value) in map {
        let key = normalize_value(raw_key)?;
        let value = normalize_value(raw_value)?;

        if is_merge_key(&key) {
            apply_merge_value(&mut normalized, &value)?;
        } else {
            normalized.insert(key, value);
        }
    }

    Ok(Value::Mapping(normalized))
}

fn normalize_tagged(tag: Tag, inner: Value) -> ParseResult<Value> {
    let normalized = normalize_value(inner)?;

    if !tag.is_yaml_core_schema() {
        return Ok(Value::Tagged(tag, Box::new(normalized)));
    }

    match tag.suffix.as_str() {
        "map" | "seq" => Ok(normalized),
        "str" => coerce_tagged_string(&normalized),
        "int" => coerce_tagged_int(&normalized, &tag),
        "float" => coerce_tagged_float(&normalized, &tag),
        "bool" => coerce_tagged_bool(&normalized, &tag),
        "null" => coerce_tagged_null(&normalized, &tag),
        _ => Err(ParseError::InvalidTaggedValue {
            tag: tag.to_string(),
            value: value_summary(&normalized).to_owned(),
        }),
    }
}

fn coerce_tagged_string(value: &Value) -> ParseResult<Value> {
    let text = scalar_text(value).ok_or_else(|| ParseError::InvalidTaggedValue {
        tag: "!!str".to_owned(),
        value: value_summary(value).to_owned(),
    })?;
    Ok(Value::Value(ScalarOwned::String(text)))
}

fn coerce_tagged_int(value: &Value, tag: &Tag) -> ParseResult<Value> {
    let text = scalar_text(value).ok_or_else(|| ParseError::InvalidTaggedValue {
        tag: tag.to_string(),
        value: value_summary(value).to_owned(),
    })?;
    parse_yaml_int(&text)
        .map(|int| Value::Value(ScalarOwned::Integer(int)))
        .ok_or_else(|| ParseError::InvalidTaggedValue {
            tag: tag.to_string(),
            value: text,
        })
}

fn coerce_tagged_float(value: &Value, tag: &Tag) -> ParseResult<Value> {
    let text = scalar_text(value).ok_or_else(|| ParseError::InvalidTaggedValue {
        tag: tag.to_string(),
        value: value_summary(value).to_owned(),
    })?;
    parse_core_schema_fp(&text)
        .map(ordered_float::OrderedFloat)
        .map(ScalarOwned::FloatingPoint)
        .map(Value::Value)
        .ok_or_else(|| ParseError::InvalidTaggedValue {
            tag: tag.to_string(),
            value: text,
        })
}

fn coerce_tagged_bool(value: &Value, tag: &Tag) -> ParseResult<Value> {
    let text = scalar_text(value).ok_or_else(|| ParseError::InvalidTaggedValue {
        tag: tag.to_string(),
        value: value_summary(value).to_owned(),
    })?;

    match text.as_str() {
        "true" | "True" | "TRUE" => Ok(Value::Value(ScalarOwned::Boolean(true))),
        "false" | "False" | "FALSE" => Ok(Value::Value(ScalarOwned::Boolean(false))),
        _ => Err(ParseError::InvalidTaggedValue {
            tag: tag.to_string(),
            value: text,
        }),
    }
}

fn coerce_tagged_null(value: &Value, tag: &Tag) -> ParseResult<Value> {
    let text = scalar_text(value).ok_or_else(|| ParseError::InvalidTaggedValue {
        tag: tag.to_string(),
        value: value_summary(value).to_owned(),
    })?;

    match text.as_str() {
        "" | "~" | "null" | "Null" | "NULL" => Ok(Value::Value(ScalarOwned::Null)),
        _ => Err(ParseError::InvalidTaggedValue {
            tag: tag.to_string(),
            value: text,
        }),
    }
}

fn scalar_text(value: &Value) -> Option<String> {
    match value {
        Value::Representation(text, _, _) => Some(text.clone()),
        Value::Value(ScalarOwned::Null) => Some("null".to_owned()),
        Value::Value(ScalarOwned::Boolean(value)) => Some(value.to_string()),
        Value::Value(ScalarOwned::Integer(value)) => Some(value.to_string()),
        Value::Value(ScalarOwned::FloatingPoint(value)) => Some(value.to_string()),
        Value::Value(ScalarOwned::String(value)) => Some(value.clone()),
        Value::Tagged(_, inner) => scalar_text(inner),
        _ => None,
    }
}

fn parse_yaml_int(text: &str) -> Option<i64> {
    let (sign, raw) = if let Some(rest) = text.strip_prefix('-') {
        (-1_i64, rest)
    } else if let Some(rest) = text.strip_prefix('+') {
        (1_i64, rest)
    } else {
        (1_i64, text)
    };

    let parsed = if let Some(number) = raw.strip_prefix("0x") {
        i64::from_str_radix(number, 16).ok()
    } else if let Some(number) = raw.strip_prefix("0o") {
        i64::from_str_radix(number, 8).ok()
    } else {
        raw.parse::<i64>().ok()
    }?;

    if sign < 0 {
        parsed.checked_neg()
    } else {
        Some(parsed)
    }
}

fn is_merge_key(key: &Value) -> bool {
    matches!(key, Value::Value(ScalarOwned::String(text)) if text == "<<")
        || matches!(key, Value::Representation(text, _, _) if text == "<<")
}

fn apply_merge_value(target: &mut MappingOwned, value: &Value) -> ParseResult<()> {
    match value {
        Value::Mapping(map) => {
            merge_mapping(target, map);
            Ok(())
        }
        Value::Sequence(items) => {
            for item in items {
                match item {
                    Value::Mapping(map) => merge_mapping(target, map),
                    Value::Tagged(_, inner) => apply_merge_value(target, inner)?,
                    other => {
                        return Err(ParseError::InvalidMergeValue {
                            kind: value_kind(other),
                        });
                    }
                }
            }
            Ok(())
        }
        Value::Tagged(_, inner) => apply_merge_value(target, inner),
        other => Err(ParseError::InvalidMergeValue {
            kind: value_kind(other),
        }),
    }
}

fn merge_mapping(target: &mut MappingOwned, source: &MappingOwned) {
    for (key, value) in source {
        if !target.contains_key(key) {
            target.insert(key.clone(), value.clone());
        }
    }
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Value(ScalarOwned::Null) => "null scalar",
        Value::Value(ScalarOwned::Boolean(_)) => "boolean scalar",
        Value::Value(ScalarOwned::Integer(_)) => "integer scalar",
        Value::Value(ScalarOwned::FloatingPoint(_)) => "float scalar",
        Value::Value(ScalarOwned::String(_)) => "string scalar",
        Value::Representation(_, _, _) => "scalar representation",
        Value::Sequence(_) => "sequence",
        Value::Mapping(_) => "mapping",
        Value::Tagged(_, _) => "tagged value",
        Value::Alias(_) => "alias",
        Value::BadValue => "bad value",
    }
}

fn value_summary(value: &Value) -> &str {
    match value {
        Value::Value(ScalarOwned::Null) => "null",
        Value::Value(ScalarOwned::Boolean(_)) => "boolean",
        Value::Value(ScalarOwned::Integer(_)) => "integer",
        Value::Value(ScalarOwned::FloatingPoint(_)) => "float",
        Value::Value(ScalarOwned::String(_)) => "string",
        Value::Representation(_, _, _) => "representation",
        Value::Sequence(_) => "sequence",
        Value::Mapping(_) => "mapping",
        Value::Tagged(_, _) => "tagged value",
        Value::Alias(_) => "alias",
        Value::BadValue => "bad value",
    }
}
