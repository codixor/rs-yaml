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
        ParseError::InvalidTaggedValue { ref tag, .. } if tag.contains("bool")
    ));
}

#[test]
fn emit_errors_stay_lightweight() {
    let error = EmitError::UnsupportedType("complex key not supported".to_owned());
    assert_eq!(
        error.to_string(),
        "unsupported type for serialization: complex key not supported"
    );
}

#[test]
fn duplicate_mapping_keys_report_first_and_duplicate_positions() {
    let error = Parser::validate_no_duplicate_mapping_keys("root:\n  title: One\n  title: Two\n")
        .unwrap_err();

    assert!(matches!(
        error,
        ParseError::DuplicateKey {
            ref key,
            first_line: 2,
            duplicate_line: 3,
            ..
        } if key == "title"
    ));
    assert!(error.to_string().contains("line 2"));
    assert!(error.to_string().contains("line 3"));
}

#[test]
fn duplicate_mapping_key_validation_is_scoped_per_mapping() {
    Parser::validate_no_duplicate_mapping_keys("left:\n  title: One\nright:\n  title: Two\n")
        .unwrap();
}
