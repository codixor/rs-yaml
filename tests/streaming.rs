#[cfg(feature = "streaming")]
use rs_yaml::EmitterConfig;
#[cfg(all(feature = "arena", feature = "streaming"))]
use rs_yaml::streaming::format_streaming_arena;
#[cfg(feature = "streaming")]
use rs_yaml::streaming::{format_streaming, is_streaming_suitable};

#[cfg(feature = "streaming")]
#[test]
fn streaming_format_handles_simple_mapping_sequence_and_nested_shapes() {
    let config = EmitterConfig::default();

    let mapping = format_streaming("key: value\n", &config).unwrap();
    assert!(mapping.contains("key: value"));

    let sequence = format_streaming("- item1\n- item2\n", &config).unwrap();
    assert!(sequence.contains("- item1"));
    assert!(sequence.contains("- item2"));

    let nested = format_streaming("outer:\n  inner: value\n", &config).unwrap();
    assert!(nested.contains("outer:"));
    assert!(nested.contains("inner: value"));
}

#[cfg(feature = "streaming")]
#[test]
fn streaming_format_preserves_float_text() {
    let output = format_streaming("version: 1.0\n", &EmitterConfig::default()).unwrap();
    assert!(output.contains("version: 1.0"));
}

#[cfg(feature = "streaming")]
#[test]
fn streaming_suitability_rejects_anchor_dense_inputs() {
    let input = "&a 1\n&a 2\n*a\n*a\n";
    assert!(!is_streaming_suitable(input));
}

#[cfg(feature = "streaming")]
#[test]
fn streaming_format_preserves_anchor_names_and_sequence_shapes() {
    let input = "defaults: &base\n  adapter: postgres\nservice:\n  <<: *base\n  tags:\n    - api\n    - stable\n";
    let output = format_streaming(input, &EmitterConfig::default()).unwrap();

    assert!(output.contains("&base"));
    assert!(output.contains("*base"));
    assert!(output.contains("- api"));
    assert!(output.contains("- stable"));
}

#[cfg(feature = "streaming")]
#[test]
fn streaming_format_preserves_block_scalar_keep_chomp() {
    let input = "description: |+\n  line one\n  line two\n\n";
    let output = format_streaming(input, &EmitterConfig::default()).unwrap();

    assert!(output.contains("description: |+"));
    assert!(output.contains("  line one"));
    assert!(output.contains("  line two"));
}

#[cfg(all(feature = "arena", feature = "streaming"))]
#[test]
fn arena_backend_matches_standard_streaming_output() {
    let input = "service:\n  version: 1.0\n  replicas: 3\n";
    let config = EmitterConfig::default();

    let standard = format_streaming(input, &config).unwrap();
    let arena = format_streaming_arena(input, &config).unwrap();

    assert_eq!(standard, arena);
}

#[cfg(all(feature = "arena", feature = "streaming"))]
#[test]
fn arena_backend_matches_standard_on_deep_and_anchor_heavy_inputs() {
    let input = "a: &foo\n  x: 1\nb: &bar\n  y: 2\nc: *foo\nd: *bar\nnested:\n  list:\n    - one\n    - two\n";
    let config = EmitterConfig::default();

    let standard = format_streaming(input, &config).unwrap();
    let arena = format_streaming_arena(input, &config).unwrap();

    assert_eq!(standard, arena);
}
