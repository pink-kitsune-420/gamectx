# Working on gamectx

Rules for anyone, human or agent, changing this repository. Status and next ideas live in
[docs/ROADMAP.md](docs/ROADMAP.md); update it in the same commit as the work. See also
[AGENT.md](AGENT.md).

## Mission: Accessible, Efficient AI Gaming

`gamectx` is an open, modular game context compiler designed so developers can build rich,
stateful AI characters and game systems that run anywhere—starting on modest local hardware
(such as a four-core CPU with 8 GB RAM running a 1B–3B local model at $0 API cost) and
scaling progressively up to larger local or frontier models without sacrificing host
authority over game rules and state.

## Non-negotiables

- **Zero third-party dependencies.** Rust standard library only (`std`). Minimum Rust is
  1.82 (`rust-version` in `Cargo.toml`); CI checks it. This keeps the core free of
  supply-chain, license-compatibility and transitive-dependency risks.
- **Strict clean-room originality (from-scratch only).** Write every algorithm, schema,
  prompt, character definition, test fixture and example world from scratch for this
  project. Never copy, adapt, translate or port implementation code, prompts, character
  cards, biographies, lore or assets from other repositories, commercial SDKs or existing
  games. Do not inspect third-party implementation files when designing features; rely only
  on public specifications, research and model cards.
- **Original example content and privacy hygiene.** Use only original settings and
  characters (such as *Ember Keep* and *Lantern Vault*). Never commit API keys, tokens,
  credentials, personal filesystem paths, usernames, hostnames or emails in tracked files.
- **Open sharing under MIT.** All contributions to this repository are shared openly under
  the root [LICENSE](LICENSE) (MIT License, provided "AS IS" without warranty).
- **Conformance cases define the formats.** Any change to a `conformance/**/expected.json`
  is a format change: say so in the CHANGELOG with **[format]**, and review every blessed
  output by hand. Blessing records what the code does, not what it should do.
- **Never change existing output silently.** New features add cases; old cases stay
  byte-identical unless a format change is intended and announced.
- **Claims need numbers.** A feature that is meant to save tokens, reduce latency/memory or
  improve accuracy ships only with a measurement from `eval/` (or a free offline check
  where that is enough), and docs quote the measured numbers and their limits exactly.
- **Model output is untrusted; the host owns truth.** Anything a model sends (tool calls,
  queries, formulas, memories, speech, appraisal or initiative proposals) is validated with
  bounded size, bounded work and current revision/permission checks. Models propose; the
  host validates and executes.

## How features are built here

1. Prototype in `eval/` (Python) with predeclared expectations before observing model
   outputs. Run offline and token-only checks (`--tokens-only`, `--cap 0` for local) first.
   Hosted accuracy runs use the two smaller default models within explicit `--cap` limits;
   larger or additional models require explicit authorization.
2. If it wins, implement in Rust, then prove the Rust output is byte-identical to the
   independent prototype that was measured.
3. Add unit tests, conformance cases (including malformed/error cases), and fuzz anything
   that parses model-written text (`tests/robustness.rs`).
4. Update `README.md`, `docs/FORMAT.md`, `docs/GUIDE.md`, `CHANGELOG.md` and
   `docs/ROADMAP.md` with real output. Code examples in docs must run as tests
   (`tests/readme_api.rs`, doctests).

## Checks before every commit

```bash
cargo fmt --check &&
cargo clippy --all-targets --locked -- -D warnings &&
cargo test --locked &&
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
```

Chain them with `&&` so a failure stops the commit. After a deliberate format change:
`GAMECTX_BLESS=1 cargo test --test conformance`, then review `git diff conformance/`.
Run applicable binding checks, Rust 1.82 checks and a credential/personal-path sweep before
committing.

## Layout

`src/` is grouped by domain (0.4.0a): `data/` strict JSON, tables, views, formulas, catalogs
and the bounded SQL query engine · `packet/` the `Context` builder, budgeted assembly,
manifests, tool schemas/signatures, full-request accounting, token estimates and sync ·
`knowledge/` memory, recall, typed facts, lore, attributed notes, inventory, observations,
evidence selection, location, world and time windows, the world timeline, places, things and recipes · `character/` scoped character tools,
life (appraisals, continuity, checkpoints, initiative, speech), dialogue, routines, parties,
archetypes, cast packs, the living world and `roleplay/` policies (greetings, expression,
wording, viewpoints, reactions, deliberation, actions, commands, message chunks) ·
`runtime/` reaction brains, decisions, scheduling, procedures, outcomes, replay, request
tickets and spending · `deploy/` deployment profiles, model policy, tuning and turn
telemetry · `media/` visual specs/registry, voice delivery and provider routing · `kit/`
integration kit, the `local` reference adapter and persistent bridges.
`bindings/` Python, TypeScript/Node, C# and C ABI adapters · `examples/` runnable demos by
domain, shared `fixtures/` and the `lantern-vault/` game · `docs/` guides, characters,
studio, evidence and research · `eval/` offline references and model evaluations by topic
(Python 3.12+, Node 22; `eval/common/provenance.py` maps pre-0.4 evidence paths).
