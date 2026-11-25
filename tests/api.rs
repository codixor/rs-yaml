use rs_yaml::{EmitError, EmitterConfig, ParseError, ScalarOwned, Value};

#[test]
fn emitter_config_defaults_and_builder_clamps_are_publicly_stable() {
    let defaults = EmitterConfig::default();
    assert_eq!(defaults.indent, 2);
    assert_eq!(defaults.width, 80);
    assert_eq!(defaults.default_flow_style, None);
    assert!(!defaults.explicit_start);
    assert!(defaults.compact);
    assert!(!defaults.multiline_strings);

    let built = EmitterConfig::new()
        .with_indent(100)
        .with_width(10)
        .with_default_flow_style(Some(true))
        .with_explicit_start(true)
        .with_compact(false)
        .with_multiline_strings(true);

    assert_eq!(built.indent, 9);
    assert_eq!(built.width, 20);
    assert_eq!(built.default_flow_style, Some(true));
    assert!(built.explicit_start);
    assert!(!built.compact);
    assert!(built.multiline_strings);
}

#[test]
fn parse_and_emit_errors_have_human_readable_messages() {
    let parse = rs_yaml::safe_load("a: [").unwrap_err();
    assert!(matches!(parse, ParseError::Syntax { .. }));
    assert!(parse.to_string().contains("line"));

    let emit = EmitError::UnsupportedType("complex key not supported");
    assert_eq!(
        emit.to_string(),
        "unsupported type for serialization: complex key not supported"
    );
}

#[test]
fn value_aliases_cover_common_scalar_shapes() {
    assert!(matches!(
        Value::Value(ScalarOwned::Null),
        Value::Value(ScalarOwned::Null)
    ));
    assert!(matches!(
        Value::Value(ScalarOwned::Boolean(true)),
        Value::Value(ScalarOwned::Boolean(true))
    ));
    assert!(matches!(
        Value::Value(ScalarOwned::Integer(42)),
        Value::Value(ScalarOwned::Integer(42))
    ));
    assert!(matches!(
        Value::Value(ScalarOwned::String("yaml".to_owned())),
        Value::Value(ScalarOwned::String(_))
    ));
}
