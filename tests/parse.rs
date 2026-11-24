mod support;

use pretty_assertions::assert_eq;
use rs_yaml::{Map, Parser, ScalarOwned, Value, YamlDataOwned, safe_load, safe_load_all};
use saphyr_parser::ScalarStyle;

fn string_key(key: &str) -> Value {
    Value::Value(ScalarOwned::String(key.to_owned()))
}

fn map_value<'a>(map: &'a Map, key: &str) -> &'a Value {
    map.get(&string_key(key))
        .unwrap_or_else(|| panic!("missing mapping key {key:?}"))
}

fn preserving_map_value<'a>(map: &'a Map, key: &str) -> &'a Value {
    map.iter()
        .find_map(|(candidate, value)| match candidate {
            Value::Value(ScalarOwned::String(text)) | Value::Representation(text, _, _)
                if text == key =>
            {
                Some(value)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing preserving-style mapping key {key:?}"))
}

#[test]
fn parse_simple_mapping_fixture() {
    let document = Parser::parse_str(&support::fixture("parse/simple_mapping.yaml")).unwrap();
    let Value::Mapping(map) = document else {
        panic!("expected mapping root");
    };

    assert_eq!(map_value(&map, "name").as_str(), Some("yaml"));
    assert!(matches!(
        map_value(&map, "enabled"),
        Value::Value(ScalarOwned::Boolean(true))
    ));
    assert!(matches!(
        map_value(&map, "count"),
        Value::Value(ScalarOwned::Integer(3))
    ));
}

#[test]
fn safe_load_treats_comment_only_documents_as_null() {
    let document = safe_load(&support::fixture("parse/comments_only.yaml")).unwrap();
    assert!(matches!(document, Value::Value(ScalarOwned::Null)));
}

#[test]
fn safe_load_resolves_yaml_merge_keys() {
    let document = safe_load(&support::fixture("parse/merge_keys.yaml")).unwrap();
    let Value::Mapping(root) = document else {
        panic!("expected mapping root");
    };
    let Value::Mapping(development) = map_value(&root, "development") else {
        panic!("expected development mapping");
    };

    assert!(!development.keys().any(|key| key.as_str() == Some("<<")));
    assert_eq!(map_value(development, "adapter").as_str(), Some("postgres"));
    assert_eq!(map_value(development, "host").as_str(), Some("localhost"));
    assert_eq!(map_value(development, "database").as_str(), Some("dev_db"));
}

#[test]
fn safe_load_honors_explicit_core_tags_for_quoted_scalars() {
    let document = safe_load(&support::fixture("parse/explicit_tags.yaml")).unwrap();
    let Value::Mapping(map) = document else {
        panic!("expected mapping root");
    };

    assert!(matches!(
        map_value(&map, "int"),
        Value::Value(ScalarOwned::Integer(42))
    ));
    assert!(matches!(
        map_value(&map, "float"),
        Value::Value(ScalarOwned::FloatingPoint(_))
    ));
    assert!(matches!(
        map_value(&map, "bool"),
        Value::Value(ScalarOwned::Boolean(true))
    ));
    assert!(matches!(
        map_value(&map, "null"),
        Value::Value(ScalarOwned::Null)
    ));
}

#[test]
fn safe_load_all_keeps_multiple_documents() {
    let documents = safe_load_all("---\nname: first\n---\nname: second\n").unwrap();
    assert_eq!(documents.len(), 2);

    let Value::Mapping(first) = &documents[0] else {
        panic!("expected first mapping document");
    };
    let Value::Mapping(second) = &documents[1] else {
        panic!("expected second mapping document");
    };

    assert_eq!(map_value(first, "name").as_str(), Some("first"));
    assert_eq!(map_value(second, "name").as_str(), Some("second"));
}

#[test]
fn preserving_styles_keeps_literal_block_scalars() {
    let documents =
        Parser::parse_all_preserving_styles(&support::fixture("format/directives_and_blocks.yaml"))
            .unwrap();
    assert_eq!(documents.len(), 1);

    let Value::Mapping(root) = &documents[0] else {
        panic!("expected mapping root");
    };

    assert!(matches!(
        preserving_map_value(root, "description"),
        Value::Representation(text, ScalarStyle::Literal, _) if text == "first line\nsecond line\n"
    ));
}

#[test]
fn parse_marked_str_preserves_nested_source_spans() {
    let document = Parser::parse_marked_str("root:\n  title: Hello\n").unwrap();

    let YamlDataOwned::Mapping(root) = &document.data else {
        panic!("expected marked mapping root");
    };
    let (root_key, root_value) = root
        .iter()
        .find(|(key, _)| {
            matches!(&key.data, YamlDataOwned::Representation(text, _, _) if text == "root")
        })
        .expect("missing root key");

    assert_eq!(root_key.span.start.line(), 1);
    assert_eq!(root_key.span.start.col(), 0);

    let YamlDataOwned::Mapping(child) = &root_value.data else {
        panic!("expected marked child mapping");
    };
    let (title_key, title_value) = child
        .iter()
        .find(|(key, _)| {
            matches!(&key.data, YamlDataOwned::Representation(text, _, _) if text == "title")
        })
        .expect("missing title key");

    assert_eq!(title_key.span.start.line(), 2);
    assert!(title_key.span.start.col() >= 2);
    assert_eq!(title_value.span.start.line(), 2);
    assert!(title_value.span.start.col() > title_key.span.start.col());
}

#[test]
fn marked_block_scalar_span_starts_at_the_indicator() {
    let value = Parser::parse_marked_str("key: |\n  content\n").unwrap();
    let YamlDataOwned::Mapping(map) = value.data else {
        panic!("expected a mapping");
    };
    let (_, scalar) = map.iter().next().unwrap();
    assert_eq!(scalar.span.start.index(), 5);
}
