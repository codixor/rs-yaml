# rs-yaml

A Rust 1.100 library for parsing, emitting, formatting and linting YAML. The compatibility API re-exports Saphyr 0.4 value and span types. Callers that also name those types through a direct Saphyr dependency should use the 0.4 series. Marked block scalar spans start at the `|` or `>` indicator. The standalone crate also exposes the Rust libraries from [fast-yaml](https://github.com/bug-ops/fast-yaml).

```toml
[dependencies]
rs-yaml = "0.1.1"
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
| `decode_input`, `decode_input_owned` | Validate UTF-8 and distinguish unsupported byte encodings. |
| `find_comments`, `has_comments` | Detect YAML comments and access their source ranges before formatting. The formatter discards comments. |
| `fast` | Full fast-yaml-core 0.8 API, including guarded events, parsing limits, sets, arbitrary-size integers, merge policies and emission budgets. |
| `lint` (feature `lint`) | Rule-based diagnostics, configurable rules, source locations and text, JSON and SARIF output. |
| `parallel` (feature `parallel`) | Ordered multi-document parsing, worker limits and file batch operations. |

The default `streaming` feature formats parser events directly. Enable `arena` for the arena-backed formatter. Disabling default features selects the legacy DOM formatter. The facade modules preserve upstream type identity. For example, `parallel::parse_parallel` returns `Vec<fast::Value>`. The compatibility parser uses a separate `rs_yaml::Value` and returns null for an empty stream.

## Handling input

Compatibility loaders cap each input at 8 MiB, expanded nesting at 64 nodes, streams at 4,096 documents, and estimated materialization work at 32 MiB. The estimate charges nodes, text, tags, stored anchors and alias copies. A limit violation returns `ParseError::ResourceLimit`. Use `fast::Parser::parse_all_with_limits` for configurable upstream budgets. Applications reading external files or requests should bound the incoming bytes before collecting them into a string.

## Development

Run `bash scripts/validate.sh` or use Cargo directly. CI tests default, minimal and all-feature builds on Rust 1.100, runs Clippy with warnings denied, checks both dependency graphs with cargo-deny and exercises two bounded AddressSanitizer fuzz targets on a pinned nightly. The Criterion suite is available through `bash scripts/bench.sh`.

Imported from [codixor/rust-misc at ae68abb](https://github.com/codixor/rust-misc/tree/ae68abb54c655a8fcc009123fbc31821b82435e4/rs-yaml). The original design incorporates work by the fast-yaml contributors. Both projects use the MIT or Apache-2.0 license at your option.

## Releases

Bump the package version in `Cargo.toml` and refresh both Cargo lockfiles. Pushing to `main` or pushing a matching `v<version>` tag runs the checks and publishes the new version through the GitHub `cratesio` environment. The workflow creates the matching GitHub release and tag. Further pushes with an already released version skip publication.
