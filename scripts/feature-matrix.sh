#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cargo check -p rs-yaml --manifest-path "$ROOT/Cargo.toml"
cargo check -p rs-yaml --no-default-features --manifest-path "$ROOT/Cargo.toml"
cargo check -p rs-yaml --features streaming --manifest-path "$ROOT/Cargo.toml"
cargo check -p rs-yaml --features "streaming,arena" --manifest-path "$ROOT/Cargo.toml"
cargo check -p rs-yaml --all-features --manifest-path "$ROOT/Cargo.toml"
