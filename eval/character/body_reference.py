#!/usr/bin/env python3
"""Body and needs reference implementation (Gap 2 and 12).

Pure host computation for character body state and needs dynamics:
- Need definitions: id, label, rate_per_tick, threshold, satisfiers (action or item -> amount)
- Body state: fatigue (0..100, accumulates with actions, recovers with rest),
  health (0..100), injuries, conditions (asleep, sick, unconscious, dead).
- Condition effects:
  - Action catalog: blocked verbs, extra fatigue/time costs.
  - Routines: forced rest / incapacitated.
  - Speech: silent while asleep, unconscious, or dead.
"""

from typing import Any, Dict, List, Optional, Tuple

MAX_BEINGS = 64
MAX_OPS = 64
MAX_NEEDS = 16
MAX_INJURIES = 16
MAX_CONDITIONS = 16

STANDARD_CONDITIONS = {"asleep", "sick", "unconscious", "dead"}


class BodyReference:
    def __init__(self):
        # being_id -> dict
        self.beings: Dict[str, Dict[str, Any]] = {}

    def _default_being(self, being_id: str) -> Dict[str, Any]:
        return {
            "id": being_id,
            "fatigue": 0,
            "health": 100,
            "conditions": [],
            "injuries": [],
            "needs": {},
        }

    def op_init(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being_id = args.get("being") or args.get("id")
        if not being_id or not isinstance(being_id, str):
            raise ValueError("invalid being id")

        if len(self.beings) >= MAX_BEINGS and being_id not in self.beings:
            raise ValueError("too many beings")

        being = self._default_being(being_id)
        if "fatigue" in args:
            f = int(args["fatigue"])
            if not (0 <= f <= 100):
                raise ValueError("fatigue out of bounds")
            being["fatigue"] = f

        if "health" in args:
            h = int(args["health"])
            if not (0 <= h <= 100):
                raise ValueError("health out of bounds")
            being["health"] = h
            if h == 0 and "dead" not in being["conditions"]:
                being["conditions"].append("dead")

        if "conditions" in args:
            conds = args["conditions"]
            if not isinstance(conds, list) or len(conds) > MAX_CONDITIONS:
                raise ValueError("invalid conditions")
            seen = set()
            clean_conds = []
            for c in conds:
                if not isinstance(c, str) or not c or c in seen:
                    raise ValueError("invalid condition entry")
                seen.add(c)
                clean_conds.append(c)
            being["conditions"] = clean_conds

        if "injuries" in args:
            injs = args["injuries"]
            if not isinstance(injs, list) or len(injs) > MAX_INJURIES:
                raise ValueError("invalid injuries")
            clean_injs = []
            seen_ids = set()
            for inj in injs:
                if not isinstance(inj, dict) or "id" not in inj:
                    raise ValueError("invalid injury entry")
                iid = inj["id"]
                if not isinstance(iid, str) or not iid or iid in seen_ids:
                    raise ValueError("duplicate or invalid injury id")
                seen_ids.add(iid)
                sev = int(inj.get("severity", 10))
                if not (0 <= sev <= 100):
                    raise ValueError("injury severity out of bounds")
                clean_injs.append({
                    "id": iid,
                    "label": inj.get("label", iid),
                    "severity": sev,
                    "location": inj.get("location", "general"),
                    "effects": inj.get("effects", {}),
                })
            being["injuries"] = clean_injs

        if "needs" in args:
            needs_list = args["needs"]
            if not isinstance(needs_list, list) or len(needs_list) > MAX_NEEDS:
                raise ValueError("invalid needs list")
            needs_map = {}
            for n in needs_list:
                if not isinstance(n, dict) or "id" not in n:
                    raise ValueError("invalid need entry")
                nid = n["id"]
                if not isinstance(nid, str) or not nid or nid in needs_map:
                    raise ValueError("duplicate or invalid need id")
                thresh = int(n.get("threshold", 70))
                if not (0 <= thresh <= 100):
                    raise ValueError("threshold out of bounds")
                rate = int(n.get("rate_per_tick", 0))
                if not (0 <= rate <= 100):
                    raise ValueError("rate_per_tick out of bounds")
                init_val = int(n.get("level", 0))
                if not (0 <= init_val <= 100):
                    raise ValueError("need level out of bounds")
                satisfiers = n.get("satisfiers", [])
                if not isinstance(satisfiers, list):
                    raise ValueError("satisfiers must be list")
                clean_sats = []
                for s in satisfiers:
                    if not isinstance(s, dict):
                        raise ValueError("satisfier must be dict")
                    act = s.get("action")
                    itm = s.get("item")
                    amt = int(s.get("amount", 20))
                    if not (0 <= amt <= 100):
                        raise ValueError("satisfier amount out of bounds")
                    clean_sats.append({
                        "action": act,
                        "item": itm,
                        "amount": amt,
                    })
                needs_map[nid] = {
                    "id": nid,
                    "label": n.get("label", nid),
                    "threshold": thresh,
                    "rate_per_tick": rate,
                    "level": init_val,
                    "satisfiers": clean_sats,
                }
            being["needs"] = needs_map

        self.beings[being_id] = being
        return {"ok": True, "being": being_id}

    def _get_being(self, being_id: str) -> Dict[str, Any]:
        if being_id not in self.beings:
            self.beings[being_id] = self._default_being(being_id)
        return self.beings[being_id]

    def can_speak(self, being: Dict[str, Any]) -> Tuple[bool, Optional[str]]:
        conds = being["conditions"]
        if "dead" in conds:
            return False, "dead"
        if "unconscious" in conds:
            return False, "unconscious"
        if "asleep" in conds:
            return False, "asleep"
        return True, None

    def routine_override(self, being: Dict[str, Any]) -> Optional[str]:
        conds = being["conditions"]
        if "dead" in conds:
            return "incapacitated"
        if "unconscious" in conds:
            return "incapacitated"
        if "asleep" in conds:
            return "rest"
        if being["fatigue"] >= 90:
            return "rest"
        return None

    def op_assess(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being_id = args.get("being") or args.get("id")
        if not being_id or not isinstance(being_id, str):
            raise ValueError("invalid being id")
        being = self._get_being(being_id)
        can_spk, spk_ref = self.can_speak(being)
        r_override = self.routine_override(being)

        # Summarize needs
        needs_arr = []
        for nid in sorted(being["needs"].keys()):
            n = being["needs"][nid]
            needs_arr.append({
                "id": n["id"],
                "label": n["label"],
                "level": n["level"],
                "threshold": n["threshold"],
                "rate_per_tick": n["rate_per_tick"],
                "urgent": n["level"] >= n["threshold"],
            })

        return {
            "being": being_id,
            "fatigue": being["fatigue"],
            "health": being["health"],
            "conditions": sorted(being["conditions"]),
            "injuries": being["injuries"],
            "needs": needs_arr,
            "can_speak": can_spk,
            "speech_refusal": spk_ref,
            "routine_override": r_override,
        }

    def op_tick(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being_id = args.get("being") or args.get("id")
        if not being_id or not isinstance(being_id, str):
            raise ValueError("invalid being id")
        being = self._get_being(being_id)

        ticks = int(args.get("ticks", 1))
        if ticks < 1 or ticks > 1000:
            raise ValueError("ticks must be between 1 and 1000")

        # If dead, nothing advances
        if "dead" in being["conditions"]:
            return {
                "ok": True,
                "being": being_id,
                "fatigue": being["fatigue"],
                "health": 0,
                "conditions": sorted(being["conditions"]),
            }

        is_asleep = "asleep" in being["conditions"]
        is_sick = "sick" in being["conditions"]
        resting = args.get("resting", False) or is_asleep

        # Fatigue recovery
        if is_asleep:
            recovery = 15 * ticks
            being["fatigue"] = max(0, being["fatigue"] - recovery)
        elif resting:
            recovery = 5 * ticks
            being["fatigue"] = max(0, being["fatigue"] - recovery)

        # Advance needs
        deprived_damage = 0
        for nid, n in being["needs"].items():
            rate = n["rate_per_tick"]
            if is_sick:
                rate = rate + 1
            n["level"] = min(100, n["level"] + rate * ticks)
            if n["level"] >= 100 and rate > 0:
                # Maxed out need causes 5 damage per tick
                deprived_damage += 5 * ticks

        if deprived_damage > 0:
            being["health"] = max(0, being["health"] - deprived_damage)
            if being["health"] == 0:
                if "dead" not in being["conditions"]:
                    being["conditions"].append("dead")
                if "asleep" in being["conditions"]:
                    being["conditions"].remove("asleep")

        return {
            "ok": True,
            "being": being_id,
            "fatigue": being["fatigue"],
            "health": being["health"],
            "conditions": sorted(being["conditions"]),
        }

    def op_satisfy(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being_id = args.get("being") or args.get("id")
        if not being_id or not isinstance(being_id, str):
            raise ValueError("invalid being id")
        being = self._get_being(being_id)

        if "dead" in being["conditions"]:
            raise ValueError("cannot satisfy needs of dead being")

        satisfied = []
        # Direct satisfaction
        if "need" in args:
            nid = args["need"]
            if nid not in being["needs"]:
                raise ValueError("unknown need")
            amt = int(args.get("amount", 20))
            if amt < 0:
                raise ValueError("negative satisfy amount")
            cur = being["needs"][nid]["level"]
            being["needs"][nid]["level"] = max(0, cur - amt)
            satisfied.append({"need": nid, "amount": amt, "new_level": being["needs"][nid]["level"]})

        # By item
        elif "item" in args:
            item_name = args["item"]
            count = int(args.get("count", 1))
            if count < 1:
                raise ValueError("invalid item count")
            matched = False
            for nid, n in being["needs"].items():
                for sat in n["satisfiers"]:
                    if sat.get("item") == item_name:
                        matched = True
                        amt = sat.get("amount", 20) * count
                        cur = n["level"]
                        n["level"] = max(0, cur - amt)
                        satisfied.append({"need": nid, "amount": amt, "new_level": n["level"]})
            if not matched:
                raise ValueError(f"no satisfier for item {item_name}")

        # By action
        elif "action" in args:
            action_name = args["action"]
            matched = False
            for nid, n in being["needs"].items():
                for sat in n["satisfiers"]:
                    if sat.get("action") == action_name:
                        matched = True
                        amt = sat.get("amount", 20)
                        cur = n["level"]
                        n["level"] = max(0, cur - amt)
                        satisfied.append({"need": nid, "amount": amt, "new_level": n["level"]})
            if not matched:
                raise ValueError(f"no satisfier for action {action_name}")

        else:
            raise ValueError("must specify need, item, or action")

        return {"ok": True, "being": being_id, "satisfied": satisfied}

    def op_rest(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being_id = args.get("being") or args.get("id")
        if not being_id or not isinstance(being_id, str):
            raise ValueError("invalid being id")
        being = self._get_being(being_id)

        if "dead" in being["conditions"]:
            raise ValueError("dead being cannot rest")

        ticks = int(args.get("ticks", 1))
        if ticks < 1 or ticks > 1000:
            raise ValueError("ticks must be between 1 and 1000")

        asleep = bool(args.get("asleep", False))
        if asleep:
            if "asleep" not in being["conditions"]:
                being["conditions"].append("asleep")
        else:
            if "asleep" in being["conditions"]:
                being["conditions"].remove("asleep")

        return self.op_tick({"being": being_id, "ticks": ticks, "resting": True})

    def op_apply_action(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being_id = args.get("being") or args.get("id")
        if not being_id or not isinstance(being_id, str):
            raise ValueError("invalid being id")
        being = self._get_being(being_id)

        action = args.get("action")
        if not isinstance(action, dict) or "id" not in action:
            raise ValueError("invalid action definition")

        act_id = action["id"]
        conds = being["conditions"]

        # Condition gates
        if "dead" in conds:
            return {"ok": False, "being": being_id, "refusal": "actor is dead"}
        if "unconscious" in conds:
            return {"ok": False, "being": being_id, "refusal": "actor is unconscious"}
        if "asleep" in conds and act_id not in ("wake", "sleep", "rest"):
            return {"ok": False, "being": being_id, "refusal": "actor is asleep"}

        # Injuries check
        for inj in being["injuries"]:
            effects = inj.get("effects", {})
            blocked = effects.get("blocked_actions", [])
            if act_id in blocked:
                return {"ok": False, "being": being_id, "refusal": f"blocked by injury: {inj['id']}"}

        # Disallowed conditions in action actor spec
        actor_spec = action.get("actor", {})
        body_spec = actor_spec.get("body_state", {})
        disallowed = body_spec.get("disallowed_conditions", [])
        for c in conds:
            if c in disallowed:
                return {"ok": False, "being": being_id, "refusal": f"cannot act while {c}"}

        # Max fatigue limit
        max_f = body_spec.get("max_fatigue", 100)
        if being["fatigue"] > max_f:
            return {"ok": False, "being": being_id, "refusal": f"too fatigued ({being['fatigue']} > {max_f})"}

        # Min condition/health limit
        min_cond = body_spec.get("min_condition", 0)
        if being["health"] < min_cond:
            return {"ok": False, "being": being_id, "refusal": f"condition too low ({being['health']} < {min_cond})"}

        cost = action.get("cost", {})
        base_fatigue = int(cost.get("fatigue", 0))

        if being["fatigue"] >= 100 and base_fatigue > 0:
            return {"ok": False, "being": being_id, "refusal": "actor is exhausted"}

        # Sick penalty: +50% fatigue (min +2)
        effective_fatigue = base_fatigue
        if "sick" in conds and effective_fatigue > 0:
            effective_fatigue += max(2, effective_fatigue // 2)

        # Injury extra fatigue
        for inj in being["injuries"]:
            extra_f = int(inj.get("effects", {}).get("extra_fatigue", 0))
            effective_fatigue += extra_f

        being["fatigue"] = min(100, being["fatigue"] + effective_fatigue)

        # Check wake action
        if act_id == "wake" and "asleep" in being["conditions"]:
            being["conditions"].remove("asleep")

        # Check if action satisfies needs
        satisfied_needs = []
        for nid, n in being["needs"].items():
            for sat in n["satisfiers"]:
                if sat.get("action") == act_id:
                    amt = sat.get("amount", 20)
                    n["level"] = max(0, n["level"] - amt)
                    satisfied_needs.append({"need": nid, "amount": amt, "new_level": n["level"]})

        return {
            "ok": True,
            "being": being_id,
            "action": act_id,
            "effective_fatigue_cost": effective_fatigue,
            "fatigue": being["fatigue"],
            "health": being["health"],
            "conditions": sorted(being["conditions"]),
            "satisfied_needs": satisfied_needs,
        }

    def op_set_condition(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being_id = args.get("being") or args.get("id")
        if not being_id or not isinstance(being_id, str):
            raise ValueError("invalid being id")
        being = self._get_being(being_id)

        cond = args.get("condition")
        if not cond or not isinstance(cond, str):
            raise ValueError("invalid condition")

        if cond not in being["conditions"]:
            if len(being["conditions"]) >= MAX_CONDITIONS:
                raise ValueError("too many conditions")
            being["conditions"].append(cond)

        if cond == "dead":
            being["health"] = 0
            if "asleep" in being["conditions"]:
                being["conditions"].remove("asleep")

        return {"ok": True, "being": being_id, "conditions": sorted(being["conditions"])}

    def op_remove_condition(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being_id = args.get("being") or args.get("id")
        if not being_id or not isinstance(being_id, str):
            raise ValueError("invalid being id")
        being = self._get_being(being_id)

        cond = args.get("condition")
        if not cond or not isinstance(cond, str):
            raise ValueError("invalid condition")

        if cond in being["conditions"]:
            being["conditions"].remove(cond)

        return {"ok": True, "being": being_id, "conditions": sorted(being["conditions"])}

    def op_add_injury(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being_id = args.get("being") or args.get("id")
        if not being_id or not isinstance(being_id, str):
            raise ValueError("invalid being id")
        being = self._get_being(being_id)

        inj = args.get("injury")
        if not isinstance(inj, dict) or "id" not in inj:
            raise ValueError("invalid injury")

        iid = inj["id"]
        if not isinstance(iid, str) or not iid:
            raise ValueError("invalid injury id")

        if any(x["id"] == iid for x in being["injuries"]):
            raise ValueError("injury id already exists")

        if len(being["injuries"]) >= MAX_INJURIES:
            raise ValueError("too many injuries")

        sev = int(inj.get("severity", 10))
        if not (0 <= sev <= 100):
            raise ValueError("severity out of bounds")

        clean_inj = {
            "id": iid,
            "label": inj.get("label", iid),
            "severity": sev,
            "location": inj.get("location", "general"),
            "effects": inj.get("effects", {}),
        }
        being["injuries"].append(clean_inj)

        # Direct health reduction from injury severity
        being["health"] = max(0, being["health"] - sev)
        if being["health"] == 0 and "dead" not in being["conditions"]:
            being["conditions"].append("dead")

        return {"ok": True, "being": being_id, "injuries": being["injuries"], "health": being["health"]}

    def op_heal_injury(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being_id = args.get("being") or args.get("id")
        if not being_id or not isinstance(being_id, str):
            raise ValueError("invalid being id")
        being = self._get_being(being_id)

        iid = args.get("injury_id") or args.get("id")
        if not iid or not isinstance(iid, str):
            raise ValueError("invalid injury id")

        target = None
        for x in being["injuries"]:
            if x["id"] == iid:
                target = x
                break

        if not target:
            raise ValueError(f"injury {iid} not found")

        amount = int(args.get("amount", target["severity"]))
        target["severity"] = max(0, target["severity"] - amount)
        if target["severity"] == 0:
            being["injuries"] = [x for x in being["injuries"] if x["id"] != iid]

        heal_hp = int(args.get("heal_health", amount))
        being["health"] = min(100, being["health"] + heal_hp)

        return {"ok": True, "being": being_id, "injuries": being["injuries"], "health": being["health"]}

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

            if op == "init":
                results.append(self.op_init(args))
            elif op == "assess":
                results.append(self.op_assess(args))
            elif op == "tick":
                results.append(self.op_tick(args))
            elif op == "satisfy":
                results.append(self.op_satisfy(args))
            elif op == "rest":
                results.append(self.op_rest(args))
            elif op == "apply_action":
                results.append(self.op_apply_action(args))
            elif op == "set_condition":
                results.append(self.op_set_condition(args))
            elif op == "remove_condition":
                results.append(self.op_remove_condition(args))
            elif op == "add_injury":
                results.append(self.op_add_injury(args))
            elif op == "heal_injury":
                results.append(self.op_heal_injury(args))
            else:
                raise ValueError(f"unknown op: {op}")

        return {"version": 1, "results": results}
