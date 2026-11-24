use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use indoc::indoc;
use rs_yaml::{Emitter, EmitterConfig, Parser, ScalarOwned, Value, safe_load};
use saphyr::{LoadableYamlNode, YamlEmitter, YamlOwned};

const SMALL_DOCUMENT: &str = indoc! {r"
name: yaml
enabled: true
count: 3
ratio: 1.5
"};

const NESTED_DOCUMENT: &str = indoc! {r"
service:
  name: api
  version: 1.0
  metadata:
    owner: ops
    regions:
      - us-east
      - eu-west
    limits:
      requests: 1000
      burst: 100
"};

const MERGE_DOCUMENT: &str = indoc! {r"
defaults: &defaults
  adapter: postgres
  host: localhost
  pool: 10

development:
  <<: *defaults
  database: dev_db
"};

const TAGGED_DOCUMENT: &str = indoc! {r"
int: !!int '42'
float: !!float '3'
bool: !!bool 'true'
'null': !!null 'null'
"};

const FORMAT_DOCUMENT: &str = indoc! {r"
title: example
description: |
  first line
  second line
items:
  - one
  - two
"};

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("rs-yaml/parse");
    for (name, input) in [
        ("small", SMALL_DOCUMENT),
        ("nested", NESTED_DOCUMENT),
        ("merge", MERGE_DOCUMENT),
    ] {
        group.bench_with_input(BenchmarkId::new("yaml", name), input, |b, input| {
            b.iter(|| Parser::parse_str(black_box(input)).unwrap());
        });
        group.bench_with_input(BenchmarkId::new("saphyr", name), input, |b, input| {
            b.iter(|| YamlOwned::load_from_str(black_box(input)).unwrap());
        });
    }
    group.finish();
}

fn bench_emit(c: &mut Criterion) {
    let small = Parser::parse_str(SMALL_DOCUMENT).unwrap();
    let nested = Parser::parse_str(NESTED_DOCUMENT).unwrap();
    let mut group = c.benchmark_group("rs-yaml/emit");

    for (name, value) in [("small", &small), ("nested", &nested)] {
        group.bench_with_input(BenchmarkId::new("yaml", name), value, |b, value| {
            b.iter(|| Emitter::emit_str(black_box(value)).unwrap());
        });
        group.bench_with_input(BenchmarkId::new("saphyr", name), value, |b, value| {
            b.iter(|| emit_with_saphyr(black_box(value)).unwrap());
        });
    }

    group.finish();
}

fn bench_safe_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("rs-yaml/safe_load");
    group.bench_function("merge_keys", |b| {
        b.iter(|| safe_load(black_box(MERGE_DOCUMENT)).unwrap());
    });
    group.bench_function("explicit_tags", |b| {
        b.iter(|| safe_load(black_box(TAGGED_DOCUMENT)).unwrap());
    });
    group.finish();
}

fn bench_style_preserving_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("rs-yaml/style_parse");
    group.bench_function("preserving_styles", |b| {
        b.iter(|| Parser::parse_all_preserving_styles(black_box(FORMAT_DOCUMENT)).unwrap());
    });
    group.finish();
}

fn bench_format(c: &mut Criterion) {
    let mut group = c.benchmark_group("rs-yaml/format");
    let config = EmitterConfig::default();

    group.bench_function("emitter_auto", |b| {
        b.iter(|| {
            Emitter::format_with_config(black_box(FORMAT_DOCUMENT), black_box(&config)).unwrap();
        });
    });

    #[cfg(feature = "streaming")]
    group.bench_function("streaming_std", |b| {
        b.iter(|| {
            rs_yaml::streaming::format_streaming(black_box(FORMAT_DOCUMENT), black_box(&config))
                .unwrap();
        });
    });

    #[cfg(feature = "arena")]
    group.bench_function("streaming_arena", |b| {
        b.iter(|| {
            rs_yaml::streaming::format_streaming_arena(
                black_box(FORMAT_DOCUMENT),
                black_box(&config),
            )
            .unwrap();
        });
    });

    group.finish();
}

fn emit_with_saphyr(value: &Value) -> Result<String, saphyr::EmitError> {
    let yaml: saphyr::Yaml = value.into();
    let mut output = String::new();
    let mut emitter = YamlEmitter::new(&mut output);
    emitter.dump(&yaml)?;
    if matches!(value, Value::Value(ScalarOwned::String(_))) && !output.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

criterion_group!(
    benches,
    bench_parse,
    bench_emit,
    bench_safe_load,
    bench_style_preserving_parse,
    bench_format
);
criterion_main!(benches);
