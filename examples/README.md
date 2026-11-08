# Examples

Every example runs offline with no model, network or game engine. Folders mirror the
library's domain modules; run any example by its stable name:

```sh
cargo run --example NAME
```

| Folder | Examples (`NAME`) | Shows |
|---|---|---|
| [data/](data/) | `query-workflow`, `query-plans` | Model-written SQL over game tables, plans and host contracts |
| [packet/](packet/) | `local-turn`, `optimized-turn`, `task-workflow` | Protected scenes, full-request counting, intent boundaries, size-and-task hints |
| [knowledge/](knowledge/) | `evidence-memory`, `registered-facts`, `info_tools`, `lore` | Evidence and hypotheses, typed facts, informational notes, authored lore |
| [character/](character/) | `character-life`, `character-speech`, `character_tools`, `conversation`, `conversation_tools`, `routines`, `routine-context`, `party`, `living-world` | Character continuity, speech context, scoped tools, conversations, routines, parties and cast packs |
| [runtime/](runtime/) | `adaptive-brain`, `preloaded-brains`, `runtime-loop`, `runtime-upgrades`, `integration-tools` | Reaction brains, frame scheduling, replay and spending |
| [media/](media/) | `visual` | Art contracts, approved fallbacks and repair |
| [kit/](kit/) | `registered-project` | Shared registration, recipes and validated callbacks; also the kit starter files |
| [lantern-vault/](lantern-vault/) | `dungeon` | A playable original text dungeon and optional browser studio ([README](lantern-vault/README.md)) |

[fixtures/](fixtures/) holds the editable original data the examples, tests and evaluations
share: cast packs, character definitions, brain bindings, deployment profiles, the Ember
Keep packet manifest, party scenes and visual art contracts.
