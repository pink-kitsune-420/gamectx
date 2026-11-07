# Optional dynamic characters

`life` keeps approved definitions, subjective attitudes and attributed experiences in
separate layers. Nothing is enabled by default. `Config::resolve` merges shared defaults
and per-character overrides; `enabled`, `balanced`, `deception`, `mistaken` and requested
`tools` are independent host choices. `CharacterTools` owns the optional layer alongside
its existing interaction notebook and routine runtime. Existing relationships, promises,
source answers and tool formats are unchanged.

Run `cargo run --example character-life`. Edit `examples/characters/setting.json` and
`merchant-life.json`. `Definition::draft` validates IDs, setting/calendar, exact shared
history references and bounded fields. The host reviews and explicitly installs a draft
with `Life::accept`; no model tool can approve it. Review prose implications against the
setting too: a structural validator cannot detect every textual contradiction. Authored
history must already exist in the host setting, so a draft cannot create future choices.
Nonhuman identities and host calendar IDs work without a model.

Facets carry identity, values, voice, boundaries, practical knowledge, social conventions,
groups, reputation, etiquette, lore/private information, goals, commitments, priorities
and physical/emotional meanings. Attitudes distinguish preferences, opinions, relationship
dimensions, beliefs/reports/hypotheses/misconceptions/unknowns and temporary states. Tags
select applicable traits; audiences gate them. Numeric levels are design controls, never
psychological measurements or probabilities. Existing relationship notes remain the place
for reported quotations; these attitudes are author-selected appraisal parameters.

Host `experience` preserves kind, source, time, scope, causes and retention intent.
`appraise` requires a matching direct/outcome experience, current state revision and an
authored rule; a report or intended action cannot justify a rule application. Each step
is at most 20, a reversal passes through zero, and cooldown/replay guards prevent a single
event from reversing a lasting view. Changes retain reasons and supporting/contradicting
event tokens. Different definitions can appraise the same event differently.

`life_change` only proposes a rule/event/reason. `life_recall` returns selected traits
and attitudes, never engine truth. Register these individually in configuration, expose
through `CharacterTools::enable` or budgeted assembly, and check live `action/target`
permissions. Host mutations revoke exposure. Context tags can select opinions for dialogue,
attention or activity ranking. More context does not register additional tools.

Balanced policy is opt-in context guidance: expression, tastes, jokes, hypothetical and
permitted flavor do not require citations for every sentence. Facts, reports, inference,
beliefs and proposals stay distinct. Useful partial answers and natural uncertainty are
encouraged. This policy does not semantically validate generated prose. High-consequence
answers still use validated host data. Prior speech failures remain unchanged.

## Stage completion record

1. Definitions/appraisals: implemented bounded Rust and independent Python parity, original
   editable files, disabled/private/nonhuman/replay/stale tests and additive fixtures.
   Conversational usefulness is unmeasured; no model call or accuracy claim.
2. Scene/conversation continuity: pending.
3. Durable episodic memory: pending.
4. Initiative: pending.
5. Controls, bindings and integrated demonstration: pending.
