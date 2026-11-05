# Working on gamectx

Rules for anyone, human or agent, changing this repository. Status and next ideas live in
[docs/ROADMAP.md](docs/ROADMAP.md); update it in the same commit as the work.

## Non-negotiables

- **Zero third-party dependencies.** Rust standard library only. Minimum Rust is 1.82
  (`rust-version` in Cargo.toml); CI checks it.
- **Conformance cases define the formats.** Any change to a `conformance/**/expected.json`
  is a format change: say so in the CHANGELOG with **[format]**, and review every blessed
  output by hand. Blessing records what the code does, not what it should do.
- **Never change existing output silently.** New features add cases; old cases stay
  byte-identical unless a format change is intended and announced.
- **Claims need numbers.** A feature that is meant to save tokens or improve accuracy ships
  only with a measurement from `eval/` (or a free offline check where that is enough),
  and docs quote the measured numbers exactly.
- **Model output is untrusted.** Anything a model sends (tool calls, formulas, memories)
  is validated with bounded size and work.
- **Original example content only.** Nothing from existing games, and no personal paths,
  names or emails in tracked files.

## How features are built here

1. Prototype in `eval/` (Python) and measure. Token-only checks (`--tokens-only`) cost a
   fraction of a cent; run them first. Accuracy runs use the two smaller default models;
   larger models only when asked.
2. If it wins, implement in Rust, then prove the Rust output is byte-identical to the
   prototype that was measured.
3. Add unit tests, conformance cases (with error cases), and fuzz anything that parses
   model-written text (`tests/robustness.rs`).
4. Update README, docs/FORMAT.md, docs/GUIDE.md, CHANGELOG and ROADMAP with real output.
   Code examples in docs must run as tests (`tests/readme_api.rs`, doctests).

## Checks before every commit

```bash
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

Chain them with `&&` so a failure stops the commit. After a deliberate format change:
`GAMECTX_BLESS=1 cargo test --test conformance`, then review `git diff conformance/`.

## Layout

`src/json.rs` strict JSON · `table.rs` tables and grouping · `view.rs` derive/where/
relevant_to/sort/limit/fields/round · `signatures.rs` and `schema.rs` action schemas ·
`catalog.rs` searchable registries · `memory.rs` recall · `formula.rs` safe formulas ·
`budget.rs` packet assembly · `estimate.rs` token calibration · `context.rs` the easy API ·
`manifest.rs` and `main.rs` the command-line tool · `eval/` model measurements.
