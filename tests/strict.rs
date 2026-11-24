use rs_yaml::{Emitter, Parser, safe_load, safe_load_all};

#[test]
fn all_loaders_and_formatters_reject_equivalent_mapping_keys() {
    for yaml in [
        "a: 1\na: 2",
        "{key: 1, 'key': 2}",
        "parent: {0x10: a, 16: b}",
        "{true: a, !!bool TRUE: b}",
        "? [a, b]\n: first\n? [a, b]\n: second",
        "? {a: 1, b: 2}\n: first\n? {b: 2, a: 1}\n: second",
        "key: &seq [a, b]\n? *seq\n: first\n? [a, b]\n: second",
        "<<: {a: 1}\n<<: {b: 2}",
        "? [<<]\n: first\n? ['<<']\n: second",
        "? {a: <<}\n: first\n? {a: '<<'}\n: second",
    ] {
        assert!(safe_load(yaml).is_err(), "{yaml}");
        assert!(safe_load_all(yaml).is_err(), "{yaml}");
        assert!(Parser::parse_marked_str(yaml).is_err(), "{yaml}");
        assert!(Parser::parse_all_preserving_styles(yaml).is_err(), "{yaml}");
        assert!(
            Parser::validate_no_duplicate_mapping_keys(yaml).is_err(),
            "{yaml}"
        );
        assert!(Emitter::format(yaml).is_err(), "{yaml}");
    }
}

#[test]
fn single_document_entry_points_reject_even_an_empty_second_document() {
    for yaml in ["a: 1\n---\nb: 2", "---\n...\n---\n", "---\n---\n"] {
        assert!(safe_load(yaml).is_err());
        assert!(Parser::parse_marked_str(yaml).is_err());
        assert_eq!(safe_load_all(yaml).unwrap().len(), 2);
        assert_eq!(Parser::parse_all_marked(yaml).unwrap().len(), 2);
    }
    assert!(safe_load("message: |\n  ---\n").is_ok());
}

#[test]
fn distinct_tags_scopes_and_explicit_merge_overrides_are_valid() {
    let docs = safe_load_all("1: a\n'1': b\n---\n1: c").unwrap();
    assert_eq!(docs[0].as_mapping().unwrap().len(), 2);
    let doc = safe_load(
        "base: &base {key: default}\na: {key: first, <<: *base}\nb: {<<: *base, key: second}",
    )
    .unwrap();
    assert_eq!(doc["a"]["key"].as_str(), Some("first"));
    assert_eq!(doc["b"]["key"].as_str(), Some("second"));
}

#[test]
fn errors_do_not_retain_or_quote_input_values() {
    const MARKER: &str = "private-fixture-marker";
    for yaml in [
        format!("config: !!int {MARKER}"),
        format!("{MARKER}: a\n{MARKER}: b"),
        format!("config: !!{MARKER} value"),
        format!("config: *{MARKER}"),
    ] {
        let error = safe_load(&yaml).unwrap_err();
        assert!(!format!("{error}\n{error:?}").contains(MARKER));
        let mut source = std::error::Error::source(&error);
        while let Some(error) = source {
            assert!(!format!("{error}\n{error:?}").contains(MARKER));
            source = error.source();
        }
    }
}

#[test]
fn normalized_complex_keys_cannot_overwrite_each_other_after_merge_expansion() {
    let yaml = "? {<<: {a: 1}}\n: first\n? {a: 1}\n: second";
    assert!(safe_load(yaml).is_err());
}

#[test]
fn duplicate_locations_point_to_keys_and_not_their_values() {
    let error = safe_load("root:\n  sample: first\n  sample: second\n").unwrap_err();
    assert!(matches!(
        error,
        rs_yaml::ParseError::DuplicateKey {
            first_line: 2,
            first_column: 3,
            duplicate_line: 3,
            duplicate_column: 3,
            ..
        }
    ));
}

#[test]
fn compatibility_conversion_never_replaces_a_second_explicit_key() {
    // Saphyr drops this collection's mismatched core tag and normalization removes
    // the scalar style. The owned keys would otherwise be identical.
    let yaml = "? !!seq {a: b}\n: one\n? {a: 'b'}\n: two";
    assert!(safe_load(yaml).is_err());
}
