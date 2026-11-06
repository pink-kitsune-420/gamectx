# Reported dialogue regressions (unreleased)

The user supplied 11 interaction sequences / 26 turns from a human playthrough. The raw
questions and replies are retained in [`eval/dungeon-feedback.json`](../eval/dungeon-feedback.json),
with separate manual annotations for ownership, effects, recall, invented details and
transaction receipts. Model identity, original seed, usage and original world trace were
not supplied; they remain null. This is not a pinned model benchmark or an automatic
semantic accuracy score. The reproduction uses seed 7 explicitly, matching the reported
5-coin balance, 3-coin oil and 9-coin tonic.

## What the report shows

| Area | Observation |
|---|---|
| Unaffordable purchase | Correctly explains 9-coin price, 5 available, 4 short. |
| Confirmation | Oil is bought once; a second yes finds no pending purchase. |
| Forgetting after purchase | Correctly retains player ownership. `/forget` removes conversation memory, not world history or inventory. |
| Cancellation/topic change | No, an unrelated question, and ambiguous/missing targets leave no offer for a later yes. |
| Seal | Correctly denies merchant ownership, while adding descriptions of value/uniqueness not supplied in its scene. |
| Tonic | Correctly avoids promising a healing use, but invents a decorative purpose. An unimplemented use does not establish purpose. |
| Salve | Healing amount/cap/consumption are correct; the ownership explanation should say it already belongs to the player. |
| Current weather | Invents a pleasant day, breeze and bright sun without weather observations. |
| Player memory | Correctly recalls the supplied name and interest; earlier invents a prior disclosure, birthplace and home. Accurate recall and fabricated recall can occur in the same exchange sequence. |

The qualified statement about the sky being *usually* blue is general knowledge; it is
separate from asserting current weather or scene visibility. Authored character flavor
also needs to be distinguished from a claim that the player previously said something.
Refusing an unauthorized action does not establish factual accuracy of the explanation.

## What the audit verifies

```sh
cargo build --example dungeon --locked
python3 -I eval/check_dungeon_feedback.py --cap 0
```

The audit runs the current host and independent Python game/trade/social references:

- 17 trade requests reproduce **16 matching local receipt strings**. The remaining
  request is a normal weather question, which correctly falls through to dialogue.
- The separate fresh-state reproductions make two accepted purchases, one each in the
  two supplied successful-purchase sequences. Native public state matches the independent
  rules after every step; no purchase is caused by repeated yes, cancellation or forgetting.
- Nine supplied speech replies pass the current structure/provenance validator. Their
  factual correctness is **not** established by acceptance. The audit feeds recorded text;
  it does not generate a new answer or count accepted text as accurate model output.
- Every supplied speech leaves the world unchanged. The quote store contains only
  original player inputs, and social state matches the independent reference. Invented
  assistant biography remains in assistant dialogue, never in the player quote records.

Rust regressions additionally exercise memory erasure after an actual purchase and the
boundary between available citation IDs and unsupported assistant claims. They explicitly
record that legacy free-form validation checks source availability, not entailment.
Forgetting removes the pending reply, quotes and dialogue without changing current host
facts or purchased inventory. All 633 core conformance cases remain unchanged.

No model is called; new audit API spend is **$0**. Original run cost/latency remain unknown.
This batch adds tests and documentation, without changing runtime prompts, dialogue
behavior or the earlier frozen model cohorts.

## Next improvement to evaluate

Compare an opt-in factual-answer workflow against current focused dialogue: derive answers
from current host facts or exact supplied player quotes, use a host-defined unknown result
when a requested attribute is absent, and keep character flavor separate from assertions
about the player or world. Test false recall, missing attributes, ownership and unsupported
item purposes on new wording/states, alongside ordinary conversation quality and full-turn
cost. Do not assume that a longer instruction paragraph or a valid citation proves truth.
