# Security review

The standalone import was reviewed on 2026-11-24 using the ECC security-review checklist. The review covered untrusted YAML input, scalar emission, both streaming backends, the optional Rust integrations, dependency manifests and lockfiles, repository history, and the GitHub workflow.

| Surface | Finding and change |
| --- | --- |
| Compatibility loading | The original Saphyr loader clones anchors and aliases. All compatibility parsing and duplicate-key validation paths now account for depth, copies, tag and scalar bytes, and document count before forwarding events to the loader. Merge resolution moves entries into the destination map. |
| Serialization | Rewriting float spellings in completed text could change user data. Float spellings are now chosen on scalar nodes. Flow keys use the scalar serializer, block-style representations are handled before reaching Saphyr's emitter, and recursive emission has a depth check. |
| Streaming formatting | Both adapters call fast-yaml-core's guarded event formatters. They check input byte length first. Comments are discarded, and callers can detect them beforehand. |
| Dependencies | Both Cargo graphs have lockfiles and a cargo-deny policy for RustSec advisories, yanked packages, approved licenses, crate sources and version constraints. The original Saphyr value API and the upstream Rust API use different parser versions. |
| Repository and CI | First-party Rust forbids unsafe code. Actions are pinned to commit hashes, checkout discards credentials, and checks use read-only contents access. The release job can create tags and releases. Only its Cargo publication step receives the crates.io environment token. A redacted Gitleaks scan covers the imported Git history before public push. |

## Boundaries

The compatibility loader's logical budget estimates work and allocation. Applications should also limit input bytes at the I/O boundary and select concurrency appropriate to their memory budget. The `fast`, `lint` and `parallel` modules use their own upstream limits. APIs that accept paths use the calling process's filesystem permissions. Only pass paths the application is authorized to read or modify.

The [checks workflow](.github/workflows/checks.yml) runs the feature matrix, dependency checks, and bounded AddressSanitizer fuzzing on parsing, emission, comment ranges and formatter agreement. Each run starts with the small YAML fixtures in `tests/golden`. Results apply to the tested revision and dependency locks.
