use std::fmt::Write as _;

use crate::error::{EmitError, EmitResult};
use crate::value::Value;
use saphyr::{Scalar, Yaml, YamlEmitter};
use saphyr_parser::ScalarStyle;
use std::borrow::Cow;

/// Configuration for YAML emission.
///
/// Controls formatting, style, and output options when serializing YAML.
#[derive(Debug, Clone)]
pub struct EmitterConfig {
    /// Indentation width in spaces (default: 2).
    ///
    /// Controls the number of spaces used for each indentation level.
    /// Valid range: 1-9 (values outside this range will be clamped).
    ///
    /// Note: saphyr currently uses fixed 2-space indentation.
    /// This parameter is accepted for `PyYAML` API compatibility but
    /// may require post-processing to fully support custom values.
    pub indent: usize,

    /// Maximum line width for wrapping (default: 80).
    ///
    /// When lines exceed this width, the emitter will attempt to wrap them.
    /// Valid range: 20-1000 (values outside this range will be clamped).
    ///
    /// Note: saphyr has limited control over line wrapping.
    /// This parameter is accepted for `PyYAML` API compatibility.
    pub width: usize,

    /// Default flow style for collections (default: None).
    ///
    /// - `None`: Use block style (multi-line)
    /// - `Some(true)`: Force flow style (inline: `[...]`, `{...}`)
    /// - `Some(false)`: Force block style (explicit)
    pub default_flow_style: Option<bool>,

    /// Add explicit document start marker `---` (default: false).
    ///
    /// When true, prepends `---\n` to the output.
    pub explicit_start: bool,

    /// Enable compact inline notation (default: true).
    ///
    /// Controls whether saphyr uses compact notation for
    /// inline sequences and mappings.
    pub compact: bool,

    /// Render multiline strings in literal style (default: false).
    ///
    /// When true, strings containing newlines will be rendered
    /// using literal block scalar notation (`|`).
    pub multiline_strings: bool,
}

impl Default for EmitterConfig {
    fn default() -> Self {
        Self {
            indent: 2,
            width: 80,
            default_flow_style: None,
            explicit_start: false,
            compact: true,
            multiline_strings: false,
        }
    }
}

impl EmitterConfig {
    #[cfg(feature = "streaming")]
    pub(crate) fn upstream(&self) -> EmitResult<fast_yaml_core::EmitterConfig> {
        let indent = fast_yaml_core::Indent::new(self.indent.clamp(1, 9))
            .map_err(|e| EmitError::Emit(e.to_string()))?;
        Ok(fast_yaml_core::EmitterConfig::new()
            .with_indent(indent)
            .with_layout(fast_yaml_core::Layout::from_flow_flag(
                self.default_flow_style,
            ))
            .with_explicit_start(self.explicit_start)
            .with_multiline_strings(self.multiline_strings))
    }

    /// Create a new emitter configuration with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set indentation width (clamped to 1-9).
    #[must_use]
    pub fn with_indent(mut self, indent: usize) -> Self {
        self.indent = indent.clamp(1, 9);
        self
    }

    /// Set line width (clamped to 20-1000).
    #[must_use]
    pub fn with_width(mut self, width: usize) -> Self {
        self.width = width.clamp(20, 1000);
        self
    }

    /// Set default flow style for collections.
    #[must_use]
    pub const fn with_default_flow_style(mut self, flow_style: Option<bool>) -> Self {
        self.default_flow_style = flow_style;
        self
    }

    /// Set explicit document start marker.
    #[must_use]
    pub const fn with_explicit_start(mut self, explicit_start: bool) -> Self {
        self.explicit_start = explicit_start;
        self
    }

    /// Set compact inline notation.
    #[must_use]
    pub const fn with_compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    /// Set multiline string rendering.
    #[must_use]
    pub const fn with_multiline_strings(mut self, multiline_strings: bool) -> Self {
        self.multiline_strings = multiline_strings;
        self
    }
}

/// Emitter for YAML documents.
///
/// Wraps saphyr's `YamlEmitter` to provide a consistent API.
#[derive(Debug)]
pub struct Emitter;

impl Emitter {
    /// Emit a single YAML document to a string with configuration.
    ///
    /// # Errors
    ///
    /// Returns `EmitError::Emit` if the value cannot be serialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use rs_yaml::{Emitter, EmitterConfig, ScalarOwned, Value};
    ///
    /// let value = Value::Value(ScalarOwned::String("test".to_string()));
    /// let config = EmitterConfig::new().with_explicit_start(true);
    /// let yaml = Emitter::emit_str_with_config(&value, &config)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn emit_str_with_config(value: &Value, config: &EmitterConfig) -> EmitResult<String> {
        // When flow style is requested, use the custom path that renders {k: v} / [a, b].
        if config.default_flow_style == Some(true) {
            let raw = Self::emit_flow(value, 0)?;
            let mut output = Self::apply_formatting(raw, config);
            if !output.is_empty() && !output.ends_with('\n') {
                output.push('\n');
            }
            return Ok(output);
        }

        let mut output = String::new();
        {
            let mut emitter = YamlEmitter::new(&mut output);

            // Apply saphyr native configuration
            emitter.compact(config.compact);
            emitter.multiline(config.multiline_strings);

            // Convert YamlOwned to Yaml for emission
            let yaml_borrowed = Self::emission_node(value, 0)?;
            emitter
                .dump(&yaml_borrowed)
                .map_err(|e| EmitError::Emit(e.to_string()))?;
        }

        // Apply post-processing for configuration options
        output = Self::apply_formatting(output, config);

        // Ensure output always ends with a newline
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }

        Ok(output)
    }

    // Prepare scalar spellings before serializing. Rewriting completed YAML can change
    // user strings and block contents that happen to spell a special floating-point value.
    fn emission_node(value: &Value, depth: usize) -> EmitResult<Yaml<'_>> {
        Self::check_depth(depth)?;
        Ok(match value {
            Value::Value(scalar) => Self::emission_scalar(scalar.into()),
            Value::Representation(text, style, tag) => {
                let scalar = if *style == ScalarStyle::Plain && tag.is_none() {
                    Scalar::parse_from_cow(Cow::Borrowed(text))
                } else {
                    Scalar::String(Cow::Borrowed(text))
                };
                let node = Self::emission_scalar(scalar);
                match tag {
                    Some(tag) => Yaml::Tagged(Cow::Borrowed(tag), Box::new(node)),
                    None => node,
                }
            }
            Value::Tagged(tag, inner) => Yaml::Tagged(
                Cow::Borrowed(tag),
                Box::new(Self::emission_node(inner, depth + 1)?),
            ),
            Value::Sequence(items) => Yaml::Sequence(
                items
                    .iter()
                    .map(|item| Self::emission_node(item, depth + 1))
                    .collect::<EmitResult<_>>()?,
            ),
            Value::Mapping(items) => Yaml::Mapping(
                items
                    .iter()
                    .map(|(k, v)| {
                        Ok((
                            Self::emission_node(k, depth + 1)?,
                            Self::emission_node(v, depth + 1)?,
                        ))
                    })
                    .collect::<EmitResult<_>>()?,
            ),
            Value::BadValue => Yaml::BadValue,
            Value::Alias(_) => {
                return Err(EmitError::UnsupportedType(
                    "alias without an anchor definition".into(),
                ));
            }
        })
    }

    fn emission_scalar(scalar: Scalar<'_>) -> Yaml<'_> {
        if let Scalar::FloatingPoint(value) = scalar {
            let value = value.0;
            let text = if value.is_nan() {
                Cow::Borrowed(".nan")
            } else if value == f64::INFINITY {
                Cow::Borrowed(".inf")
            } else if value == f64::NEG_INFINITY {
                Cow::Borrowed("-.inf")
            } else {
                Cow::Owned(format!("{value:?}"))
            };
            Yaml::Representation(text, ScalarStyle::Plain, None)
        } else {
            Yaml::Value(scalar)
        }
    }

    fn check_depth(depth: usize) -> EmitResult<()> {
        if depth >= crate::load_guard::MAX_DEPTH {
            return Err(EmitError::Emit(
                "value exceeds maximum nesting depth".into(),
            ));
        }
        Ok(())
    }

    /// Emit a single YAML document to a string with default configuration.
    ///
    /// # Errors
    ///
    /// Returns `EmitError::Emit` if the value cannot be serialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use rs_yaml::{Emitter, ScalarOwned, Value};
    ///
    /// let value = Value::Value(ScalarOwned::String("test".to_string()));
    /// let yaml = Emitter::emit_str(&value)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn emit_str(value: &Value) -> EmitResult<String> {
        Self::emit_str_with_config(value, &EmitterConfig::default())
    }

    /// Emit multiple YAML documents to a string with document separators and configuration.
    ///
    /// # Errors
    ///
    /// Returns `EmitError::Emit` if any value cannot be serialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use rs_yaml::{Emitter, EmitterConfig, ScalarOwned, Value};
    ///
    /// let docs = vec![
    ///     Value::Value(ScalarOwned::String("first".to_string())),
    ///     Value::Value(ScalarOwned::String("second".to_string())),
    /// ];
    /// let config = EmitterConfig::new().with_explicit_start(true);
    /// let yaml = Emitter::emit_all_with_config(&docs, &config)?;
    /// assert!(yaml.contains("---"));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn emit_all_with_config(values: &[Value], config: &EmitterConfig) -> EmitResult<String> {
        let mut output = String::new();

        // Create single config variant for non-first documents (avoids cloning per document)
        let inner_config = EmitterConfig {
            explicit_start: false,
            ..*config
        };

        for (i, value) in values.iter().enumerate() {
            // Add document separator before each document (except first if explicit_start is false)
            if i > 0 || config.explicit_start {
                output.push_str("---\n");
            }

            // Always use inner_config (with explicit_start=false) since we handle
            // document separators explicitly above
            let doc = Self::emit_str_with_config(value, &inner_config)?;
            output.push_str(&doc);

            // Ensure document ends with newline for proper separation
            if !output.ends_with('\n') {
                output.push('\n');
            }
        }

        Ok(output)
    }

    /// Emit multiple YAML documents to a string with document separators.
    ///
    /// # Errors
    ///
    /// Returns `EmitError::Emit` if any value cannot be serialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use rs_yaml::{Emitter, ScalarOwned, Value};
    ///
    /// let docs = vec![
    ///     Value::Value(ScalarOwned::String("first".to_string())),
    ///     Value::Value(ScalarOwned::String("second".to_string())),
    /// ];
    /// let yaml = Emitter::emit_all(&docs)?;
    /// assert!(yaml.contains("---"));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn emit_all(values: &[Value]) -> EmitResult<String> {
        Self::emit_all_with_config(values, &EmitterConfig::default())
    }

    /// Apply formatting configuration to YAML output.
    ///
    /// Handles `explicit_start` and potentially other post-processing.
    fn apply_formatting(mut output: String, config: &EmitterConfig) -> String {
        // Handle explicit_start
        if config.explicit_start {
            if !output.starts_with("---\n") {
                output.insert_str(0, "---\n");
            }
        } else if output.starts_with("---\n") {
            output.drain(..4);
        }

        // Re-indent when caller requests a width other than saphyr's fixed 2 spaces.
        if config.indent != 2 {
            output = Self::reindent(&output, config.indent.clamp(1, 9));
        }

        output
    }

    /// Extract `%YAML` and `%TAG` directive lines that appear before the first `---`.
    ///
    /// Returns the directive block (with a trailing newline) or an empty string.
    #[cfg(not(feature = "streaming"))]
    fn extract_directives(input: &str) -> String {
        let mut directives = String::new();
        for line in input.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("---") || trimmed.starts_with("...") {
                break;
            }
            if trimmed.starts_with("%YAML") || trimmed.starts_with("%TAG") {
                directives.push_str(line);
                directives.push('\n');
            }
        }
        directives
    }

    /// Format a YAML string with configuration.
    ///
    /// Uses streaming formatter for large files when the `streaming` feature is enabled,
    /// falling back to DOM-based formatting for small files or complex cases.
    ///
    /// Block scalar styles (`|` literal and `>` folded) are preserved in the output.
    /// `%YAML` and `%TAG` directives are extracted and prepended to the formatted output.
    ///
    /// Comments are not preserved by the current formatter pipeline.
    ///
    /// # Errors
    ///
    /// Returns `EmitError::Emit` if the YAML cannot be parsed or formatted.
    ///
    /// # Examples
    ///
    /// ```
    /// use rs_yaml::{Emitter, EmitterConfig};
    ///
    /// let yaml = "key: value\nlist:\n  - item1\n  - item2\n";
    /// let config = EmitterConfig::default();
    /// let formatted = Emitter::format_with_config(yaml, &config)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn format_with_config(input: &str, config: &EmitterConfig) -> EmitResult<String> {
        #[cfg(feature = "arena")]
        {
            crate::streaming::format_streaming_arena(input, config)
        }

        #[cfg(all(feature = "streaming", not(feature = "arena")))]
        {
            crate::streaming::format_streaming(input, config)
        }

        // Non-streaming fallback: parse with early_parse=false to preserve block scalar styles
        // (literal | and folded >) instead of converting them to double-quoted strings.
        #[cfg(not(feature = "streaming"))]
        {
            let directives = Self::extract_directives(input);
            let docs = crate::Parser::parse_all_preserving_styles(input)
                .map_err(|e| EmitError::Emit(e.to_string()))?;
            if docs.is_empty() {
                return Ok(String::new());
            }
            let inner_config = EmitterConfig {
                explicit_start: false,
                indent: config.indent.clamp(1, 9),
                ..*config
            };
            let mut output = String::new();
            for (i, doc) in docs.iter().enumerate() {
                if i > 0 || config.explicit_start {
                    if !output.is_empty() && !output.ends_with('\n') {
                        output.push('\n');
                    }
                    output.push_str("---\n");
                }
                let emitted = Self::emit_str_preserving_styles(doc, &inner_config, 0)?;
                output.push_str(&emitted);
            }
            if !output.is_empty() && !output.ends_with('\n') {
                output.push('\n');
            }
            Ok(Self::prepend_directives(&directives, output))
        }
    }

    /// Prepend directive lines to formatted output.
    ///
    /// If `directives` is non-empty, inserts them before the first `---` line
    /// or at the beginning of the output.
    #[cfg(not(feature = "streaming"))]
    fn prepend_directives(directives: &str, formatted: String) -> String {
        if directives.is_empty() {
            return formatted;
        }
        // Per YAML 1.2 spec §6.8.1, a directive end marker (`---`) MUST follow
        // any directives. If the formatter already emitted `---`, just prepend the
        // directives; otherwise inject the required marker between them.
        if formatted.starts_with("---") {
            format!("{directives}{formatted}")
        } else {
            format!("{directives}---\n{formatted}")
        }
    }

    /// Emit a YAML value to a string, preserving block scalar styles.
    ///
    /// Handles `Literal` (`|`) and `Folded` (`>`) styles directly.
    /// All other nodes are delegated to the saphyr emitter.
    ///
    /// # Errors
    ///
    /// Returns `EmitError::Emit` if the value cannot be serialized.
    #[cfg(any(test, not(feature = "streaming")))]
    #[allow(dead_code)]
    fn emit_str_preserving_styles(
        value: &Value,
        config: &EmitterConfig,
        indent_level: usize,
    ) -> EmitResult<String> {
        // Fast path: no block scalars — use standard saphyr emitter
        if !Self::has_block_scalar(value) {
            return Self::emit_str_with_config(value, config);
        }
        let raw = Self::emit_value(value, config, indent_level)?;
        // Apply the same post-processing (special floats, explicit_start) as the standard path
        Ok(Self::apply_formatting(raw, config))
    }

    /// Check whether the value tree contains any Literal or Folded block scalars.
    #[cfg(any(test, not(feature = "streaming")))]
    #[allow(dead_code)]
    fn has_block_scalar(value: &Value) -> bool {
        match value {
            Value::Representation(_, ScalarStyle::Literal | ScalarStyle::Folded, _) => true,
            Value::Sequence(seq) => seq.iter().any(Self::has_block_scalar),
            Value::Mapping(map) => map
                .iter()
                .any(|(k, v)| Self::has_block_scalar(k) || Self::has_block_scalar(v)),
            Value::Tagged(_, inner) => Self::has_block_scalar(inner),
            _ => false,
        }
    }

    /// Recursively emit a YAML value, handling block scalars manually.
    ///
    /// `indent_level` is the current nesting depth (in units of `config.indent` spaces).
    /// Returns YAML text without a leading `---\n` document marker.
    #[cfg(any(test, not(feature = "streaming")))]
    #[allow(dead_code)]
    fn emit_value(
        value: &Value,
        config: &EmitterConfig,
        indent_level: usize,
    ) -> EmitResult<String> {
        match value {
            Value::Representation(content, ScalarStyle::Literal, _) => Ok(
                Self::format_block_scalar(content, '|', config.indent, indent_level),
            ),
            Value::Representation(content, ScalarStyle::Folded, _) => Ok(
                Self::format_block_scalar(content, '>', config.indent, indent_level),
            ),
            Value::Mapping(map) => {
                let indent = " ".repeat(indent_level * config.indent);
                let mut out = String::new();
                for (k, v) in map {
                    let key_str = Self::emit_scalar_inline(k)?;
                    match v {
                        Value::Representation(_, ScalarStyle::Literal | ScalarStyle::Folded, _) => {
                            // Block scalar directly as value: `key: |\n  line\n`
                            let val_str = Self::emit_value(v, config, indent_level + 1)?;
                            write!(out, "{indent}{key_str}: {val_str}")
                                .map_err(|e| EmitError::Emit(e.to_string()))?;
                        }
                        Value::Mapping(_) | Value::Sequence(_) => {
                            writeln!(out, "{indent}{key_str}:")
                                .map_err(|e| EmitError::Emit(e.to_string()))?;
                            let val_str = Self::emit_value(v, config, indent_level + 1)?;
                            out.push_str(&val_str);
                        }
                        _ => {
                            let val_str = Self::emit_value_inline(v)?;
                            writeln!(out, "{indent}{key_str}: {val_str}")
                                .map_err(|e| EmitError::Emit(e.to_string()))?;
                        }
                    }
                }
                Ok(out)
            }
            Value::Sequence(seq) => {
                let indent = " ".repeat(indent_level * config.indent);
                let mut out = String::new();
                for item in seq {
                    match item {
                        Value::Representation(_, ScalarStyle::Literal | ScalarStyle::Folded, _) => {
                            let item_str = Self::emit_value(item, config, indent_level + 1)?;
                            write!(out, "{indent}- {item_str}")
                                .map_err(|e| EmitError::Emit(e.to_string()))?;
                        }
                        Value::Mapping(_) | Value::Sequence(_) => {
                            writeln!(out, "{indent}-")
                                .map_err(|e| EmitError::Emit(e.to_string()))?;
                            let item_str = Self::emit_value(item, config, indent_level + 1)?;
                            out.push_str(&item_str);
                        }
                        _ => {
                            let item_str = Self::emit_value_inline(item)?;
                            writeln!(out, "{indent}- {item_str}")
                                .map_err(|e| EmitError::Emit(e.to_string()))?;
                        }
                    }
                }
                Ok(out)
            }
            // Scalars and everything else: delegate to saphyr (no block style involved)
            _ => Self::emit_value_inline(value).map(|s| format!("{s}\n")),
        }
    }

    /// Emit a scalar key as an inline string (no trailing newline).
    #[cfg(any(test, not(feature = "streaming")))]
    fn emit_scalar_inline(value: &Value) -> EmitResult<String> {
        match value {
            Value::Value(_) | Value::Representation(..) => Self::emit_value_inline(value),
            _ => Err(EmitError::UnsupportedType(
                "complex key not supported here".into(),
            )),
        }
    }

    /// Emit any non-block-scalar value as an inline string (no trailing newline).
    fn emit_value_inline(value: &Value) -> EmitResult<String> {
        let mut out = String::new();
        {
            let mut emitter = YamlEmitter::new(&mut out);
            emitter.compact(true);
            let yaml = Self::emission_node(value, 0)?;
            emitter
                .dump(&yaml)
                .map_err(|e| EmitError::Emit(e.to_string()))?;
        }
        // saphyr emits "---\nvalue\n" — strip markers
        let trimmed = out
            .strip_prefix("---\n")
            .unwrap_or(&out)
            .trim_end_matches('\n');
        Ok(trimmed.to_string())
    }

    /// Format a block scalar header + indented body lines.
    ///
    /// Returns text starting with the block indicator (`|` or `>`),
    /// followed by `\n` and indented content lines.
    #[cfg(any(test, not(feature = "streaming")))]
    #[allow(dead_code)]
    fn format_block_scalar(
        content: &str,
        indicator: char,
        indent_width: usize,
        indent_level: usize,
    ) -> String {
        let child_indent = " ".repeat(indent_level * indent_width);

        // Chomping: keep (+) if trailing blank lines, strip (-) if no trailing newline,
        // clip (default) otherwise.
        let chomping = if content.ends_with("\n\n") {
            "+"
        } else if !content.ends_with('\n') {
            "-"
        } else {
            ""
        };

        let mut out = format!("{indicator}{chomping}\n");
        for line in content.lines() {
            if line.is_empty() {
                out.push('\n');
            } else {
                // String::writeln never fails
                let _ = writeln!(out, "{child_indent}{line}");
            }
        }
        out
    }

    /// Emit a value in YAML flow style: mappings as `{k: v}`, sequences as `[a, b]`.
    ///
    /// Scalar values are rendered inline.  Nested collections are also rendered
    /// in flow style recursively.
    ///
    /// Returns a string without a leading `---\n` marker and without a trailing newline.
    fn emit_flow(value: &Value, depth: usize) -> EmitResult<String> {
        Self::check_depth(depth)?;
        match value {
            Value::Mapping(map) => {
                let mut out = String::from("{");
                for (i, (k, v)) in map.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    let key_str = Self::emit_flow(k, depth + 1)?;
                    let val_str = Self::emit_flow(v, depth + 1)?;
                    write!(out, "{key_str}: {val_str}")
                        .map_err(|e| EmitError::Emit(e.to_string()))?;
                }
                out.push('}');
                Ok(out)
            }
            Value::Sequence(seq) => {
                let mut out = String::from("[");
                for (i, item) in seq.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(&Self::emit_flow(item, depth + 1)?);
                }
                out.push(']');
                Ok(out)
            }
            // Scalars: use the inline scalar renderer (no trailing newline)
            _ => Self::emit_value_inline(value),
        }
    }

    /// Re-indent saphyr output from 2-space indentation to `target` spaces per level.
    ///
    /// Lines that form block scalar bodies (content under `|` / `>`) retain their
    /// relative spacing; only the base indent level is rescaled.
    ///
    /// `---` / `...` markers and directive lines (`%YAML`, `%TAG`) are left unchanged.
    fn reindent(output: &str, target: usize) -> String {
        let mut result = String::with_capacity(output.len());
        let mut in_block_scalar = false;
        let mut block_scalar_base_indent: usize = 0;

        for (i, line) in output.lines().enumerate() {
            if i > 0 {
                result.push('\n');
            }

            // Directives and document markers: never re-indent.
            let trimmed = line.trim_start();
            if trimmed.starts_with("---")
                || trimmed.starts_with("...")
                || trimmed.starts_with("%YAML")
                || trimmed.starts_with("%TAG")
            {
                in_block_scalar = false;
                result.push_str(line);
                continue;
            }

            let leading = line.len() - trimmed.len();
            let level = leading / 2; // saphyr always uses 2-space indent

            if in_block_scalar {
                // Inside a block scalar body: keep lines that are deeper than the
                // mapping/sequence key that introduced the scalar.
                if leading > block_scalar_base_indent {
                    // Rescale: base_level * target + (extra spaces beyond base)
                    let base_level = block_scalar_base_indent / 2;
                    let extra = leading - block_scalar_base_indent;
                    let new_leading = base_level * target + extra;
                    let spaces = " ".repeat(new_leading);
                    result.push_str(&spaces);
                    result.push_str(trimmed);
                    continue;
                }
                // Dedented back out of the block scalar
                in_block_scalar = false;
            }

            // Detect start of block scalar: line ends with `|` or `>` (with optional
            // chomping indicator and trailing whitespace).
            let value_part = trimmed.trim_end_matches(|c: char| c.is_whitespace());
            let last_nonws = value_part.trim_start_matches(|c: char| c != '|' && c != '>');
            if last_nonws.starts_with('|') || last_nonws.starts_with('>') {
                in_block_scalar = true;
                block_scalar_base_indent = leading;
            }

            let new_leading = level * target;
            let spaces = " ".repeat(new_leading);
            result.push_str(&spaces);
            result.push_str(trimmed);
        }

        // Preserve trailing newline if present
        if output.ends_with('\n') {
            result.push('\n');
        }

        result
    }

    /// Format a YAML string with default configuration.
    ///
    /// Uses streaming formatter for large files when the `streaming` feature is enabled.
    ///
    /// # Errors
    ///
    /// Returns `EmitError::Emit` if the YAML cannot be parsed or formatted.
    ///
    /// # Examples
    ///
    /// ```
    /// use rs_yaml::Emitter;
    ///
    /// let yaml = "key: value\nlist:\n  - item1\n  - item2\n";
    /// let formatted = Emitter::format(yaml)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn format(input: &str) -> EmitResult<String> {
        Self::format_with_config(input, &EmitterConfig::default())
    }
}
