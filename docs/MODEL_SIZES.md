# Model-size workload guide (unreleased)

Use this as a guide to **what work to give the model**, with editable starting profiles.
Parameter count alone does not establish capability. Fine-tuning, quantization, template,
task structure and hardware matter. The ranges below are proposed experiments, not
validated capability ceilings, minimum prompt lengths or architectural context limits.
Only the specific Qwen 3B deployment has a local measurement in this change.

| Approximate class | Start with this task | Hydrated-content range to try | Initial choices / output cap |
|---|---|---:|---:|
| 1B | One intent label or one supplied legal action/target pair | 400–700 tokens | up to 4 / 64 tokens |
| 1.5B | One intent/entity extraction or a few prefiltered choices | 500–900 | up to 6 / 96 |
| 3B | One scene, a bounded decision, or short grounded dialogue | 800–1,400 | up to 8 / 256 |
| 8B | Test richer single-scene reasoning with bounded tools | 1,000–2,200 | up to 16 / 256 |
| 30B | Test short explicit multi-step plans with validation at each step | 1,200–3,200 | up to 32 / 384 |
| 70B | Test broader planning or nuanced dialogue with explicit state/tool bounds | 1,500–4,800 | up to 32 / 512 |

Do not pad a prompt to reach the low end. Start smaller whenever the task fits. The
locally tested 3B decision prompts were substantially smaller than the illustrative
3B range. Increasing prompt size requires its own correctness and latency check.

For **1B–1.5B**, do most work before the call: identify the scene, filter ownership and
eligibility, compute totals, and present a small set of readable choices. Ask one question
and require a tiny structured response. If the correct candidate cannot be retained in
that set, use more exact retrieval, split the task or ask for clarification. Never remove
a required fact or the correct choice merely to meet a size hint.

For **3B**, keep one current scene, the relevant facts, the immediate request and a few
whole dialogue exchanges. A short response grounded in an approved outcome is a useful
experiment. Campaign recall, simultaneous character bookkeeping and rule resolution
still belong in external state and tools; schema validity does not prove comprehension.

For **8B and above**, evaluate additional bindings or planning depth gradually. More
parameters do not make arithmetic, permissions, inventory updates or stale-state checks
model responsibilities. Tool results and current authoritative facts remain explicit.
A correct short plan is more useful than a long explanation that cannot be executed.

Keep the same acceptance checks for every tier: required facts retained, correct entity
binding and negation, legal proposals, host validation, bounded output, and acceptable
complete-turn latency/memory. A syntactically valid but wrong decision still fails.

## Profiles and overrides

[`examples/deployments`](../examples/deployments) contains editable templates for 1B,
1.5B, 3B, 8B, 30B and 70B. `deployment::ModelSize::starting_profile` generates the same
format; bridge/CLI workflow kind `preset` accepts aliases such as `1.5` or `1.5b`.
Other sizes use an ordinary explicit `deployment::Profile`, without changing the library.

Each result contains `guideline` and the unchanged v1 `profile` definition. Every generic
guideline is explicitly `measured:false`. Profile identity values in examples are fixtures:
replace them with the actual model digest, quantization, template hash, tokenizer identity
and runner/build identity before use. Exact identity checks remain mandatory when adopting
an evaluated profile. Size-class labels are approximate organizational metadata.

The templates start at 2K configured context for 1B/1.5B/3B, 4K for 8B and 8K for 30B/70B.
These are unmeasured editable configurations, not model limits or recommendations that
larger models need larger prompts. Threads start at four, sampling at zero, standard
supplemental guidance and schema transport. Check transport support on the exact build.
Change context/output/threads/guidance explicitly and test the actual workload.

The table's content targets exclude hidden tools/template framing. The full-request
counter must include that framing and reserve output plus safety. A 2K starting context
can be expanded when measurements and required facts justify it. Do not assume a model
fits RAM from its parameter count: weights, KV, buffers, game/OS usage and concurrency
all matter. Larger templates do not imply that 30B/70B fit an 8 GB device.

Start with one resident model and one active request on a constrained host. Adding a
1.5B interpreter ahead of a writer adds another call and potentially another loaded model;
measure complete-turn benefit before adopting that architecture. The local brain,
deterministic command handling and host tools can handle routine reactions without a
language-model call.

See [LOCAL](LOCAL.md) for the executable APIs and [the local report](LOCAL_MODEL_2026_11_06.md)
for what was actually measured. No larger or 1.5B model is downloaded or tested by these
configuration templates.
