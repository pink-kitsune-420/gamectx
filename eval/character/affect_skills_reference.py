#!/usr/bin/env python3
"""Affect decay, missing feelings, and skills growth reference implementation (Gaps 6 and 8).

Pure host computation for:
- Affect dynamics: mood (valence/arousal), temperament baselines, attitude decay toward baseline.
  Additive attitude kinds: fear, respect, gratitude, envy, admiration.
- Skills dynamics: practice gain with diminishing returns, threshold scaling,
  duration/risk/quality action scaling, and teaching between beings.
"""

from typing import Any, Dict, List, Optional, Tuple

MAX_BEINGS = 64
MAX_OPS = 64
MAX_ATTITUDES = 64
MAX_SKILLS_PER_BEING = 32

ATTITUDE_KINDS = {
    "preference", "opinion", "familiarity", "trust", "affection", "resentment",
    "obligation", "disagreement", "belief", "report", "hypothesis", "misconception",
    "unknown", "goal", "commitment", "unfinished", "priority", "physical",
    "emotion", "fear", "respect", "gratitude", "envy", "admiration"
}


def mood_label(valence: int, arousal: int) -> str:
    if valence >= 30 and arousal >= 20:
        return "excited"
    elif valence >= 30 and arousal < -20:
        return "serene"
    elif valence >= 30:
        return "cheerful"
    elif valence <= -30 and arousal >= 20:
        return "agitated"
    elif valence <= -30 and arousal < -20:
        return "depressed"
    elif valence <= -30:
        return "irritable"
    elif arousal >= 40:
        return "tense"
    elif arousal <= -40:
        return "lethargic"
    else:
        return "calm"


def threshold_for_level(level: int) -> int:
    return 100 + level * 15


def quality_tier(score: int) -> str:
    if score <= 24:
        return "poor"
    elif score <= 49:
        return "standard"
    elif score <= 74:
        return "fine"
    elif score <= 89:
        return "superior"
    else:
        return "masterwork"


class AffectReference:
    def __init__(self):
        self.beings: Dict[str, Dict[str, Any]] = {}

    def op_init(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being = args.get("being")
        if not being or not isinstance(being, str) or len(being) > 64:
            raise ValueError("invalid being id")

        if len(self.beings) >= MAX_BEINGS and being not in self.beings:
            raise ValueError("too many beings")

        temp = args.get("temperament", "calm")
        if not isinstance(temp, str) or len(temp) > 32:
            raise ValueError("invalid temperament")

        mood_arg = args.get("mood") or {}
        valence = int(mood_arg.get("valence", 0))
        arousal = int(mood_arg.get("arousal", 0))
        bv = int(mood_arg.get("baseline_valence", 0))
        ba = int(mood_arg.get("baseline_arousal", 0))
        dr = int(mood_arg.get("decay_rate", 5))

        if not (-100 <= valence <= 100 and -100 <= arousal <= 100 and -100 <= bv <= 100 and -100 <= ba <= 100 and 0 <= dr <= 100):
            raise ValueError("mood values out of bounds")

        mood = {
            "valence": valence,
            "arousal": arousal,
            "baseline_valence": bv,
            "baseline_arousal": ba,
            "decay_rate": dr,
            "label": mood_label(valence, arousal)
        }

        attitudes: Dict[str, Dict[str, Any]] = {}
        for item in args.get("attitudes", []):
            if len(attitudes) >= MAX_ATTITUDES:
                raise ValueError("too many attitudes")
            aid = item.get("id")
            kind = item.get("kind")
            subject = item.get("subject")
            lvl = int(item.get("level", 0))

            if not aid or not isinstance(aid, str) or len(aid) > 64:
                raise ValueError("invalid attitude id")
            if kind not in ATTITUDE_KINDS:
                raise ValueError(f"invalid attitude kind {kind}")
            if not subject or not isinstance(subject, str) or len(subject) > 64:
                raise ValueError("invalid subject")
            if not (-100 <= lvl <= 100):
                raise ValueError("level out of bounds")

            default_base = 0 if kind in {"emotion", "fear", "gratitude", "envy", "admiration"} else lvl
            base = int(item.get("baseline", default_base))
            if not (-100 <= base <= 100):
                raise ValueError("baseline out of bounds")

            default_rate = 5 if kind == "emotion" else 4 if kind == "fear" else 3 if kind in {"admiration", "gratitude", "envy"} else 2 if kind == "resentment" else 1 if kind in {"affection", "trust"} else 0
            rate = int(item.get("decay_rate", default_rate))
            if not (0 <= rate <= 100):
                raise ValueError("decay_rate out of bounds")

            reason = str(item.get("reason", ""))
            attitudes[aid] = {
                "id": aid,
                "kind": kind,
                "subject": subject,
                "level": lvl,
                "baseline": base,
                "decay_rate": rate,
                "reason": reason
            }

        state = {
            "being": being,
            "temperament": temp,
            "mood": mood,
            "attitudes": attitudes
        }
        self.beings[being] = state
        return self._format_state(state)

    def _format_state(self, state: Dict[str, Any]) -> Dict[str, Any]:
        atts = [dict(state["attitudes"][k]) for k in sorted(state["attitudes"].keys())]
        return {
            "being": state["being"],
            "temperament": state["temperament"],
            "mood": dict(state["mood"]),
            "attitudes": atts
        }

    def op_assess(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being = args.get("being")
        if being not in self.beings:
            raise ValueError("being not found")
        return self._format_state(self.beings[being])

    def op_tick(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being = args.get("being")
        ticks = int(args.get("ticks", 1))
        if being not in self.beings:
            raise ValueError("being not found")
        if not (1 <= ticks <= 100000):
            raise ValueError("ticks out of bounds")

        state = self.beings[being]
        m = state["mood"]
        m_decay = m["decay_rate"] * ticks

        if m["valence"] > m["baseline_valence"]:
            m["valence"] = max(m["baseline_valence"], m["valence"] - m_decay)
        elif m["valence"] < m["baseline_valence"]:
            m["valence"] = min(m["baseline_valence"], m["valence"] + m_decay)

        if m["arousal"] > m["baseline_arousal"]:
            m["arousal"] = max(m["baseline_arousal"], m["arousal"] - m_decay)
        elif m["arousal"] < m["baseline_arousal"]:
            m["arousal"] = min(m["baseline_arousal"], m["arousal"] + m_decay)

        m["label"] = mood_label(m["valence"], m["arousal"])

        temp = state["temperament"]
        for att in state["attitudes"].values():
            rate = att["decay_rate"]
            if ("calm" in temp and att["kind"] in {"emotion", "fear"}) or ("forgiving" in temp and att["kind"] == "resentment"):
                rate += 2
            elif "volatile" in temp and att["kind"] in {"emotion", "fear"}:
                rate = max(1, rate - 2)
            elif "vengeful" in temp and att["kind"] == "resentment":
                rate = max(1, rate - 1)

            if rate > 0:
                amt = rate * ticks
                if att["level"] > att["baseline"]:
                    att["level"] = max(att["baseline"], att["level"] - amt)
                elif att["level"] < att["baseline"]:
                    att["level"] = min(att["baseline"], att["level"] + amt)

        return self._format_state(state)

    def op_appraise(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being = args.get("being")
        kind = args.get("kind")
        subject = args.get("subject")
        delta = int(args.get("delta", 0))
        reason = str(args.get("reason", ""))

        if being not in self.beings:
            raise ValueError("being not found")
        if kind not in ATTITUDE_KINDS:
            raise ValueError("invalid kind")
        if not subject or len(subject) > 64:
            raise ValueError("invalid subject")
        if not (-100 <= delta <= 100):
            raise ValueError("delta out of bounds")

        att_id = args.get("id") or f"{kind}_{subject}"
        state = self.beings[being]

        if att_id in state["attitudes"]:
            att = state["attitudes"][att_id]
            before = att["level"]
            att["level"] = max(-100, min(100, att["level"] + delta))
            if reason:
                att["reason"] = reason
            after = att["level"]
        else:
            if len(state["attitudes"]) >= MAX_ATTITUDES:
                raise ValueError("too many attitudes")
            default_base = 0 if kind in {"emotion", "fear", "gratitude", "envy", "admiration"} else 0
            default_rate = 5 if kind == "emotion" else 4 if kind == "fear" else 3 if kind in {"admiration", "gratitude", "envy"} else 2 if kind == "resentment" else 1 if kind in {"affection", "trust"} else 0
            lvl = max(-100, min(100, delta))
            state["attitudes"][att_id] = {
                "id": att_id,
                "kind": kind,
                "subject": subject,
                "level": lvl,
                "baseline": default_base,
                "decay_rate": default_rate,
                "reason": reason
            }
            before = 0
            after = lvl

        m = state["mood"]
        if kind in {"gratitude", "respect", "admiration", "affection", "trust"}:
            m["valence"] = max(-100, min(100, m["valence"] + int(delta / 2)))
            m["arousal"] = max(-100, min(100, m["arousal"] + int(abs(delta) / 4)))
        elif kind in {"fear", "resentment", "envy"}:
            m["valence"] = max(-100, min(100, m["valence"] - int(abs(delta) / 2)))
            m["arousal"] = max(-100, min(100, m["arousal"] + int(abs(delta) / 2)))
        else:
            m["valence"] = max(-100, min(100, m["valence"] + int(delta / 4)))

        m["label"] = mood_label(m["valence"], m["arousal"])

        return {
            "being": being,
            "attitude_id": att_id,
            "kind": kind,
            "subject": subject,
            "before": before,
            "after": after,
            "delta": delta,
            "reason": reason,
            "mood": dict(m)
        }

    def op_set_mood(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being = args.get("being")
        if being not in self.beings:
            raise ValueError("being not found")
        m = self.beings[being]["mood"]

        if "valence" in args:
            m["valence"] = max(-100, min(100, int(args["valence"])))
        if "arousal" in args:
            m["arousal"] = max(-100, min(100, int(args["arousal"])))
        if "valence_delta" in args:
            m["valence"] = max(-100, min(100, m["valence"] + int(args["valence_delta"])))
        if "arousal_delta" in args:
            m["arousal"] = max(-100, min(100, m["arousal"] + int(args["arousal_delta"])))

        m["label"] = mood_label(m["valence"], m["arousal"])
        return {
            "being": being,
            "mood": dict(m)
        }

    def workflow(self, req: Dict[str, Any]) -> Dict[str, Any]:
        ops = req.get("ops")
        if not ops or not isinstance(ops, list) or len(ops) > MAX_OPS:
            raise ValueError("invalid ops")

        results = []
        for op_call in ops:
            op_name = op_call.get("op")
            args = op_call.get("args") or {}
            if op_name == "init":
                res = self.op_init(args)
            elif op_name == "assess":
                res = self.op_assess(args)
            elif op_name == "tick":
                res = self.op_tick(args)
            elif op_name == "appraise":
                res = self.op_appraise(args)
            elif op_name == "set_mood":
                res = self.op_set_mood(args)
            else:
                raise ValueError(f"unknown affect op {op_name}")
            results.append(res)

        return {"version": 1, "results": results}


class SkillsReference:
    def __init__(self):
        self.beings: Dict[str, Dict[str, Any]] = {}

    def op_init(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being = args.get("being")
        if not being or not isinstance(being, str) or len(being) > 64:
            raise ValueError("invalid being id")

        if len(self.beings) >= MAX_BEINGS and being not in self.beings:
            raise ValueError("too many beings")

        skills = {}
        for item in args.get("skills", []):
            if len(skills) >= MAX_SKILLS_PER_BEING:
                raise ValueError("too many skills")
            sid = item.get("id")
            if not sid or not isinstance(sid, str) or len(sid) > 64:
                raise ValueError("invalid skill id")
            label = str(item.get("label", sid))
            lvl = min(100, int(item.get("level", 1)))
            practice = 0 if lvl >= 100 else int(item.get("practice", 0))
            thresh = threshold_for_level(lvl)
            skills[sid] = {
                "id": sid,
                "label": label,
                "level": lvl,
                "practice": practice,
                "threshold": thresh,
                "total_actions": 0
            }

        state = {"being": being, "skills": skills}
        self.beings[being] = state
        return self._format_state(state)

    def _format_state(self, state: Dict[str, Any]) -> Dict[str, Any]:
        s_list = []
        for sid in sorted(state["skills"].keys()):
            sk = state["skills"][sid]
            pct = 100 if sk["level"] >= 100 else (sk["practice"] * 100 // sk["threshold"]) if sk["threshold"] > 0 else 0
            s_list.append({
                "id": sk["id"],
                "label": sk["label"],
                "level": sk["level"],
                "practice": sk["practice"],
                "threshold": sk["threshold"],
                "progress_percent": pct,
                "total_actions": sk["total_actions"]
            })
        return {"being": state["being"], "skills": s_list}

    def op_assess(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being = args.get("being")
        if being not in self.beings:
            raise ValueError("being not found")
        return self._format_state(self.beings[being])

    def op_practice(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being = args.get("being")
        skill_id = args.get("skill")
        raw_practice = int(args.get("practice", 0))
        success = bool(args.get("success", False))

        if being not in self.beings:
            raise ValueError("being not found")
        if not skill_id or len(skill_id) > 64:
            raise ValueError("invalid skill")

        state = self.beings[being]
        if skill_id not in state["skills"]:
            label = str(args.get("label", skill_id))
            state["skills"][skill_id] = {
                "id": skill_id,
                "label": label,
                "level": 0,
                "practice": 0,
                "threshold": threshold_for_level(0),
                "total_actions": 0
            }

        rec = state["skills"][skill_id]
        if not success or raw_practice == 0:
            return {
                "being": being,
                "skill": skill_id,
                "level": rec["level"],
                "practice": rec["practice"],
                "threshold": rec["threshold"],
                "effective_gain": 0,
                "leveled_up": False,
                "success": success
            }

        rec["total_actions"] += 1
        if rec["level"] >= 100:
            return {
                "being": being,
                "skill": skill_id,
                "level": 100,
                "practice": 0,
                "threshold": rec["threshold"],
                "effective_gain": 0,
                "leveled_up": False,
                "success": True
            }

        factor = max(10, 100 - rec["level"])
        effective_gain = max(1, (raw_practice * factor) // 100)
        rec["practice"] += effective_gain

        leveled_up = False
        while rec["practice"] >= rec["threshold"] and rec["level"] < 100:
            rec["practice"] -= rec["threshold"]
            rec["level"] += 1
            rec["threshold"] = threshold_for_level(rec["level"])
            leveled_up = True

        if rec["level"] >= 100:
            rec["practice"] = 0

        return {
            "being": being,
            "skill": skill_id,
            "level": rec["level"],
            "practice": rec["practice"],
            "threshold": rec["threshold"],
            "effective_gain": effective_gain,
            "leveled_up": leveled_up,
            "success": True
        }

    def op_evaluate_action(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being = args.get("being")
        action = args.get("action")
        base_dur = int(args.get("base_duration", 0))
        base_risk = int(args.get("base_risk", 0))
        quality_bonus = int(args.get("quality_bonus", 0))
        skill_id = args.get("skill")

        if not being or not action:
            raise ValueError("missing being or action")

        lvl = 0
        if skill_id and being in self.beings and skill_id in self.beings[being]["skills"]:
            lvl = self.beings[being]["skills"][skill_id]["level"]

        discount = (lvl * 50) // 100
        dur = (base_dur * (100 - discount)) // 100
        risk_discount = (lvl * base_risk) // 100
        risk = max(0, base_risk - risk_discount)
        quality_score = min(100, lvl + quality_bonus)
        tier = quality_tier(quality_score)

        return {
            "being": being,
            "action": action,
            "skill": skill_id,
            "level": lvl,
            "base_duration": base_dur,
            "duration": dur,
            "duration_discount_percent": discount,
            "base_risk": base_risk,
            "risk": risk,
            "quality_score": quality_score,
            "quality_tier": tier
        }

    def op_teach(self, args: Dict[str, Any]) -> Dict[str, Any]:
        teacher = args.get("teacher")
        student = args.get("student")
        skill_id = args.get("skill")
        hours = int(args.get("hours", 1))
        same_place = bool(args.get("same_place", True))

        if not teacher or not student or not skill_id:
            raise ValueError("missing parameters")
        if not (1 <= hours <= 24):
            raise ValueError("hours out of bounds")

        if not same_place:
            return {
                "ok": False,
                "reason": "teacher and student must be in the same place"
            }

        t_lvl = 0
        if teacher in self.beings and skill_id in self.beings[teacher]["skills"]:
            t_lvl = self.beings[teacher]["skills"][skill_id]["level"]

        s_lvl = 0
        if student in self.beings and skill_id in self.beings[student]["skills"]:
            s_lvl = self.beings[student]["skills"][skill_id]["level"]

        if t_lvl <= s_lvl:
            return {
                "ok": False,
                "reason": "teacher skill level must be strictly greater than student level"
            }

        gap = t_lvl - s_lvl
        base_rate = 15 + gap // 2
        raw_practice = base_rate * hours

        student_being = self.beings[student]
        if skill_id not in student_being["skills"]:
            student_being["skills"][skill_id] = {
                "id": skill_id,
                "label": skill_id,
                "level": 0,
                "practice": 0,
                "threshold": threshold_for_level(0),
                "total_actions": 0
            }

        s_rec = student_being["skills"][skill_id]
        factor = max(10, 100 - s_rec["level"])
        effective_gain = max(1, (raw_practice * factor) // 100)

        s_rec["practice"] += effective_gain
        s_rec["total_actions"] += 1
        leveled_up = False
        while s_rec["practice"] >= s_rec["threshold"] and s_rec["level"] < t_lvl:
            s_rec["practice"] -= s_rec["threshold"]
            s_rec["level"] += 1
            s_rec["threshold"] = threshold_for_level(s_rec["level"])
            leveled_up = True

        if s_rec["level"] >= t_lvl:
            s_rec["level"] = t_lvl
            s_rec["practice"] = 0

        respect_delta = min(50, 5 * hours)
        gratitude_delta = min(50, 5 * hours)

        rel_effects = [
            {
                "subject": student,
                "target": teacher,
                "attitude": "respect",
                "delta": respect_delta,
                "reason": "learned from teacher"
            },
            {
                "subject": student,
                "target": teacher,
                "attitude": "gratitude",
                "delta": gratitude_delta,
                "reason": "received dedicated instruction"
            }
        ]

        return {
            "ok": True,
            "teacher": teacher,
            "student": student,
            "skill": skill_id,
            "teacher_level": t_lvl,
            "student_level": s_rec["level"],
            "student_practice": s_rec["practice"],
            "practice_gained": effective_gain,
            "leveled_up": leveled_up,
            "relationship_effects": rel_effects
        }

    def workflow(self, req: Dict[str, Any]) -> Dict[str, Any]:
        ops = req.get("ops")
        if not ops or not isinstance(ops, list) or len(ops) > MAX_OPS:
            raise ValueError("invalid ops")

        results = []
        for op_call in ops:
            op_name = op_call.get("op")
            args = op_call.get("args") or {}
            if op_name == "init":
                res = self.op_init(args)
            elif op_name == "assess":
                res = self.op_assess(args)
            elif op_name == "practice":
                res = self.op_practice(args)
            elif op_name == "evaluate_action":
                res = self.op_evaluate_action(args)
            elif op_name == "teach":
                res = self.op_teach(args)
            else:
                raise ValueError(f"unknown skills op {op_name}")
            results.append(res)

        return {"version": 1, "results": results}


def workflow(req: Dict[str, Any]) -> Dict[str, Any]:
    # Determine dispatcher by kind or first op
    kind = req.get("kind")
    if kind == "affect":
        mgr = AffectReference()
        return mgr.workflow(req.get("args", req))
    elif kind == "skills":
        mgr = SkillsReference()
        return mgr.workflow(req.get("args", req))

    # Infer from ops
    ops = req.get("ops", [])
    if ops and ops[0].get("op") in {"practice", "evaluate_action", "teach"}:
        return SkillsReference().workflow(req)
    elif ops and any("temperament" in (o.get("args") or {}) or "mood" in (o.get("args") or {}) for o in ops):
        return AffectReference().workflow(req)
    else:
        # Default to skills if skill in args, else affect
        if ops and "skill" in (ops[0].get("args") or {}):
            return SkillsReference().workflow(req)
        return AffectReference().workflow(req)


if __name__ == "__main__":
    import json, sys
    data = json.load(sys.stdin)
    out = workflow(data)
    json.dump(out, sys.stdout)
