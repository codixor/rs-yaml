use rs_format::{Category, FormatError, Limits};
use rs_yaml::{Parser, YAML_LIMITS};
use std::error::Error;

fn check_all(input: &str, limits: Limits, accepted: bool) {
    fn check<T>(result: Result<T, rs_yaml::ParseError>, accepted: bool, input: &str) {
        match result {
            Ok(_) => assert!(accepted, "unexpected acceptance: {input:?}"),
            Err(e) => {
                assert!(!accepted, "{input:?}: {e}");
                assert_eq!(e.category(), Category::Limit, "{input:?}: {e:?}");
                assert!(e.position().is_some(), "{input:?}: {e:?}");
            }
        }
    }
    check(
        rs_yaml::safe_load_with_limits(input, &limits),
        accepted,
        input,
    );
    check(
        rs_yaml::safe_load_all_with_limits(input, &limits),
        accepted,
        input,
    );
    check(
        Parser::parse_marked_str_with_limits(input, &limits),
        accepted,
        input,
    );
    check(
        Parser::parse_all_marked_with_limits(input, &limits),
        accepted,
        input,
    );
    check(
        Parser::parse_all_preserving_styles_with_limits(input, &limits),
        accepted,
        input,
    );
    check(
        Parser::validate_no_duplicate_mapping_keys_with_limits(input, &limits),
        accepted,
        input,
    );
    #[cfg(feature = "serde")]
    check(
        rs_yaml::from_str_with_limits::<serde::de::IgnoredAny>(input, &limits),
        accepted,
        input,
    );
}

#[test]
fn encoded_token_body_boundaries() {
    for (input, size) in [
        ("{}", 0),
        ("[]", 0),
        ("''", 0),
        ("{c: ''}", 1),
        ("'éé'", 4),
        ("éé: x", 4),
        ("123456", 6),
        (r#""\u0061""#, 6),
        ("'a''b'", 4),
        ("'x'    #abcd", 4),
        ("#éé\r\n", 4),
        ("{x: 'a#b'}", 3),
        ("abc\n def", 8),
        ("|\n  a\n", 4),
        ("| #abcdef\n  x\n", 6),
        ("key: &abc 'small'", 5),
        ("&lengthy [ok]", 7),
        ("[&abcd x, *abcd]", 4),
        ("!!str 'h'", 4),
        ("%YAML 1.2\n---\nx", 4),
    ] {
        check_all(
            input,
            Limits {
                max_token_bytes: size,
                ..YAML_LIMITS
            },
            true,
        );
        if size > 0 {
            check_all(
                input,
                Limits {
                    max_token_bytes: size - 1,
                    ..YAML_LIMITS
                },
                false,
            );
        }
    }
}

#[test]
fn inclusive_depth_and_bytes() {
    for (input, depth) in [("word", 0), ("{x: y}", 1), ("[[]]", 2), ("{x: [y]}", 2)] {
        check_all(
            input,
            Limits {
                max_depth: depth,
                ..YAML_LIMITS
            },
            true,
        );
        if depth > 0 {
            check_all(
                input,
                Limits {
                    max_depth: depth - 1,
                    ..YAML_LIMITS
                },
                false,
            );
        }
        check_all(
            input,
            Limits {
                max_document_bytes: input.len() as u64,
                ..YAML_LIMITS
            },
            true,
        );
        check_all(
            input,
            Limits {
                max_document_bytes: input.len() as u64 - 1,
                ..YAML_LIMITS
            },
            false,
        );
    }
    check_all(
        "",
        Limits {
            max_document_bytes: 0,
            ..YAML_LIMITS
        },
        true,
    );
}

#[test]
fn per_document_budgets_reset_at_real_boundaries() {
    let doc = "---\n[&a x, *a]\n";
    let input = doc.repeat(2);
    let limits = Limits {
        max_expansion: 129,
        max_document_bytes: doc.len() as u64,
        ..YAML_LIMITS
    };
    assert_eq!(
        Parser::parse_all_with_limits(&input, &limits)
            .unwrap()
            .len(),
        2
    );
    for limits in [
        Limits {
            max_expansion: 128,
            ..limits
        },
        Limits {
            max_document_bytes: doc.len() as u64 - 1,
            ..limits
        },
    ] {
        assert_eq!(
            Parser::parse_all_with_limits(&input, &limits)
                .unwrap_err()
                .category(),
            Category::Limit
        );
    }
    let zero = Limits {
        max_expansion: 0,
        ..YAML_LIMITS
    };
    check_all("[&a x, y]", zero, true);
    check_all("[&a x, *a]", zero, false);
    check_all("{'<<': {a: b}}", zero, true);
    check_all("{<<: {a: b}}", zero, false);
    let explicit = "a\n...\n---\nb\n";
    assert_eq!(
        Parser::parse_all_with_limits(
            explicit,
            &Limits {
                max_document_bytes: 6,
                ..YAML_LIMITS
            }
        )
        .unwrap()
        .len(),
        2
    );
}

#[test]
fn compatibility_protections_and_contract() {
    assert_eq!(YAML_LIMITS.max_document_bytes, 8 * 1024 * 1024);
    assert_eq!(YAML_LIMITS.max_depth, 64);
    let too_deep = format!("{}x{}", "[".repeat(64), "]".repeat(64));
    assert_eq!(
        rs_yaml::safe_load(&too_deep).unwrap_err().category(),
        Category::Limit
    );
    let limits = Limits {
        max_depth: 3,
        ..YAML_LIMITS
    };
    check_all("[&a [[x]], [*a]]", limits, false);
    assert_eq!(
        rs_yaml::safe_load_with_limits("x\n---\ny", &limits)
            .unwrap_err()
            .category(),
        Category::Data
    );
}

fn redacted(error: &dyn Error) {
    let mut current = Some(error);
    while let Some(e) = current {
        for msg in [format!("{e}"), format!("{e:?}"), format!("{e:#?}")] {
            assert!(!msg.contains("private-fixture-marker"), "{msg}");
        }
        current = e.source();
    }
}

#[test]
fn utf8_duplicate_locations_are_absolute_byte_coordinates() {
    let input = "x\r\n---\r\n{é: q, private-fixture-marker: 1, private-fixture-marker: 2}";
    let error = Parser::parse_all(input).unwrap_err();
    redacted(&error);
    let pos = error.position().unwrap();
    let offset = input.rfind("private-fixture-marker").unwrap();
    assert_eq!(pos.offset, Some(offset as u64));
    assert_eq!(pos.line.unwrap().get(), 3);
    assert_eq!(
        pos.column.unwrap().get() as usize,
        offset - input.rfind('\n').unwrap()
    );
}

#[cfg(feature = "serde")]
#[test]
fn typed_unknown_keys_have_safe_positions_and_errors() {
    #[derive(Debug, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Config {
        #[allow(dead_code)]
        mode: u8,
    }
    let err = rs_yaml::from_str::<Config>("private-fixture-marker: 1").unwrap_err();
    assert_eq!(err.category(), Category::Data);
    assert_eq!(err.position().unwrap().offset, Some(0));
    redacted(&err);
    let err = rs_yaml::from_str::<Config>("mode: private-fixture-marker").unwrap_err();
    assert_eq!(err.category(), Category::Data);
    redacted(&err);
}

#[test]
fn syntax_eof_and_formatter_diagnostics_discard_backend_text() {
    for (input, category) in [
        ("{a: [private-fixture-marker}", Category::Syntax),
        ("'private-fixture-marker", Category::Eof),
        ("[private-fixture-marker", Category::Eof),
        ("*private-fixture-marker", Category::Data),
        ("!!private-fixture-marker invalid", Category::Data),
        ("<<: private-fixture-marker", Category::Data),
    ] {
        let err = rs_yaml::safe_load(input).unwrap_err();
        assert_eq!(err.category(), category, "{input}: {err:?}");
        redacted(&err);
    }
    let error = rs_yaml::Emitter::format("private-fixture-marker: 1\nprivate-fixture-marker: 2")
        .unwrap_err();
    assert_eq!(error.category(), Category::Data);
    assert_eq!(error.position().unwrap().line.unwrap().get(), 2);
    redacted(&error);
}
