use ordered_float::OrderedFloat;
use pretty_assertions::assert_eq;
use rs_yaml::{Emitter, EmitterConfig, Map, ScalarOwned, Value};

fn string_value(value: &str) -> Value {
    Value::Value(ScalarOwned::String(value.to_owned()))
}

#[test]
fn emit_document_with_explicit_start() {
    let mut map = Map::new();
    map.insert(string_value("name"), string_value("yaml"));
    map.insert(
        string_value("enabled"),
        Value::Value(ScalarOwned::Boolean(true)),
    );

    let output = Emitter::emit_str_with_config(
        &Value::Mapping(map),
        &EmitterConfig::new().with_explicit_start(true),
    )
    .unwrap();

    assert!(output.starts_with("---\n"));
    assert!(output.contains("name: yaml"));
    assert!(output.contains("enabled: true"));
    assert!(output.ends_with('\n'));
}

#[test]
fn emit_flow_style_collections() {
    let sequence = Value::Sequence(vec![
        Value::Value(ScalarOwned::Integer(1)),
        Value::Value(ScalarOwned::Integer(2)),
        Value::Value(ScalarOwned::Integer(3)),
    ]);

    let output = Emitter::emit_str_with_config(
        &sequence,
        &EmitterConfig::new().with_default_flow_style(Some(true)),
    )
    .unwrap();

    assert_eq!(output, "[1, 2, 3]\n");
}

#[test]
fn emit_multiple_documents_keeps_separators() {
    let documents = vec![string_value("first"), string_value("second")];
    let output =
        Emitter::emit_all_with_config(&documents, &EmitterConfig::new().with_explicit_start(true))
            .unwrap();

    assert!(output.contains("---\nfirst"));
    assert!(output.contains("---\nsecond"));
}

#[test]
fn emit_multiline_strings_can_use_literal_style() {
    let output = Emitter::emit_str_with_config(
        &Value::Value(ScalarOwned::String("line1\nline2".to_owned())),
        &EmitterConfig::new().with_multiline_strings(true),
    )
    .unwrap();

    assert!(output.contains("line1"));
    assert!(output.contains("line2"));
}

#[test]
fn emit_special_floats_follow_yaml12_core_schema_text() {
    let output = Emitter::emit_str(&Value::Value(ScalarOwned::FloatingPoint(OrderedFloat(
        f64::INFINITY,
    ))))
    .unwrap();

    assert_eq!(output, ".inf\n");
}
