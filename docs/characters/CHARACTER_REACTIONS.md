# Host-approved character reactions

Enable **Settings → Characters → Optional world reactions** in the local studio.
`examples/lantern-vault/web/reactions.json` is the original editable loadout; host overrides
live in the state directory's `reactions.json`. Defaults remain off. Rules select an
arbitrary registered handler using a trigger, attitude/personality gates, a host-confirmed
reason, required resources and priority. The reusable library policy is not limited to money.

`character::roleplay::ReactionPolicy::select` returns `allowed` and host-only `withheld` diagnostics.
Only `allowed` belongs in a compiled prompt or offered action schema. `authorize` rechecks
the current host frame and proposal revision immediately before execution. The host must
execute atomically, record a successful ID in `used`, and advance its scene revision.
A model can propose a selected ID; it cannot supply its own levels, inventory, reasons,
physical evidence or confirmation. This pure helper does no world mutation or model call.
The `studio-reactions` fixture exposes the same bounded contract through all bindings.

Gates can use trust, affinity, resentment, generosity, prudence or courage ranges. Reasons
are request, gratitude, emergency or threat. A threat-specific rule requires a current
host revision and a `perceived` event with host-confirmed capability, imminence and shared
scene. Reports, claims, stale evidence and mere threatening chat never satisfy that rule.
The helper does not decide whether physical danger is real: the game's authoritative
physics/action adapter must supply that evidence. The Lantern Vault demo supplies no
physical-threat proof from dialogue and therefore grants no coercive capabilities.

# Playable adapters

The studio registers gift, loan and support handlers. NPC-owned coins/items can transfer
only after all gates pass. Support demonstrates a non-monetary response. Other engines
can register handlers for assistance, leaving, calling aid, protecting someone or other
world reactions; the library does not invent implementations for those actions.

Merchant defaults require trust 17 for a small loan, plus prudent/generous dispositions.
Gifts also require gratitude from a host-confirmed fulfilled commitment witnessed by that
NPC. Keeper defaults use trust 15 and its more generous disposition. These are editable
example choices, not universal social thresholds. Current Life attitudes are scoped to
the active player. A saved subjective opinion cannot raise numeric trust or grant a favor.

Try buying chalk after accepting a chalk commitment. Eligible offers then appear near the
conversation input; withheld favors have no buttons or prompt entries. Ask for an exact
listed favor, for example “Can you lend me 2 coins?”, “Could you give me a coin?” or
“Could you give me the keeper seal?” Ask “What do I owe?” or use **Repay a loan** to repay.
The text recognizer is deliberately small: unsupported quantities/multiple resources are
refused instead of silently shrinking a request. Future repayment language does not execute
a repayment. Buttons provide exact, unambiguous requests.

Transactions recheck actual scene, resources and prior successful IDs, conserve coins,
and transfer real item custody. They use the host's single lane with **zero model calls**.
Speech describing success is constructed only after the host transaction succeeds.
Loans track lender, borrowing player and resource; another player's repayment cannot
close that debt. The example retains its shared party purse/inventory, so party members
can still use shared resources. Consuming a borrowed item leaves its debt open until the
resource can actually be returned. Each rule is usable once per NPC/listener per game,
preventing repeated gifts or borrowing loops. The bounded 128-record journal reserves
room for repayment of open loans.

Branch checkpoints replay confirmed transfers in order between normal game actions,
including purchases credited to NPC wallets. Old checkpoints without this optional
extension still load. New game clears transfers/debts and restores initial resources.
Future reactions can be disabled without undoing completed transfers. Changing rules or
initial wallets after a transfer requires New game; old branches require matching rule
configuration. Save data is trusted host-authored data: replay checks resource custody,
used IDs, rules and recorded evidence, but does not cryptographically prove historical
attitudes or re-run past physics. The host's scene revision fences UI proposals and
invalidates pending model upgrades; the original player-action revision format is unchanged.

LAN guests still cannot configure providers, run models or call world-control endpoints.
When this optional policy is enabled, a guest can request a conversational favor that the
host evaluates and executes under the same NPC gates. Debts belong to `player_2`; all
processing remains on the host. Both browsers hide a pending reply while it is generated.

# Evidence

Twenty-two independent Python/Rust cases cover eligible/withheld choices, personality,
resources, reasons, reuse, social limits, handler registration, stale/forbidden proposals,
physical-threat provenance and generic non-resource help. Two thousand mutations never
execute a world action. Unit tests verify that withheld capability IDs are absent from
actual compiled requests, failed resource transactions leave state intact, item custody
checks actual scene/owner, and requests preserve quantities/future repayment intent.

`eval/studio/reaction-host.json` records twelve deterministic workflows and 80 HTTP checks:
coin and item loans/repayment, actual item gifting, player-scoped debt, repeated/stale
rejection, personality gates, branch replay and replay with future reactions disabled.
The party plus NPC wallets retain exactly 20 coins throughout that cohort. No models
were called. `eval/studio/reaction-browser.json` records six Firefox workflows with zero page
errors: withheld buttons absent, editable settings, real loan/repayment/gift controls,
resource filtering, gift reuse prevention and minimized diagnostics. These are bounded
host/browser checks, not live social-intelligence or physical-media measurements. Prior
evaluation and conformance outputs remain frozen.
