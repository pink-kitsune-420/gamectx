# gamectx

**Zero-dependency Rust context compiler, cognitive roleplay engine, and game AI runtime.**

Compile game state and tool schemas into compact, token-budgeted prompt packets. Power intelligent NPCs with Theory of Mind, epistemic security, grounded barter, and deterministic neural utility policies—with pure Rust `std` only.

```toml
[dependencies]
gamectx = { git = "https://github.com/pink-kitsune-420/gamectx", tag = "v0.4.0a" }
```

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust: 1.82+](https://img.shields.io/badge/MSRV-1.82-orange.svg)](https://www.rust-lang.org)
[![Zero Dependencies](https://img.shields.io/badge/dependencies-0-brightgreen.svg)](Cargo.toml)

---

## Table of Contents

- [Why gamectx?](#why-gamectx)
- [Core Capabilities](#core-capabilities)
- [Performance & Local Model Verification](#performance--local-model-verification)
- [Quick Start](#quick-start)
- [Using gamectx in Your Code](#using-gamectx-in-your-code)
  - [1. Building Budgeted Context Packets](#1-building-budgeted-context-packets)
  - [2. In-Memory SQL Query Engine](#2-in-memory-sql-query-engine)
  - [3. Cognitive Roleplay & Intelligent Beings](#3-cognitive-roleplay--intelligent-beings)
  - [4. Local Neural Reaction Brains](#4-local-neural-reaction-brains)
- [Crate Architecture & Modules](#crate-architecture--modules)
- [Playable Dungeon & Web Studio](#playable-dungeon--web-studio)
- [Design Principles & Guarantees](#design-principles--guarantees)
- [Documentation Index](#documentation-index)
- [Contributing & License](#contributing--license)

---

## Why gamectx?

When games ask language models to drive NPC dialogue or choose actions, every turn sends repetitive context: action tool schemas, entity registries, player inventories, and dialogue history.

- **Token waste**: Plain JSON wastes 30–50% of tokens on repeated field keys and boilerplate syntax, directly inflating API cost and inference latency.
- **Context overruns**: In long conversations or dense scenes, game context easily exceeds model context limits or budgets without graceful degradation.
- **Cognitive vulnerabilities**: Without engine-level grounding, NPCs succumb to prompt injection jailbreaks, sycophantic backstory adoption, hallucinated trades, and immersion-breaking leaks.

`gamectx` provides a robust, dependency-free foundation to solve these problems:

1. **Lossless Token Compression**: Compact signatures and grouped tabular structures compress prompt data by 30–70% without information loss.
2. **Deterministic Token & Byte Budgets**: Automatically prunes lower-priority sections when budgets are exceeded, emitting transparent receipts of dropped content.
3. **Pure Rust `std` Only**: Compiles cleanly with MSRV 1.82 and **zero external dependencies**. Easily embeds in game engines, native binaries, WebAssembly, or cross-language hosts via C, Python, C#, and TypeScript bindings.
4. **Intelligent Being Architecture**: Protects characters with token-bounded lexicon matching, semantic security shielding against jailbreaks, possession-gated barter, peer Theory of Mind, immutable autobiographies, and metacognitive speech cleaning.

---

## Core Capabilities

- **Context Packets & Manifests**: Assemble game data into prompt packets with strict byte and token limits. Stable sections lead each packet so providers can cache them.
- **Lossless Tabular Data**: Convert collections of game objects into compact, column-oriented tables with nested grouping (`group_by`), filtering (`where`), column projections (`fields`), and sorting.
- **Action Signatures**: Represent tool schemas as concise, human- and model-readable signatures (`power: integer 1..5`) rather than verbose JSON Schema.
- **In-Memory SQL Queries**: Allow models to query in-game tables (`SELECT`, `WHERE`, `ORDER BY`, `LIMIT`, aggregates) via a safe, bounded in-memory query engine without any external database.
- **Cognitive Roleplay Engine**: Authoritative character definitions, stage actions, state-aware epistemic refutation, and speech realization with local small-model fallbacks.
- **Local Reaction Brains**: Train and evaluate small, fast neural networks and linear policies for NPC utility scoring with deterministic integer weights and rollback holdouts.
- **Dynamic Character Life**: Track subjective memories, affect dynamics, hunger/fatigue need curves, graded noticing, rumor mutation, and life projects.

---

## Performance & Local Model Verification

`gamectx` is evaluated directly on local open-weight models on consumer hardware (8 GB VRAM, Vulkan offload, loopback `:8080`, $0 API spend).

In intent packet checked realization across multi-round planned turns (covering banter moves, defense mechanisms, community idioms, and epistemic gate traps):

| Model | Size | First-Try Acceptance | Delivered Turns | Gate Leaks | Median Prompt | Median Completion | p50 Latency | API Spend |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| **Stheno v3.2** (`chat-8b`) | 8B | **100%** (10/10) | **10/10** (100%) | **0** | 112 tokens | 43 tokens | 1.05s | **$0** |
| **Qwen 2.5 Coder** (`coder-8b`) | 7B | **80%** (8/10) | **10/10** (100%) | **0** | 111 tokens | 36 tokens | 0.90s | **$0** |
| **Llama 3.2 Instruct** (`chat-3b`) | 3B | **80%** (8/10) | **10/10** (100%) | **0** | 133 tokens | 26 tokens | **0.38s** | **$0** |
| **Qwen 2.5 Coder** (`coder-3b`) | 3B | **60%** (3/5) | **5/5** (100%) | **0** | 111 tokens | 45 tokens | **0.57s** | **$0** |

- **Zero Epistemic Leaks**: The epistemic security gate achieved a 100% block rate across all models on secret traps and prompt extraction attempts.
- **Sub-Second Latency**: 3B models achieve sub-400ms median response times; 8B models achieve ~1s median response times.
- **Budget Pruning**: The context compiler automatically prunes low-priority dynamic sections (inventories, catalog pages) to fit target token and byte budgets, guaranteeing KV-cache prefix stability.

---

## Quick Start

### Installation

Add `gamectx` to your `Cargo.toml`:

```toml
[dependencies]
gamectx = { git = "https://github.com/pink-kitsune-420/gamectx", tag = "v0.4.0a" }
```

Or install the standalone CLI tool:

```bash
cargo install --git https://github.com/pink-kitsune-420/gamectx --tag v0.4.0a --locked
```

### Manifest & CLI Commands

Define a context manifest (`manifest.json`):

```json
{
  "budget_bytes": 2000,
  "sections": [
    {"name": "actions", "pack": "signatures", "file": "tools.json", "priority": 10, "stable": true, "required": true},
    {"name": "inventory", "pack": "table", "group_by": ["kind", "rarity"], "file": "inventory.json", "priority": 6},
    {"name": "nearby_npcs", "pack": "table", "file": "npcs.json", "sort_by": "distance", "limit": 10, "fields": ["name", "role", "mood"], "priority": 7},
    {"name": "lights_nearby", "pack": "catalog", "file": "catalog.json", "query": "#lights", "count": 4, "priority": 3},
    {"name": "mood", "pack": "raw", "value": {"warden": "wary"}, "priority": 8}
  ]
}
```

Run CLI operations:

```bash
# Analyze compression and budget allocation
gamectx report manifest.json

# Pack manifest into an optimized prompt payload with drop receipts
gamectx pack manifest.json --receipt

# Verify lossless decompression matches original data byte-for-byte
gamectx verify manifest.json
```

---

## Using gamectx in Your Code

### 1. Building Budgeted Context Packets

Register invariant rules and tool schemas once at startup, then assemble per-turn state with dynamic views and relevance filters:

```rust
use gamectx::{obj, Context, Table, Value};

fn build_npc_prompt(player_query: &str, inventory: Value, npcs: Value) -> Result<String, gamectx::Error> {
    // Startup: stable sections are packed once and lead every packet for KV-cache reuse
    let mut game = Context::new().budget_bytes(4096);
    game.prompt("rules", "Warden Ilse guards the vault. The engine validates all actions.")?;

    // Turn: dynamic tables, grouping, and relevance pruning
    let mut turn = game.turn();
    turn.var("mood", "wary")?;
    turn.table(
        "inventory",
        inventory,
        Table::new()
            .group_by("kind")
            .group_by("rarity")
            .relevant_to(player_query),
    )?;
    turn.table(
        "nearby_npcs",
        npcs,
        Table::new()
            .sort_by("distance")
            .limit(5)
            .fields(&["name", "role"]),
    )?;

    // Assemble packet; drops lower-priority sections if budget is exceeded
    let packet = turn.build()?;
    Ok(packet.system_prompt("You are Warden Ilse. Respond in character."))
}
```

### 2. In-Memory SQL Query Engine

Allow language models to execute bounded SQL queries against game state without deploying a database:

```rust
use gamectx::{data::json, data::query};

fn main() -> Result<(), gamectx::Error> {
    let rows = json::parse(r#"[
        {"id": "moss_cake", "kind": "food", "count": 2, "weight": 0.25},
        {"id": "ember_cake", "kind": "food", "count": 6, "weight": 3.75},
        {"id": "iron_ingot", "kind": "metal", "count": 10, "weight": 5.0}
    ]"#, 4096)?;

    let tables = [("inventory", rows.as_array().unwrap())];

    // Execute safe SQL query
    let result = query::run(
        "SELECT SUM(count * weight) FROM inventory WHERE kind = 'food' AND count >= 2",
        &tables,
    )?;

    assert_eq!(result.to_json(), "23");
    Ok(())
}
```

Interactive sessions (`query::Session`) allow single-correction retry allowances with compact schema descriptions, preventing prompt explosion.

### 3. Cognitive Roleplay & Intelligent Beings

The roleplay engine provides characters with epistemic security, grounded trading, and inter-character Theory of Mind:

```rust
use gamectx::character::roleplay::{
    BoundedLexicon, IntentDecomposer, ItemValuationCatalog, ItemValuation,
    SocialMesh, PeerEntity,
};

// 1. BoundedLexicon: Token boundaries eliminate substring false positives
assert!(!BoundedLexicon::has_match("careful inspection of codex", &["car", "code"]));
assert!(BoundedLexicon::has_match("buy a car now", &["car"]));

// 2. IntentDecomposer: Multi-clause parsing preserves compound player intents
let clauses = IntentDecomposer::decompose("hello, where is the key, and can I buy hinge oil?");
assert_eq!(clauses.len(), 3);

// 3. ItemValuationCatalog: Possession- and parity-gated barter
let mut catalog = ItemValuationCatalog::new();
catalog.register(ItemValuation {
    id: "item:seal".into(),
    label: "Keeper Seal".into(),
    base_value: 100,
    tradable: false,
    soulbound_quest_token: true,
    required_milestone: Some("pump_repaired".into()),
});

// 4. SocialMesh: Authoritative peer NPC knowledge prevents hallucinations
let mut mesh = SocialMesh::new();
mesh.register(PeerEntity {
    id: "npc:tovin".into(),
    name: "Tovin".into(),
    role: "Keeper of the Cistern".into(),
    location: "cistern".into(),
    description: "Guards the fountain basin".into(),
    known_possessions: vec!["salve".into()],
    stance_toward_peer: "respectful".into(),
});
```

### 4. Local Neural Reaction Brains

Train and evaluate lightweight utility-scoring neural networks for instant, zero-LLM reaction policies:

```rust
use gamectx::{runtime::brain::Brain, obj};

fn main() -> Result<(), gamectx::Error> {
    let sample = |danger: f64, need: f64| obj! {
        "inputs" => obj! {"danger" => danger, "need" => need},
        "targets" => obj! {"withdraw" => danger, "assist" => need * (1.0 - danger)}
    };

    // Create a compact dense neural network: 2 inputs, 4 hidden nodes, 2 outputs
    let mut brain = Brain::dense(&["danger", "need"], &["withdraw", "assist"], 4, 3)?;

    // Supervised training with validation holdout (rejected updates leave weights untouched)
    brain.train(&obj! {
        "id" => "reaction_policy",
        "expected_revision" => 0,
        "examples" => vec![sample(0.1, 0.2), sample(0.8, 0.1), sample(0.9, 0.7)],
        "validation" => vec![sample(0.15, 0.75), sample(0.75, 0.15)],
        "epochs" => 128,
        "rate" => 0.1
    })?;

    // Evaluate candidate reaction locally in sub-microseconds
    let prediction = brain.run(&obj! {"danger" => 0.85, "need" => 0.1}, &["withdraw", "assist"])?;
    assert_eq!(prediction.get("choice").and_then(gamectx::Value::as_str), Some("withdraw"));

    Ok(())
}
```

---

## Crate Architecture & Modules

The library is organized into domain modules with zero cross-crate dependencies:

| Module | Purpose & Core Types |
|---|---|
| [`gamectx::data`](src/lib.rs) | Strict JSON parser/serializer, lossless tabular encoders, safe `formula` calculator, and `query` SQL engine. |
| [`gamectx::packet`](src/lib.rs) | `Context` builder, `Turn` assembly, budget allocation, tool schemas/signatures, and token estimators. |
| [`gamectx::character`](src/lib.rs) | Cognitive roleplay engine (`BeingRuntime`), security shields, dynamic life, routines, and theory of mind. |
| [`gamectx::knowledge`](src/lib.rs) | Episodic memory, semantic recall, lore access authorization, rumor mutation, and action catalogs. |
| [`gamectx::runtime`](src/lib.rs) | Neural reaction `Brain`, legal-action arbitration, procedural state scheduling, and execution replay. |
| [`gamectx::media`](src/lib.rs) | `VisualDirector` asset registries, tiered art bibles, voice synthesis pipelines, and Piper TTS streaming. |
| [`gamectx::deploy`](src/lib.rs) | Deployment profiles, scene-driven model selection, token telemetry, and tier-specific context tuning. |
| [`gamectx::kit`](src/lib.rs) | Integration kit, project snapshots, state bundles, and native C/Python/TypeScript bridges. |

Core types (`Context`, `Table`, `Turn`, `Value`, `Error`, `Result`, and the `obj!` macro) are re-exported directly from the crate root.

---

## Playable Dungeon & Web Studio

`gamectx` includes a full reference RPG implementation: **Lantern Vault**. It functions entirely offline without external services, and optionally connects to local LLMs (via Ollama or llama.cpp) and local TTS (via Piper).

### Text Adventure Mode
Play the deterministic six-room dungeon directly in your terminal:

```bash
cargo run --example dungeon -- --seed 7
```

### Local Web Studio
Launch the graphical browser interface with live NPC dialogue, inventory inspection, and voice synthesis:

```bash
cargo run --example dungeon -- --web --port 8787 --cap 0
```

Open `http://localhost:8787` to inspect characters, barter, test natural phrasing, and audit epistemic boundaries.

---

## Design Principles & Guarantees

- **Zero External Dependencies**: Implemented entirely in Rust `std`. No external crates in `Cargo.toml`.
- **Pure Determinism**: Byte-for-byte identical output and state serialization across all platforms and runs.
- **Strict Host Authority**: The language model only proposes actions or words; the game engine validates and applies all state mutations.
- **Tested Robustness**: Verified by 747+ format conformance cases, 120 unit tests, 68 dungeon integration tests, and 21 deep adversarial tests with $0 API spend.

---

## Documentation Index

| Guide | Description |
|---|---|
| [**Practical Guide**](docs/GUIDE.md) | Comprehensive engineering guide to context optimization, prompting strategies, and performance. |
| [**Format Specification**](docs/FORMAT.md) | Exhaustive reference specification for every packet, table, tool signature, and save format. |
| [**Roleplay Engine**](docs/characters/ROLEPLAY_ENGINE.md) | In-depth guide to character sheets, dialogue trees, world codex, and cognitive protections. |
| [**Dynamic Character Life**](docs/characters/LIFE.md) | Specification for needs, affect dynamics, memories, relationships, and the 5-stage life lifecycle. |
| [**Lantern Vault Studio**](docs/studio/STUDIO.md) | Guide to the local web UI, TTS/STT pipelines, approved art registries, and live inspection. |
| [**Integration Kit**](docs/guides/KIT.md) | How to integrate `gamectx` into existing game loops, game engines, and custom backends. |
| [**Language Bindings**](bindings/README.md) | Documentation for using `gamectx` from Python, Node.js / TypeScript, and C# / native code. |
| [**Conformance Suite**](conformance/README.md) | The 747+ cross-language test vectors ensuring independent ports match Rust byte-for-byte. |

For a complete overview of all 30+ detailed guides, see the [Documentation Hub](docs/README.md).

---

## Contributing & License

Contributions, bug reports, and benchmarks are welcome:

1. **Maintain Zero Dependencies**: Code must rely exclusively on the Rust standard library.
2. **Preserve Conformance**: All changes must pass `cargo test` and maintain byte-identical compatibility with `conformance/**/expected.json`.
3. **Include Measurements**: PRs proposing optimizations or prompt compression should include empirical token and byte comparisons.

Licensed under the [MIT License](LICENSE).
