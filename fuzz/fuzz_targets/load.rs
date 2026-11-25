#![no_main]
use libfuzzer_sys::fuzz_target;
use rs_yaml::{Parser, decode_input, fast, safe_dump_all, safe_load_all};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = decode_input(data) else { return };
    if let Ok(values) = safe_load_all(text) {
        let _ = safe_dump_all(&values);
    }
    if let Ok(items) = rs_yaml::from_str::<Vec<String>>(text) {
        let _ = rs_yaml::to_string(&items);
    }
    let limits = rs_yaml::Limits {
        max_document_bytes: u64::from(data.first().copied().unwrap_or(0)),
        max_token_bytes: usize::from(data.get(1).copied().unwrap_or(0)),
        max_depth: u32::from(data.get(2).copied().unwrap_or(0) % 10),
        max_expansion: 2 * u64::from(data.get(3).copied().unwrap_or(0)),
    };
    let _ = Parser::parse_all_with_limits(text, &limits);
    let _ = rs_yaml::from_str_with_limits::<Vec<String>>(text, &limits);
    let _ = Parser::parse_all_marked(text);
    let _ = Parser::validate_no_duplicate_mapping_keys(text);
    let _ = fast::Parser::parse_all(text);
});
