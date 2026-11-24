# rs-yaml

A Rust 1.100 library for parsing, emitting, formatting and linting YAML. The compatibility API re-exports Saphyr 0.4 value and span types. Callers that also name those types through a direct Saphyr dependency should use the 0.4 series. Marked block scalar spans start at the `|` or `>` indicator. The standalone crate also exposes Codixor-maintained forks of the Rust libraries from [fast-yaml](https://github.com/bug-ops/fast-yaml).

```toml
[dependencies]
rs-yaml = "0.4.0"
```

```rust
use rs_yaml::{safe_dump, safe_load};

let doc = safe_load("project: rs-yaml\n")?;
assert_eq!(doc.as_mapping().unwrap().len(), 1);
assert_eq!(safe_load(&safe_dump(&doc)?)?, doc);
# Ok::<(), Box<dyn std::error::Error>>(())
```

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

## Typed configurations

Enable the `serde` feature on `rs-yaml` and the `derive` feature on `serde` to use derived Rust types.

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

An unknown-field error can name the misspelled mapping key and list the allowed fields. A type error can identify the expected type, such as `u16`. Input values, unknown variant names and custom visitor messages are omitted.

## Handling input

`safe_load`, `Parser::parse_str`, `Parser::parse_marked_str` and typed `from_str` reject a second document, including an empty one. Use `safe_load_all` or `Parser::parse_all_marked` to read a stream. Each mapping must have unique keys. Quoting a string key, changing an integer's spelling, or referring to a key by alias does not make it distinct. An explicit key may override one inherited through a merge. Duplicate-key errors give both source positions. Compatibility parser errors give locations and redact input text.

The typed adapter uses serde-saphyr after the compatibility checks. The `fast`, `lint` and `parallel` re-exports retain their own parsing and diagnostic policies.

Compatibility loaders cap each input at 8 MiB, expanded nesting at 64 nodes, streams at 4,096 documents, and estimated materialization work at 32 MiB. The estimate charges nodes, text, tags, stored anchors and alias copies. A limit violation returns `ParseError::ResourceLimit`, including when Saphyr reaches its own flow-nesting or alias-expansion limit. Use `fast::Parser::parse_all_with_limits` for configurable upstream budgets. Applications reading external files or requests should bound the incoming bytes before collecting them into a string.

## Development

Run `bash scripts/validate.sh` or use Cargo directly. CI tests default, minimal and all-feature builds on Rust 1.100, runs Clippy with warnings denied, checks both dependency graphs with cargo-deny and exercises two bounded AddressSanitizer fuzz targets on a pinned nightly. The Criterion suite is available through `bash scripts/bench.sh`.

Imported from [codixor/rust-misc at ae68abb](https://github.com/codixor/rust-misc/tree/ae68abb54c655a8fcc009123fbc31821b82435e4/rs-yaml). The original design incorporates work by the fast-yaml contributors. Both projects use the MIT or Apache-2.0 license at your option.

## Releases

Bump `workspace.package.version`, the three internal dependency pins, and the fuzz manifest's rs-yaml requirement when crossing a compatibility boundary. Refresh both lockfiles. All four published packages share the release version. Pushing to `main` or pushing a matching `v<version>` tag runs the checks and publishes the core, linter and parallel forks followed by rs-yaml through the GitHub `cratesio` environment. The workflow creates the matching GitHub release and tag. Further pushes with an already released version skip publication.
