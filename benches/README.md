# rs-yaml Benchmarks

Criterion output lives under the workspace `target/criterion/` directory.

Run the full benchmark flow with:

```bash
bash scripts/bench.sh
```

The benchmark surface covers:

- parser hot paths
- emitter hot paths
- `safe_load` regression cases
- formatter comparisons:
  - DOM (no default features)
  - streaming
  - streaming arena

When both `streaming` and `arena` are enabled, the public formatter already
prefers the arena backend automatically.
