#!/usr/bin/env python3
"""Motive arbiter reference implementation (Gap 3).

Deterministic, explainable ranking across:
- emergencies (physical danger, critical health, exhaustion collapse)
- needs (homeostatic physical drives with threshold transitions)
- promises and deadlines (commitments with remaining time / overdue status)
- routine slot (scheduled daily work activities)
- sheet goals (authored character goals)
- tasks (active planning tasks)
- reaction brains (situational impulses as inputs, never sole arbiter)

Returns a ranked list on a common bounded 0..=100 scale, naming the winning motive
and explaining why it won.
"""

from typing import Any, Dict, List, Optional, Tuple

MAX_MOTIVES = 64
MAX_OPS = 64

KIND_PRECEDENCE = {
    "emergency": 7,
    "need": 6,
    "promise": 5,
    "routine": 4,
    "task": 3,
    "goal": 2,
    "reaction": 1,
}


def clamp(val: int, lo: int, hi: int) -> int:
    return max(lo, min(hi, val))


class MotiveReference:
    def __init__(self):
        pass

    def evaluate_motive(self, m: Dict[str, Any]) -> Dict[str, Any]:
        mid = m.get("id")
        if not mid or not isinstance(mid, str):
            raise ValueError("invalid motive id")

        kind = m.get("kind", "goal")
        if kind not in KIND_PRECEDENCE:
            raise ValueError(f"unknown motive kind: {kind}")

        label = m.get("label", mid)
        action = m.get("action")

        # If an explicit score is provided, use it clamped to 0..100
        if "score" in m:
            score = clamp(int(m["score"]), 0, 100)
            reason = m.get("reason", f"explicit score {score}")
            return {
                "id": mid,
                "kind": kind,
                "label": label,
                "score": score,
                "action": action,
                "reason": reason,
            }

        # Otherwise compute from category attributes
        score = 0
        reason = ""

        if kind == "emergency":
            sev = int(m.get("severity", 50))
            score = clamp(80 + sev // 5, 80, 100)
            reason = m.get("reason", f"emergency severity {sev} (score {score})")

        elif kind == "need":
            level = int(m.get("level", 50))
            thresh = int(m.get("threshold", 70))
            if level >= 100:
                score = 92
                reason = f"need {mid} critical at 100 (causing damage)"
            elif level >= thresh:
                span = max(1, 100 - thresh)
                urgency = 50 + ((level - thresh) * 40) // span
                score = clamp(urgency, 50, 90)
                reason = f"need {mid} level {level} exceeds threshold {thresh} (urgent score {score})"
            else:
                span = max(1, thresh)
                score = clamp((level * 45) // span, 0, 45)
                reason = f"need {mid} level {level} below threshold {thresh} (score {score})"

        elif kind == "promise":
            prio = int(m.get("priority", 70))
            remaining = int(m.get("remaining_ticks", 10))
            overdue = m.get("overdue", False) or remaining <= 0

            if overdue:
                score = clamp(80 + prio // 5, 80, 95)
                reason = f"promise {mid} is overdue (score {score})"
            elif remaining <= 5:
                score = clamp(70 + (5 - remaining) * 2 + prio // 10, 70, 85)
                reason = f"promise {mid} impending deadline in {remaining} ticks (score {score})"
            else:
                score = clamp(prio - remaining // 2, 20, 70)
                reason = f"promise {mid} deadline in {remaining} ticks (score {score})"

        elif kind == "routine":
            prio = int(m.get("priority", 50))
            active = m.get("window_active", True)
            time_to_start = int(m.get("time_to_start", 0))

            if active:
                score = clamp(prio, 30, 75)
                reason = f"routine {mid} in active window with priority {prio} (score {score})"
            else:
                score = clamp(prio - time_to_start, 10, 50)
                reason = f"routine {mid} upcoming in {time_to_start} ticks (score {score})"

        elif kind == "goal":
            prio = int(m.get("priority", 50))
            active = m.get("active", True)
            if active:
                score = clamp(prio, 20, 70)
                reason = f"goal {mid} active with priority {prio} (score {score})"
            else:
                score = 0
                reason = f"goal {mid} conditions not met"

        elif kind == "task":
            prio = int(m.get("priority", 60))
            score = clamp(prio, 30, 80)
            reason = f"task {mid} priority {prio} (score {score})"

        elif kind == "reaction":
            raw_score = int(m.get("raw_score", m.get("priority", 50)))
            score = clamp(raw_score, 0, 65)
            reason = f"reaction impulse score {score}"

        return {
            "id": mid,
            "kind": kind,
            "label": label,
            "score": score,
            "action": action,
            "reason": reason,
        }

    def op_arbitrate(self, args: Dict[str, Any]) -> Dict[str, Any]:
        candidates: List[Dict[str, Any]] = []

        # 1. Synthesize motives from body state if provided
        body = args.get("body")
        if isinstance(body, dict):
            # Health critical emergency
            health = int(body.get("health", 100))
            if health <= 25 and health > 0:
                candidates.append({
                    "id": "critical_injury",
                    "kind": "emergency",
                    "label": "Seek medical attention immediately",
                    "severity": 100 - health * 2,
                    "action": "heal",
                    "reason": f"health critically low at {health}",
                })
            elif health == 0:
                candidates.append({
                    "id": "death",
                    "kind": "emergency",
                    "label": "Character is dead",
                    "score": 100,
                    "action": "none",
                    "reason": "character is dead",
                })

            # Fatigue emergency
            fatigue = int(body.get("fatigue", 0))
            if fatigue >= 100:
                candidates.append({
                    "id": "exhaustion_collapse",
                    "kind": "emergency",
                    "label": "Collapsed from total exhaustion",
                    "score": 95,
                    "action": "rest",
                    "reason": "fatigue reached 100 (total exhaustion)",
                })

            # Needs
            needs = body.get("needs", [])
            if isinstance(needs, list):
                for n in needs:
                    if isinstance(n, dict) and "id" in n:
                        nid = n["id"]
                        sats = n.get("satisfiers", [])
                        suggested_action = sats[0].get("action") if sats else None
                        candidates.append({
                            "id": f"need_{nid}",
                            "kind": "need",
                            "label": n.get("label", nid),
                            "level": int(n.get("level", 50)),
                            "threshold": int(n.get("threshold", 70)),
                            "action": suggested_action,
                        })

        # 2. Synthesize routine slot
        routine = args.get("routine")
        if isinstance(routine, dict) and "id" in routine:
            candidates.append({
                "id": f"routine_{routine['id']}",
                "kind": "routine",
                "label": routine.get("label", routine["id"]),
                "priority": int(routine.get("priority", 50)),
                "window_active": routine.get("window_active", True),
                "time_to_start": int(routine.get("time_to_start", 0)),
                "action": routine.get("action", routine["id"]),
            })

        # 3. Synthesize goals
        goals = args.get("goals", [])
        if isinstance(goals, list):
            for g in goals:
                if isinstance(g, dict) and "id" in g:
                    candidates.append({
                        "id": f"goal_{g['id']}",
                        "kind": "goal",
                        "label": g.get("description", g["id"]),
                        "priority": int(g.get("priority", 50)),
                        "active": g.get("active", True),
                        "action": g.get("action"),
                    })

        # 4. Synthesize promises
        promises = args.get("promises", [])
        if isinstance(promises, list):
            for p in promises:
                if isinstance(p, dict) and "id" in p:
                    candidates.append({
                        "id": f"promise_{p['id']}",
                        "kind": "promise",
                        "label": p.get("task", p["id"]),
                        "priority": int(p.get("priority", 70)),
                        "remaining_ticks": int(p.get("remaining_ticks", 10)),
                        "overdue": p.get("overdue", False),
                        "action": p.get("action"),
                    })

        # 5. Synthesize emergencies
        emergencies = args.get("emergencies", [])
        if isinstance(emergencies, list):
            for e in emergencies:
                if isinstance(e, dict) and "id" in e:
                    candidates.append({
                        "id": f"emergency_{e['id']}",
                        "kind": "emergency",
                        "label": e.get("label", e["id"]),
                        "severity": int(e.get("severity", 50)),
                        "action": e.get("action"),
                    })

        # 6. Synthesize reaction brains
        brains = args.get("brains", [])
        if isinstance(brains, list):
            for b in brains:
                if isinstance(b, dict) and "id" in b:
                    candidates.append({
                        "id": f"reaction_{b['id']}",
                        "kind": "reaction",
                        "label": b.get("label", b["id"]),
                        "raw_score": int(b.get("score", 50)),
                        "action": b.get("action"),
                    })

        # 7. Add explicit motives
        explicit = args.get("motives", [])
        if isinstance(explicit, list):
            for m in explicit:
                if isinstance(m, dict):
                    candidates.append(m)

        if len(candidates) > MAX_MOTIVES:
            raise ValueError("too many motives to arbitrate")

        # Evaluate and score all candidates
        evaluated = [self.evaluate_motive(c) for c in candidates]

        # Sort: score DESC, KIND_PRECEDENCE DESC, id ASC
        evaluated.sort(
            key=lambda x: (-x["score"], -KIND_PRECEDENCE.get(x["kind"], 0), x["id"])
        )

        winner = evaluated[0] if evaluated else None
        winning_reason = ""
        if winner:
            if len(evaluated) > 1:
                runner_up = evaluated[1]
                winning_reason = f"{winner['id']} (score {winner['score']}) won over {runner_up['id']} (score {runner_up['score']}) because {winner['reason']}"
            else:
                winning_reason = f"{winner['id']} (score {winner['score']}) won because {winner['reason']}"

        out = {
            "winner": winner,
            "winning_reason": winning_reason,
            "ranked": evaluated,
            "count": len(evaluated),
        }
        cm = args.get("commitment")
        if cm is not None:
            # Commitment inertia (pass 2, D1).
            if not isinstance(cm, dict) or "motive" not in cm or any(k not in ("motive", "inertia") for k in cm):
                raise ValueError("invalid commitment")
            cid = cm["motive"]
            if not isinstance(cid, str) or not (0 < len(cid) <= 64) or not all(
                ch.isascii() and (ch.isalnum() or ch in "_:-") for ch in cid
            ):
                raise ValueError("invalid commitment motive")
            inertia = cm.get("inertia")
            if inertia is None:
                inertia = 10
            if not isinstance(inertia, int) or isinstance(inertia, bool) or not (0 <= inertia <= 50):
                raise ValueError("invalid inertia")
            ids = [m["id"] for m in evaluated]
            present = cid in ids
            held = False
            if present and ids.index(cid) > 0:
                top = evaluated[0]
                kept = evaluated[ids.index(cid)]
                if top["kind"] != "emergency" and top["score"] < kept["score"] + inertia:
                    held = True
                    evaluated.remove(kept)
                    evaluated.insert(0, kept)
                    out["winner"] = kept
                    out["winning_reason"] = (
                        f"{kept['id']} (score {kept['score']}) held over {top['id']} (score {top['score']})"
                        f" by commitment inertia {inertia} because {kept['reason']}"
                    )
            out["commitment"] = {"held": held, "inertia": inertia, "motive": cid, "present": present}
        return out

    def workflow(self, req: Dict[str, Any]) -> Dict[str, Any]:
        if not isinstance(req, dict):
            raise ValueError("request must be dict")
        ops = req.get("ops")
        if not isinstance(ops, list) or not ops or len(ops) > MAX_OPS:
            raise ValueError("invalid ops list")

        results = []
        for op_entry in ops:
            if not isinstance(op_entry, dict) or "op" not in op_entry:
                raise ValueError("invalid op entry")
            op = op_entry["op"]
            args = op_entry.get("args", {})
            if not isinstance(args, dict):
                raise ValueError("args must be dict")

            if op == "arbitrate":
                results.append(self.op_arbitrate(args))
            else:
                raise ValueError(f"unknown op: {op}")

        return {"version": 1, "results": results}
