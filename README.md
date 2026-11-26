# rs-yaml

Parse, emit and format YAML in Rust. Use `safe_load` for a single document,
`safe_load_all` for a multi-document stream, or the optional Serde adapter for
typed configuration. Version 0.6.0 requires Rust 1.100 or newer.
[crates.io](https://crates.io/crates/rs-yaml/0.6.0) · [API reference](https://docs.rs/rs-yaml/0.6.0/rs_yaml/)

## Quick start

```toml
[dependencies]
rs-yaml = "0.6.0"
```

```rust
use rs_yaml::{safe_dump, safe_load, safe_load_all};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let doc = safe_load("project: rs-yaml\n")?;
    assert_eq!(safe_load(&safe_dump(&doc)?)?, doc);

    let stream = "---\nproject: api\n---\nproject: worker\n";
    assert!(safe_load(stream).is_err());
    assert_eq!(safe_load_all(stream)?.len(), 2);
    Ok(())
}
```

## Choosing an API

| API | Use |
| --- | --- |
| `Parser`, `Emitter`, `safe_load[_all]`, `safe_dump[_all]` | Read and write the original owned `Value` type. Marked parsing retains source spans. |
| `from_str::<T>`, `to_string` (feature `serde`) | Deserialize and serialize Rust types that implement Serde traits. |
| `decode_input`, `decode_input_owned` | Validate UTF-8 and distinguish unsupported byte encodings. |
| `find_comments`, `has_comments` | Detect YAML comments and access their source ranges before formatting. The formatter discards comments. |
| `fast` | Full fast-yaml-core 0.8 API, including guarded events, parsing limits, sets, arbitrary-size integers, merge policies and emission budgets. |
| `lint` (feature `lint`) | Rule-based diagnostics, configurable rules, source locations and text, JSON and SARIF output. |
| `parallel` (feature `parallel`) | Ordered multi-document parsing, worker limits and file batch operations. |

The default `streaming` feature formats parser events directly. Enable `arena` for the arena-backed formatter. Disabling default features selects the legacy DOM formatter. The three facade modules share the Codixor fork types. For example, `parallel::parse_parallel` returns `Vec<fast::Value>`. When migrating from 0.2, import these types through `rs_yaml` or use the matching `codixor-fast-yaml-core`, `codixor-fast-yaml-linter` and `codixor-fast-yaml-parallel` packages. Direct imports from the original fast-yaml packages have different type identities. The compatibility parser uses a separate `rs_yaml::Value` and returns null for an empty stream.

The compatibility API re-exports Saphyr 0.4 value and span types. Use the same 0.4 series for a direct Saphyr dependency. Marked block scalar spans begin at the `|` or `>` indicator. In 0.6, `ParseError::UnresolvedAlias { at }` carries the location of the reference. Match `UnresolvedAlias { .. }` when only its kind matters.

## Typed configurations

Enable the typed loader and Serde's derive macros in the consuming crate.

```sh
cargo add rs-yaml@0.6.0 --features serde
cargo add serde@1 --features derive
```

```rust
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct Config {
    enabled: bool,
    retries: u8,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config: Config = rs_yaml::from_str("enabled: true\nretries: 3")?;
    let yaml = rs_yaml::to_string(&config)?;
    assert_eq!(rs_yaml::from_str::<Config>(&yaml)?.retries, 3);
    Ok(())
}
```

For a Serde `u32` file mode, write `mode: 0o600` (octal) or `mode: 384` (decimal). The typed loader rejects the legacy spelling `mode: 0600`.

For a boolean field, use `true` or `false`. The loader also accepts their title-case and uppercase forms. It rejects YAML 1.1 spellings such as `yes` and `on`. Explicit tags must resolve to one of the seven YAML core tags, such as `!!str`. Local tags such as `!vault` and enum tags such as `!Low` return `ParseError::UnsupportedTag` with a source location.

Unknown-field and unknown-variant diagnostics use fixed descriptions. Numeric type errors can identify the expected primitive type, such as `u16`. Keys, values and custom visitor messages stay out of returned errors, including their debug output and sources.

## Handling input

`safe_load`, `Parser::parse_str`, `Parser::parse_marked_str` and typed `from_str` reject a second document, including an empty one. Use `safe_load_all` or `Parser::parse_all_marked` to read a stream. Each mapping must have unique keys. Quoting a string key, changing an integer's spelling, or referring to a key by alias does not make it distinct. An explicit key may override one inherited through a merge. Duplicate-key errors give both source positions. Compatibility parser errors give locations and redact input text.

The typed adapter uses serde-saphyr after the compatibility checks. The `fast`, `lint` and `parallel` re-exports retain their own parsing and diagnostic policies.

The compatibility and typed loaders use `YAML_LIMITS`. They cap the whole stream at 8 MiB, expanded nesting at 64 nodes (including scalar leaves), document count at 4,096, and estimated materialization work at 32 MiB. The estimate charges 128 bytes per node plus scalar and tag payload bytes, saved anchors and alias copies. These safety ceilings remain active when a caller supplies additional limits. The typed backend and Saphyr also retain their own protections.

Use the `_with_limits` form of each guarded parser or validation method, `safe_load[_all]_with_limits`, or typed `from_str_with_limits::<T>`. Each takes `&rs_format::Limits`. The crate re-exports the shared types. For example:

```rust
use rs_yaml::{Category, FormatError, Limits, safe_load_with_limits};

let limits = Limits { max_token_bytes: 3, ..Limits::default() };
let error = safe_load_with_limits("name: x", &limits).unwrap_err();
assert_eq!(error.category(), Category::Limit);
assert_eq!(error.position().unwrap().offset, Some(3));
```

- `max_document_bytes` counts source bytes per document, including its separator and line endings. An implicit document ends at the next document marker. An explicit `...` ends after its line terminator. Leading and trailing stream trivia count against the adjacent document.
- `max_depth` counts expanded containers. A scalar has depth zero and a root collection has depth one. Aliases must also fit at their point of use.
- `max_token_bytes` counts UTF-8 bytes in the encoded body of each scalar, key, comment, property name or directive word. Quotes and the comment indicator are excluded. Escapes and doubled quotes count as written. A block scalar body begins after its header line and includes its physical indentation and newlines. Saphyr can buffer a token before reporting its span. The 8 MiB input ceiling bounds this scanner input independently.
- `max_expansion` charges the materialization estimate of the target at each alias use and of the source at each merge. An alias used for a merge incurs both charges. Ordinary nodes and anchor definitions alone have zero expansion cost. The expansion budget resets at each document.

Both `ParseError` and `EmitError` implement `rs_format::FormatError`. A known position uses a zero-based byte offset and one-based line and byte column. Duplicate-key errors retain both positions and report the duplicate through the trait. A normalization error with no source span reports no position. In 0.5 the error variants no longer carry a duplicate key, invalid value or raw scanner/Serde error.

The `fast`, `lint` and `parallel` facades expose their upstream policies and diagnostics. Applications reading a file or HTTP body should bound incoming bytes before collecting the string.

## Development

Run `bash scripts/validate.sh` or use Cargo directly. CI tests default, minimal and all-feature builds on Rust 1.100, runs Clippy with warnings denied, checks both dependency graphs with cargo-deny and exercises two bounded AddressSanitizer fuzz targets on a pinned nightly. The Criterion suite is available through `bash scripts/bench.sh`.

Originally developed in the `codixor/rust-misc` workspace at `ae68abb`. The standalone crate also exposes Codixor-maintained forks of the fast-yaml libraries. Both projects use the MIT or Apache-2.0 license at your option. [Upstream fast-yaml](https://github.com/bug-ops/fast-yaml)

## Releases

Bump `workspace.package.version`, the three internal dependency pins, and the fuzz manifest's rs-yaml requirement when crossing a compatibility boundary. Refresh both lockfiles. All four published packages share the release version. Pushes to `main` run the checks only. To release, push a tag named `v<version>` at the release commit. The tag must match `workspace.package.version`.

After the tag's checks pass, the workflow publishes the core, linter and parallel forks, followed by rs-yaml, through the GitHub `cratesio` environment. It then creates the GitHub release for the pushed tag. Rerunning the workflow verifies already-published packages and resumes any unfinished steps.
