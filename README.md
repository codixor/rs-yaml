# rs-yaml

`rs-yaml` is the workspace YAML parser/emitter crate.

It stays library-first and intentionally small:

- `Parser`
- `Emitter`
- `Value`
- `ParseError`
- `EmitError`
- convenience helpers `safe_load` and `safe_load_all`

The formatter path is additive, but the measured fast path is now the default:

- default build: `streaming` formatter enabled
- `--no-default-features`: DOM fallback path
- `--features streaming,arena`: same public API, arena backend preferred for
  formatting internals

## Current behavior guarantees

The crate now has explicit proof coverage for the high-value parser regressions:

- comment-only and truly empty documents load as YAML null
- YAML merge keys `<<` are resolved during `safe_load`
- explicit core tags like `!!int`, `!!float`, `!!bool`, and `!!null` override
  quoted scalar style correctly

Formatting is intentionally documented as lossy for comments today. The
formatter preserves directives and block scalar style, but it does not preserve
comments yet.

## Examples

- `cargo run -p rs-yaml --example parse_document`
- `cargo run -p rs-yaml --example emit_document`
- `cargo run -p rs-yaml --example safe_load`
- `cargo run -p rs-yaml --example format_document`
- `cargo run -p rs-yaml --example streaming_format --features streaming`

## Validation

- `bash rs-yaml/scripts/validate.sh`
- `bash rs-yaml/scripts/feature-matrix.sh`
- `bash rs-yaml/scripts/bench.sh`

## Benchmarks

The Criterion surface covers:

- parser hot paths
- emitter hot paths
- `safe_load` regression paths
- formatter comparisons across DOM, streaming, and arena backends
- direct `saphyr` baselines for parse and emit

Current measured decision:

- default streaming formatter improved `Emitter::format_with_config` by roughly
  `34-41%` compared with the DOM/default fallback on the same fixture
- arena formatting stayed only marginally faster than standard streaming, so it
  remains optional instead of becoming a default dependency choice

See [benches/README.md](./benches/README.md) for the bench entry points.

## Interop rule

Future `kdl` interop should follow the same rule we used for JSON:

- keep a shared semantic bridge where it is truly lossless
- keep any generated YAML-on-KDL contract additive
- benchmark both paths before choosing the default long-term owner

The execution roadmap lives in [PLAN.md](./PLAN.md).
