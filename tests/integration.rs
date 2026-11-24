use rs_yaml::{Emitter, EmitterConfig, ParseError, Parser, ScalarOwned, Value, fast};

#[test]
fn comments_decoding_and_budgeted_loading() {
    let text = rs_yaml::decode_input(b"\xEF\xBB\xBFname: '# kept' # note\n").unwrap();
    let spans = rs_yaml::find_comments(text, rs_yaml::MaxScanAhead::DEFAULT).unwrap();
    assert_eq!(
        spans.iter().map(|r| &text[r.clone()]).collect::<Vec<_>>(),
        ["# note"]
    );
    assert!(rs_yaml::decode_input(b"\xff\xfe").is_err());
    let limits = rs_yaml::ParseLimits {
        max_documents: fast::MaxDocuments::new(1).unwrap(),
        ..Default::default()
    };
    assert!(fast::Parser::parse_all_with_limits("---\none\n---\ntwo\n", &limits).is_err());
}

#[test]
fn quoted_merge_key_and_flow_strings_round_trip() {
    let value = rs_yaml::safe_load(r#"{"<<": 42, "a\"b:c": "inf"}"#).unwrap();
    assert_eq!(value.as_mapping().unwrap().len(), 2);
    let flow = EmitterConfig::default().with_default_flow_style(Some(true));
    assert_eq!(
        rs_yaml::safe_load(&Emitter::emit_str_with_config(&value, &flow).unwrap()).unwrap(),
        value
    );
    assert_eq!(
        rs_yaml::safe_load(&rs_yaml::safe_dump(&value).unwrap()).unwrap(),
        value
    );
}

#[test]
fn scalar_types_and_styles_survive_emission() {
    let marked_style = Parser::parse_all_preserving_styles("|\n  sample\n").unwrap();
    assert_eq!(
        rs_yaml::safe_load(&rs_yaml::safe_dump_all(&marked_style).unwrap())
            .unwrap()
            .as_str(),
        Some("sample\n")
    );
    let values = vec![
        Value::Value(ScalarOwned::String("---prefix".into())),
        Value::Value(ScalarOwned::String("literal:\ninf".into())),
        Value::Value(ScalarOwned::FloatingPoint(rs_yaml::OrderedFloat(1.0))),
    ];
    assert_eq!(
        rs_yaml::safe_load_all(&rs_yaml::safe_dump_all(&values).unwrap()).unwrap(),
        values
    );
}

#[test]
fn compatibility_entry_points_share_the_depth_limit() {
    let document = format!("{}0{}", "[".repeat(65), "]".repeat(65));
    assert!(matches!(
        Parser::parse_str(&document),
        Err(ParseError::ResourceLimit(_))
    ));
    assert!(matches!(
        Parser::parse_marked_str(&document),
        Err(ParseError::ResourceLimit(_))
    ));
    assert!(matches!(
        Parser::parse_all_preserving_styles(&document),
        Err(ParseError::ResourceLimit(_))
    ));
    assert!(matches!(
        Parser::validate_no_duplicate_mapping_keys(&document),
        Err(ParseError::ResourceLimit(_))
    ));
}

#[cfg(feature = "lint")]
#[test]
fn reports_lint_findings_through_the_public_api() {
    let linter = rs_yaml::lint::Linter::with_config(Default::default());
    assert!(!linter.lint("a: 1  \n").unwrap().is_empty());
}

#[cfg(feature = "parallel")]
#[test]
fn parallel_parser_preserves_stream_order() {
    use rs_yaml::parallel::{Config, Workers, parse_parallel_with_config};
    let cfg = Config::new()
        .with_workers(Workers::from_count(2).unwrap())
        .with_sequential_threshold(1);
    let input = "---\nfirst\n---\nsecond\n";
    assert_eq!(
        parse_parallel_with_config(input, &cfg).unwrap(),
        fast::Parser::parse_all(input).unwrap()
    );
}

#[test]
fn block_scalar_header_comments_have_source_byte_ranges() {
    let yaml = "label: >- # première\r\n  literal # content\r\nnext: 1 # last\r\n";
    let ranges = rs_yaml::find_comments(yaml, rs_yaml::MaxScanAhead::DEFAULT).unwrap();
    let comments: Vec<_> = ranges.into_iter().map(|range| &yaml[range]).collect();
    assert_eq!(comments, ["# première", "# last"]);
}
