mod support;

use rs_yaml::Emitter;

#[test]
fn format_preserves_directives_and_block_scalar_style() {
    let input = support::fixture("format/directives_and_blocks.yaml");
    let output = Emitter::format(&input).unwrap();

    assert!(output.starts_with("%YAML 1.2\n---\n"));
    assert!(output.contains("description: |"));
    assert!(output.contains("  first line"));
    assert!(output.contains("  second line"));
}

#[test]
fn format_is_explicitly_lossy_for_comments_today() {
    let input = support::fixture("format/comments.yaml");
    let output = Emitter::format(&input).unwrap();

    assert!(output.contains("key: value"));
    assert!(output.contains("child: 1"));
    assert!(!output.contains("# header comment"));
    assert!(!output.contains("# inline comment"));
    assert!(!output.contains("# nested comment"));
}
