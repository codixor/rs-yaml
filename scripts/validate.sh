#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cargo test -p rs-yaml --manifest-path "$ROOT/Cargo.toml"
cargo test -p rs-yaml --examples --manifest-path "$ROOT/Cargo.toml"
cargo test -p rs-yaml --no-default-features --manifest-path "$ROOT/Cargo.toml"
cargo test -p rs-yaml --no-default-features --examples --manifest-path "$ROOT/Cargo.toml"
cargo test -p rs-yaml --features "streaming,arena" --manifest-path "$ROOT/Cargo.toml"
cargo test -p rs-yaml --features "streaming,arena" --examples --manifest-path "$ROOT/Cargo.toml"
cargo clippy -p rs-yaml --all-targets --all-features --manifest-path "$ROOT/Cargo.toml" -- -D warnings
cargo doc -p rs-yaml --no-deps --manifest-path "$ROOT/Cargo.toml"
cargo check -p rs-yaml --all-features --benches --manifest-path "$ROOT/Cargo.toml"
bash "$ROOT/scripts/feature-matrix.sh"
