#![no_main]
use libfuzzer_sys::fuzz_target;
use rs_yaml::{EmitterConfig, MaxScanAhead, find_comments, lint, streaming};

fuzz_target!(|text: &str| {
    let options = EmitterConfig::default();
    let heap = streaming::format_streaming(text, &options);
    let arena = streaming::format_streaming_arena(text, &options);
    assert_eq!(heap.as_ref().ok(), arena.as_ref().ok());
    if let Ok(ranges) = find_comments(text, MaxScanAhead::DEFAULT) {
        for range in ranges {
            assert!(text.get(range).is_some());
        }
    }
    let _ = lint::Linter::with_config(lint::LintConfig::default()).lint(text);
});
