# Historical rust-misc import plan

This plan records work in the parent monorepo. See [README.md](README.md) for the supported standalone features and commands.

Goal: turn `rs-yaml` into the workspace’s main YAML parser/emitter crate with a
small stable public surface, crate-root proof tests, reproducible benchmarks,
typed `thiserror` errors, and measured `kdl` interop.

## Direction

- [x] Keep the crate library-first.
- [x] Keep the core public API centered on:
  - [x] `Parser`
  - [x] `Emitter`
  - [x] `Value`
  - [x] `ParseError`
  - [x] `EmitError`
- [x] Keep `thiserror` as the public error model.
- [x] Align all public naming and docs to `rs-yaml`, not donor names like
  `fast_yaml_core`.
- [x] Add crate-root proof tests and golden fixtures.
- [x] Add examples, validation scripts, and a cleaner benchmark matrix.
- [ ] Use performance helper crates only after measurement.

## Current Ownership Decision

- [x] Treat this crate as first-party workspace code, not a frozen donor.
- [x] Keep the current `saphyr`/`saphyr-parser` foundation for now.
- [ ] Revisit replacement only if correctness, maintenance, or measured
  performance makes ownership unattractive later.

## Stage 1: Workspace, Naming, And Manifest Cleanup

- [x] Align `Cargo.toml` to workspace metadata:
  - [x] edition
  - [x] rust-version
  - [x] license
  - [x] version
  - [x] repository
  - [x] description
  - [x] keywords/categories
- [x] Add `README.md`.
- [x] Add `PLAN.md`.
- [x] Audit docs and examples for old crate names:
  - [x] `fast_yaml_core`
  - [x] any stale donor wording
- [x] Rename the current bench surface to match the crate shape better than
  `arena_benchmark`.

## Stage 2: Public API And Error Surface

- [x] Keep `thiserror` in [error.rs](./src/error.rs).
- [x] Keep split public errors exactly as-is:
  - [x] `ParseError`
  - [x] `EmitError`
- [x] Skip an umbrella `YamlError` for now so the public API stays small.
- [ ] Keep error payloads lightweight and stable:
  - [x] parse location
  - [x] scanner/source errors
  - [x] emitter configuration/unsupported-shape errors
- [x] Re-export the minimal surface clearly at crate root.

## Stage 3: Parser, Value, And Emitter Cleanup

- [x] Keep the crate centered on value parsing and emission.
- [ ] Audit the parser API in [parser.rs](./src/parser.rs):
  - [x] `parse_str`
  - [x] `parse_all`
  - [x] style-preserving parse path
  - [x] keep the style-preserving path public as an additive formatting helper
- [ ] Audit the emitter API in [emitter.rs](./src/emitter.rs):
  - [x] `emit_str`
  - [x] `emit_str_with_config`
  - [x] `emit_all_with_config`
  - [x] tighten the config story so compatibility knobs and actually-supported
    knobs are clearer
- [x] Keep direct `saphyr` value re-exports for now so the public surface stays
  lightweight.

## Stage 4: Tests To Crate-Root And Golden Matrix

- [x] Add focused crate-root tests for:
  - [x] parse basics
  - [x] emit basics
  - [x] scalar handling
  - [x] multiline/style preservation
  - [x] streaming formatter
  - [x] error behavior
- [x] Add golden fixtures under `tests/golden/` for:
  - [x] simple mappings/sequences
  - [x] multiline scalars
  - [x] anchors and aliases
  - [x] canonicalization edge cases
- [x] Add explicit regression coverage for:
  - [x] comment-only null documents
  - [x] merge key resolution
  - [x] explicit core tags on quoted scalars
  - [x] comment-loss formatting limitation

## Stage 5: Benchmark Matrix

- [x] Replace the current single bench story with one cleaner Criterion surface.
- [x] Add benchmark groups for:
  - [x] parse small document
  - [x] parse nested document
  - [x] emit small document
  - [x] emit nested document
  - [x] style-preserving parse
  - [x] streaming formatter
  - [x] std backend vs arena backend
- [x] Add comparison baselines where useful:
  - [x] direct `saphyr` parse/emission
  - [x] current crate baseline

## Stage 6: Performance Pass

- [ ] Profile before changing parser/emitter internals.
- [ ] Keep the current safe implementation as the baseline.
- [x] Only adopt helper crates if they produce a validated `>=30%` win on a
  real hot path.
- [ ] Candidate measured upgrades:
  - [x] `memchr` is already present
  - [ ] more targeted `memchr` use in hot scalar scanning if profiling justifies it
  - [x] default formatter now uses `streaming` because it benchmarked at about
    `34-41%` faster than the DOM fallback
  - [x] `bumpalo` arena backend stays optional because it did not clear the
    adoption bar over standard streaming
  - [ ] output preallocation tuning in emitter paths only if bench data justifies it

## Stage 7: Examples, Docs, And Validation Scripts

- [x] Add examples for:
  - [x] parse a document
  - [x] emit a document
  - [x] preserve block scalar styles
  - [x] stream-format a document
- [x] Add validation and benchmark scripts.
- [ ] Run and record:
  - [x] `cargo test`
  - [x] `cargo test --examples`
  - [x] `cargo clippy --all-targets --all-features -D warnings`
  - [x] `cargo doc --no-deps`
  - [x] `cargo check --benches`

## Stage 8: KDL Interop Contract

- [x] Add YAML interop additively on top of the neutral `kdl` model.
- [x] Keep conversion through a shared model, not raw text bridging.
- [x] Add YAML text -> neutral interop conversion.
- [x] Add YAML value -> neutral interop conversion.
- [x] Add neutral interop -> YAML text conversion.
- [x] Add a generated YAML-on-KDL contract path for callers that actually need
  a `rs_kdl::Document`.
- [x] Add proof tests for both directions:
  - [x] YAML text -> interop -> YAML text
  - [x] YAML value -> interop
  - [x] YAML text -> KDL contract -> YAML text
  - [x] scalar YAML root through the KDL contract
  - [x] invalid contract rejection
  - [x] non-string map keys rejected
  - [x] tagged values rejected
- [x] Benchmark both paths before adopting the long-term owner.
- [x] Prefer the path that keeps YAML parse/emit performance isolated from
  interop costs.
- [x] Current measured ownership decision:
  - [x] direct shared-model YAML bridge is preferred
  - [x] generated YAML-on-KDL contract stays additive for callers that need a `rs_kdl::Document`
  - [x] YAML text rendering preserves interop numeric lexemes so `1.0` stays `1.0`

## Success Criteria

- [ ] `rs-yaml` is the clear main YAML parser/emitter crate in the workspace.
- [x] Public errors use `thiserror`.
- [x] Crate-root proof tests exist.
- [x] Golden fixtures protect parse and emit behavior.
- [x] Benchmark matrix exists and is reproducible.
- [x] Performance helpers and fast paths are adopted only where measurement justifies them.
- [x] `kdl` interop is added through a measured shared-model + contract layer,
  not ad-hoc text conversion.
