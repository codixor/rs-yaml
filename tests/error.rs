use rs_yaml::{EmitError, ParseError, Parser, safe_load};

#[test]
fn invalid_merge_values_are_reported_clearly() {
    let error = safe_load("target:\n  <<: 1\n").unwrap_err();
    assert!(matches!(
        error,
        ParseError::InvalidMergeValue {
            kind: "integer scalar"
        }
    ));
}

#[test]
fn invalid_explicit_tags_return_typed_errors() {
    let error = safe_load("flag: !!bool 'yes'\n").unwrap_err();
    assert!(matches!(
        error,
        ParseError::InvalidTaggedValue { tag: "!!bool" }
    ));
}

#[test]
fn emit_errors_stay_lightweight() {
    let error = EmitError::UnsupportedType("complex key not supported");
    assert_eq!(
        error.to_string(),
        "unsupported type for serialization: complex key not supported"
    );
}

#[test]
fn duplicate_mapping_keys_report_first_and_duplicate_positions() {
    let error = Parser::validate_no_duplicate_mapping_keys("root:\n  title: One\n  title: Two\n")
        .unwrap_err();

    let ParseError::DuplicateKey { first, duplicate } = &error else {
        panic!("{error}")
    };
    assert_eq!(first.0.line.unwrap().get(), 2);
    assert_eq!(duplicate.0.line.unwrap().get(), 3);
    assert!(error.to_string().contains("line 2"));
    assert!(error.to_string().contains("line 3"));
}

#[test]
fn duplicate_mapping_key_validation_is_scoped_per_mapping() {
    Parser::validate_no_duplicate_mapping_keys("left:\n  title: One\nright:\n  title: Two\n")
        .unwrap();
}

#[test]
fn flow_nesting_limit_is_reported_as_a_limit() {
    let input = format!("{}0{}", "[".repeat(256), "]".repeat(256));
    assert!(matches!(
        safe_load(&input),
        Err(ParseError::ResourceLimit { .. })
    ));
}

#[test]
fn upstream_alias_budget_is_reported_as_a_limit() {
    use saphyr::{YamlLoader, YamlOwned};
    use saphyr_parser::{BufferedInput, SpannedEventReceiver};

    let mut loader = YamlLoader::<YamlOwned>::default();
    loader.alias_limit(1);
    let input = "item: &item [one, two]\ncopy: *item";
    for pair in saphyr_parser::Parser::new(BufferedInput::new(input.chars())) {
        let (event, span) = pair.unwrap();
        loader.on_event(event, span);
        if let Some(error) = loader.error() {
            assert!(matches!(
                ParseError::from(error.clone()),
                ParseError::ResourceLimit { .. }
            ));
            return;
        }
    }
    panic!("the configured alias budget should be reached");
}

#[test]
fn core_tag_errors_show_the_written_spelling() {
    let error = rs_yaml::safe_load("token: !!int private-fixture-marker")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("!!int") && !error.contains("tag:yaml.org"),
        "{error}"
    );
    assert!(!error.contains("private-fixture-marker"), "{error}");
}
