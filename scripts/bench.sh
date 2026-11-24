#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cargo bench -p rs-yaml --no-default-features --bench yaml_core --manifest-path "$ROOT/Cargo.toml" -- --sample-size 20
cargo bench -p rs-yaml --features streaming --bench yaml_core --manifest-path "$ROOT/Cargo.toml" -- 'rs-yaml/format/' --sample-size 20
cargo bench -p rs-yaml --features "streaming,arena" --bench yaml_core --manifest-path "$ROOT/Cargo.toml" -- 'rs-yaml/format/' --sample-size 20
