#!/usr/bin/env python3
"""Task and Planning reference implementation (Gap 4).

Features:
- Task object schema: id, owner, assigned_by/for, motive_ref, steps, status,
  progress, deadline, priority, reserved_resources, bounded on_fail/on_block,
  replan_count, retry_count, and history of confirmed outcomes.
- Deterministic Planner: graph search / prerequisite chaining over ActionCatalog
  for item acquisition, recipe crafting, location travel, and need satisfaction.
- Host Validator for Model Proposals & Replans: checks action validity against
  ActionCatalog, target bindings, causal chains, and bounds.
- Confirmed Outcome Step Advancement: only host-confirmed outcomes advance steps.
- Bounded depth (<= 8), step count (<= 16), and replans (<= 3).
"""

from typing import Any, Dict, List, Optional
import copy

from limits_reference import Limits, limited

MAX_TASKS = 64
MAX_STEPS = 16
MAX_PLAN_DEPTH = 8
MAX_REPLANS = 3
MAX_RETRIES = 3
MAX_RESERVED_RESOURCES = 16
MAX_HISTORY_ENTRIES = 32
MAX_OPS = 64

VALID_STATUSES = {
    "proposed",
    "accepted",
    "active",
    "blocked",
    "suspended",
    "done",
    "failed",
    "abandoned",
}

VALID_POLICIES = {"replan", "retry", "ask_help", "suspend", "abandon"}

# Recognized Action Catalog verbs
KNOWN_ACTIONS = {
    "activity", "add_member", "animal_train", "approach", "ask", "barter", "behave",
    "build", "calm", "can_carry", "check_relation", "cook", "craft", "create",
    "debt_query", "decline", "destroy", "dismantle", "enter", "feed", "fitness_train",
    "followup", "gesture", "gift", "harvest", "help", "lead", "load", "loan", "look",
    "move", "pet", "prop_interact", "remove_member", "repair", "repay", "ride",
    "service", "set_customs", "set_relation", "silence", "store", "take", "talk",
    "transfer", "travel", "travel_cost", "unload", "wait",
}

REQUIRED_SLOTS = {
    "travel": ["destination"],
    "cook": ["recipe"],
    "craft": ["recipe"],
    "barter": ["partner", "offer", "want"],
    "take": ["item"],
    "store": ["item"],
    "talk": ["listener"],
    "ask": ["listener", "topic"],
    "help": ["target"],
    "service": ["service", "target"],
}


def clamp(val: int, lo: int, hi: int) -> int:
    return max(lo, min(hi, val))


class TaskReference:
    def __init__(self):
        self.tasks: Dict[str, Dict[str, Any]] = {}
        self.limits = Limits([("tasks", MAX_TASKS, 1_000_000), ("steps", MAX_STEPS, 256), ("reserved", MAX_RESERVED_RESOURCES, 256), ("history", MAX_HISTORY_ENTRIES, 1024)])

    def workflow(self, args: Dict[str, Any]) -> Dict[str, Any]:
        return limited(args, self.limits, self.run_inner)

    def run_inner(self, args: Dict[str, Any]) -> Dict[str, Any]:
        if not isinstance(args, dict):
            raise ValueError("args must be an object")
        ops = args.get("ops")
        if not isinstance(ops, list) or len(ops) == 0:
            raise ValueError("ops must be a non-empty list")
        if len(ops) > MAX_OPS:
            raise ValueError(f"ops limit exceeded: {len(ops)} > {MAX_OPS}")

        results = []
        for op_spec in ops:
            if not isinstance(op_spec, dict):
                raise ValueError("op spec must be an object")
            op_name = op_spec.get("op")
            op_args = op_spec.get("args", {})
            if not isinstance(op_name, str) or not isinstance(op_args, dict):
                raise ValueError("op must have string 'op' and dict 'args'")

            if op_name == "propose":
                res = self.op_propose(op_args)
            elif op_name == "accept":
                res = self.op_accept(op_args)
            elif op_name == "advance":
                res = self.op_advance(op_args)
            elif op_name == "block":
                res = self.op_block(op_args)
            elif op_name == "replan":
                res = self.op_replan(op_args)
            elif op_name == "suspend":
                res = self.op_suspend(op_args)
            elif op_name == "resume":
                res = self.op_resume(op_args)
            elif op_name == "abandon":
                res = self.op_abandon(op_args)
            elif op_name == "get":
                res = self.op_get(op_args)
            elif op_name == "list":
                res = self.op_list(op_args)
            elif op_name == "validate":
                res = self.op_validate(op_args)
            else:
                raise ValueError(f"unknown task op: {op_name}")
            results.append(res)

        return {"results": results, "total_tasks": len(self.tasks)}

    def op_propose(self, args: Dict[str, Any]) -> Dict[str, Any]:
        tid = args.get("id")
        if not tid or not isinstance(tid, str) or len(tid) > 64:
            raise ValueError("invalid task id")
        owner = args.get("owner")
        if not owner or not isinstance(owner, str) or len(owner) > 64:
            raise ValueError("invalid task owner")

        if len(self.tasks) >= self.limits["tasks"] and tid not in self.tasks:
            raise ValueError("max tasks limit reached")

        priority = clamp(int(args.get("priority", 50)), 0, 100)
        on_fail = args.get("on_fail", "replan")
        if on_fail not in VALID_POLICIES:
            raise ValueError(f"unknown on_fail policy: {on_fail}")
        on_block = args.get("on_block", "replan")
        if on_block not in VALID_POLICIES:
            raise ValueError(f"unknown on_block policy: {on_block}")

        assigned_by = args.get("assigned_by")
        assigned_for = args.get("for")
        motive_ref = args.get("motive_ref")
        deadline = args.get("deadline")
        if deadline is not None:
            deadline = max(0, int(deadline))

        reserved = args.get("reserved_resources", [])
        if not isinstance(reserved, list) or len(reserved) > self.limits["reserved"]:
            raise ValueError("invalid reserved_resources")

        # Either proposed steps are provided, or goal is given to deterministic planner
        steps_raw = args.get("steps")
        goal = args.get("goal")

        if steps_raw is not None:
            val = self.validate_steps(steps_raw, owner, args.get("world_state", {}))
            if not val["valid"]:
                return {
                    "accepted": False,
                    "task_id": tid,
                    "reason": val["reason"],
                    "errors": val["errors"],
                }
            steps = val["steps"]
        elif goal is not None:
            plan_res = self.plan_deterministic(owner, goal, args.get("world_state", {}))
            if not plan_res["success"]:
                return {
                    "accepted": False,
                    "task_id": tid,
                    "reason": plan_res["reason"],
                    "errors": [plan_res["reason"]],
                }
            steps = plan_res["steps"]
        else:
            raise ValueError("either 'steps' or 'goal' must be provided to propose")

        task = {
            "id": tid,
            "owner": owner,
            "assigned_by": assigned_by,
            "for": assigned_for,
            "motive_ref": motive_ref,
            "steps": steps,
            "status": "proposed",
            "blocked_reason": None,
            "progress": 0,
            "deadline": deadline,
            "priority": priority,
            "reserved_resources": reserved,
            "on_fail": on_fail,
            "on_block": on_block,
            "replan_count": 0,
            "retry_count": 0,
            "history": [],
        }

        self.tasks[tid] = task
        return {"accepted": True, "task_id": tid, "task": copy.deepcopy(task)}

    def op_accept(self, args: Dict[str, Any]) -> Dict[str, Any]:
        tid = args.get("id")
        task = self.tasks.get(tid)
        if not task:
            raise ValueError(f"task not found: {tid}")

        if task["status"] not in ("proposed", "suspended"):
            raise ValueError(f"cannot accept task in status '{task['status']}'")

        start = bool(args.get("start", True))
        if start:
            task["status"] = "active"
            if len(task["steps"]) > 0:
                task["steps"][0]["status"] = "active"
        else:
            task["status"] = "accepted"

        return {"accepted": True, "task_id": tid, "status": task["status"], "task": copy.deepcopy(task)}

    def op_advance(self, args: Dict[str, Any]) -> Dict[str, Any]:
        tid = args.get("id")
        task = self.tasks.get(tid)
        if not task:
            raise ValueError(f"task not found: {tid}")

        if task["status"] != "active":
            raise ValueError(f"cannot advance task in non-active status '{task['status']}'")

        outcome = args.get("outcome")
        if not isinstance(outcome, dict) or "success" not in outcome:
            raise ValueError("outcome must be an object with boolean 'success'")

        prog = task["progress"]
        if prog >= len(task["steps"]):
            task["status"] = "done"
            return {"advanced": True, "task_id": tid, "task_status": "done", "next_step": None, "task": copy.deepcopy(task)}

        cur_step = task["steps"][prog]
        tick = args.get("tick")

        hist_entry = {
            "step_id": cur_step["id"],
            "action": cur_step["action"],
            "success": bool(outcome["success"]),
            "outcome": copy.deepcopy(outcome),
            "tick": tick,
        }
        if len(task["history"]) < self.limits["history"]:
            task["history"].append(hist_entry)

        if outcome["success"]:
            cur_step["status"] = "done"
            task["retry_count"] = 0
            task["progress"] += 1
            if task["progress"] >= len(task["steps"]):
                task["status"] = "done"
                return {
                    "advanced": True,
                    "task_id": tid,
                    "task_status": "done",
                    "next_step": None,
                    "task": copy.deepcopy(task),
                }
            else:
                next_step = task["steps"][task["progress"]]
                next_step["status"] = "active"
                return {
                    "advanced": True,
                    "task_id": tid,
                    "task_status": "active",
                    "next_step": next_step["id"],
                    "task": copy.deepcopy(task),
                }
        else:
            cur_step["status"] = "failed"
            policy = task["on_fail"]
            if policy == "retry" and task["retry_count"] < MAX_RETRIES:
                task["retry_count"] += 1
                cur_step["status"] = "active"
                return {
                    "advanced": False,
                    "task_id": tid,
                    "action": "retrying",
                    "task_status": "active",
                    "retry_count": task["retry_count"],
                    "task": copy.deepcopy(task),
                }
            elif policy == "replan" and task["replan_count"] < MAX_REPLANS:
                task["status"] = "blocked"
                task["blocked_reason"] = outcome.get("reason", "step_failed")
                return {
                    "advanced": False,
                    "task_id": tid,
                    "action": "blocked_for_replan",
                    "task_status": "blocked",
                    "task": copy.deepcopy(task),
                }
            elif policy == "suspend":
                task["status"] = "suspended"
                return {
                    "advanced": False,
                    "task_id": tid,
                    "action": "suspended",
                    "task_status": "suspended",
                    "task": copy.deepcopy(task),
                }
            else:
                task["status"] = "failed"
                return {
                    "advanced": False,
                    "task_id": tid,
                    "action": "failed",
                    "task_status": "failed",
                    "task": copy.deepcopy(task),
                }

    def op_block(self, args: Dict[str, Any]) -> Dict[str, Any]:
        tid = args.get("id")
        task = self.tasks.get(tid)
        if not task:
            raise ValueError(f"task not found: {tid}")

        reason = args.get("reason", "unspecified_block")
        task["status"] = "blocked"
        task["blocked_reason"] = reason
        return {"blocked": True, "task_id": tid, "task_status": "blocked", "reason": reason, "task": copy.deepcopy(task)}

    def op_replan(self, args: Dict[str, Any]) -> Dict[str, Any]:
        tid = args.get("id")
        task = self.tasks.get(tid)
        if not task:
            raise ValueError(f"task not found: {tid}")

        if task["replan_count"] >= MAX_REPLANS:
            task["status"] = "failed"
            return {
                "replanned": False,
                "task_id": tid,
                "reason": "replan_limit_exceeded",
                "task_status": "failed",
                "task": copy.deepcopy(task),
            }

        steps_raw = args.get("steps")
        goal = args.get("goal")

        if steps_raw is not None:
            val = self.validate_steps(steps_raw, task["owner"], args.get("world_state", {}))
            if not val["valid"]:
                return {
                    "replanned": False,
                    "task_id": tid,
                    "reason": val["reason"],
                    "errors": val["errors"],
                    "task": copy.deepcopy(task),
                }
            new_steps = val["steps"]
        elif goal is not None:
            plan_res = self.plan_deterministic(task["owner"], goal, args.get("world_state", {}))
            if not plan_res["success"]:
                return {
                    "replanned": False,
                    "task_id": tid,
                    "reason": plan_res["reason"],
                    "errors": [plan_res["reason"]],
                    "task": copy.deepcopy(task),
                }
            new_steps = plan_res["steps"]
        else:
            raise ValueError("either 'steps' or 'goal' required for replan")

        # Keep completed steps, replace remainder
        completed_steps = [s for s in task["steps"] if s["status"] == "done"]
        if len(completed_steps) + len(new_steps) > self.limits["steps"]:
            raise ValueError("replanned steps exceed max step limit")

        if len(new_steps) > 0:
            new_steps[0]["status"] = "active"

        task["steps"] = completed_steps + new_steps
        task["progress"] = len(completed_steps)
        task["status"] = "active"
        task["blocked_reason"] = None
        task["replan_count"] += 1
        task["retry_count"] = 0

        return {
            "replanned": True,
            "task_id": tid,
            "replan_count": task["replan_count"],
            "task_status": "active",
            "task": copy.deepcopy(task),
        }

    def op_suspend(self, args: Dict[str, Any]) -> Dict[str, Any]:
        tid = args.get("id")
        task = self.tasks.get(tid)
        if not task:
            raise ValueError(f"task not found: {tid}")
        task["status"] = "suspended"
        return {"suspended": True, "task_id": tid, "task_status": "suspended", "task": copy.deepcopy(task)}

    def op_resume(self, args: Dict[str, Any]) -> Dict[str, Any]:
        tid = args.get("id")
        task = self.tasks.get(tid)
        if not task:
            raise ValueError(f"task not found: {tid}")
        task["status"] = "active"
        return {"resumed": True, "task_id": tid, "task_status": "active", "task": copy.deepcopy(task)}

    def op_abandon(self, args: Dict[str, Any]) -> Dict[str, Any]:
        tid = args.get("id")
        task = self.tasks.get(tid)
        if not task:
            raise ValueError(f"task not found: {tid}")
        task["status"] = "abandoned"
        task["blocked_reason"] = args.get("reason", "abandoned_by_actor")
        return {"abandoned": True, "task_id": tid, "task_status": "abandoned", "task": copy.deepcopy(task)}

    def op_get(self, args: Dict[str, Any]) -> Dict[str, Any]:
        tid = args.get("id")
        task = self.tasks.get(tid)
        if not task:
            raise ValueError(f"task not found: {tid}")
        return {"task": copy.deepcopy(task)}

    def op_list(self, args: Dict[str, Any]) -> Dict[str, Any]:
        owner = args.get("owner")
        status = args.get("status")
        matched = []
        for tid in sorted(self.tasks.keys()):
            t = self.tasks[tid]
            if owner and t["owner"] != owner:
                continue
            if status and t["status"] != status:
                continue
            matched.append(copy.deepcopy(t))
        return {"tasks": matched}

    def op_validate(self, args: Dict[str, Any]) -> Dict[str, Any]:
        steps = args.get("steps", [])
        owner = args.get("owner", "unknown")
        world_state = args.get("world_state", {})
        val = self.validate_steps(steps, owner, world_state)
        return {
            "valid": val["valid"],
            "reason": val.get("reason"),
            "errors": val.get("errors", []),
            "step_count": len(steps) if isinstance(steps, list) else 0,
        }

    def validate_steps(self, steps_raw: Any, owner: str, world_state: Dict[str, Any]) -> Dict[str, Any]:
        if not isinstance(steps_raw, list):
            return {"valid": False, "reason": "steps_not_a_list", "errors": ["steps must be an array"]}
        if len(steps_raw) == 0:
            return {"valid": False, "reason": "empty_steps", "errors": ["plan cannot be empty"]}
        if len(steps_raw) > self.limits["steps"]:
            return {"valid": False, "reason": "too_many_steps", "errors": [f"step count {len(steps_raw)} > {self.limits['steps']}"]}

        errors = []
        cleaned_steps = []
        step_ids = set()

        # Simulated state tracking for causal chain verification
        sim_location = world_state.get("location")
        sim_inventory = dict(world_state.get("inventory", {}))

        for idx, s in enumerate(steps_raw):
            if not isinstance(s, dict):
                errors.append(f"step[{idx}] must be an object")
                continue

            sid = s.get("id", f"step_{idx + 1}")
            if not isinstance(sid, str) or len(sid) == 0 or len(sid) > 64:
                errors.append(f"step[{idx}] invalid step id: {sid}")
            if sid in step_ids:
                errors.append(f"step[{idx}] duplicate step id: {sid}")
            step_ids.add(sid)

            action = s.get("action")
            if not action or not isinstance(action, str) or action not in KNOWN_ACTIONS:
                errors.append(f"step[{idx}] unknown action catalog verb: {action}")
                continue

            bindings = s.get("target_bindings", {})
            if not isinstance(bindings, dict):
                errors.append(f"step[{idx}] target_bindings must be an object")
                continue

            # Check required target slots
            req_slots = REQUIRED_SLOTS.get(action, [])
            for slot in req_slots:
                if slot not in bindings:
                    errors.append(f"step[{idx}] action '{action}' missing required target slot '{slot}'")

            # Causal check: travel updates simulated location
            if action == "travel":
                dest = bindings.get("destination")
                if dest:
                    sim_location = dest
            elif action in ("cook", "craft"):
                # Requires recipe bindings
                recipe = bindings.get("recipe")
                recipes = world_state.get("recipes", {})
                if recipe and recipe in recipes:
                    rinfo = recipes[recipe]
                    req_station = rinfo.get("station")
                    if req_station and sim_location and req_station != sim_location:
                        errors.append(f"step[{idx}] action '{action}' requires station at '{req_station}', but actor is at '{sim_location}'")
                    # Check ingredients
                    for inp in rinfo.get("inputs", []):
                        item = inp.get("item")
                        cnt = inp.get("count", 1)
                        if sim_inventory.get(item, 0) < cnt:
                            errors.append(f"step[{idx}] action '{action}' missing input ingredient '{item}' ({sim_inventory.get(item, 0)} < {cnt})")
                        else:
                            sim_inventory[item] -= cnt
                    # Produce outputs
                    for out in rinfo.get("outputs", []):
                        item = out.get("item")
                        cnt = out.get("count", 1)
                        sim_inventory[item] = sim_inventory.get(item, 0) + cnt
            elif action == "barter":
                want = bindings.get("want")
                offer = bindings.get("offer")
                partner = bindings.get("partner")
                if offer and sim_inventory.get(offer, 0) <= 0:
                    errors.append(f"step[{idx}] action 'barter' cannot offer '{offer}', actor has none")
                else:
                    if offer and offer in sim_inventory:
                        sim_inventory[offer] -= 1
                    if want:
                        sim_inventory[want] = sim_inventory.get(want, 0) + 1

            preconds = s.get("preconditions")
            expected_effects = s.get("expected_effects")
            est_dur = max(0, int(s.get("estimated_duration", 1)))
            est_fatigue = max(0, int(s.get("estimated_fatigue", 1)))

            cleaned_steps.append({
                "id": sid,
                "action": action,
                "target_bindings": copy.deepcopy(bindings),
                "preconditions": copy.deepcopy(preconds) if preconds is not None else None,
                "expected_effects": copy.deepcopy(expected_effects) if expected_effects is not None else None,
                "status": "pending",
                "estimated_duration": est_dur,
                "estimated_fatigue": est_fatigue,
            })

        if errors:
            return {"valid": False, "reason": "step_validation_failed", "errors": errors}

        return {"valid": True, "steps": cleaned_steps, "errors": []}

    def plan_deterministic(self, owner: str, goal: Dict[str, Any], world_state: Dict[str, Any]) -> Dict[str, Any]:
        if not isinstance(goal, dict):
            return {"success": False, "reason": "invalid_goal_object"}

        gkind = goal.get("kind")
        cur_location = world_state.get("location", "home")
        inventory = dict(world_state.get("inventory", {}))
        recipes = world_state.get("recipes", {})
        sources = world_state.get("sources", [])

        steps = []
        step_num = 1

        if gkind in ("cook", "craft"):
            recipe_name = goal.get("recipe")
            if not recipe_name or recipe_name not in recipes:
                return {"success": False, "reason": f"unknown_recipe:{recipe_name}"}

            rinfo = recipes[recipe_name]
            station = rinfo.get("station")
            inputs = rinfo.get("inputs", [])

            # Check ingredients
            missing_items = []
            for inp in inputs:
                item = inp.get("item")
                needed = inp.get("count", 1)
                available = inventory.get(item, 0)
                if available < needed:
                    missing_items.append((item, needed - available))

            # Acquire missing items
            for item, deficit in missing_items:
                source = None
                for s in sources:
                    if s.get("item") == item:
                        source = s
                        break
                if not source:
                    return {"success": False, "reason": f"no_source_for:{item}"}

                source_place = source.get("place")
                if source_place and cur_location != source_place:
                    steps.append({
                        "id": f"step_{step_num}",
                        "action": "travel",
                        "target_bindings": {"destination": source_place},
                        "preconditions": None,
                        "expected_effects": None,
                        "status": "pending",
                        "estimated_duration": 5,
                        "estimated_fatigue": 2,
                    })
                    step_num += 1
                    cur_location = source_place

                partner = source.get("partner", "merchant")
                cost_item = source.get("cost_item", "coins")
                steps.append({
                    "id": f"step_{step_num}",
                    "action": "barter",
                    "target_bindings": {
                        "partner": partner,
                        "offer": cost_item,
                        "want": item,
                    },
                    "preconditions": None,
                    "expected_effects": None,
                    "status": "pending",
                    "estimated_duration": 2,
                    "estimated_fatigue": 1,
                })
                step_num += 1
                inventory[item] = inventory.get(item, 0) + deficit

            # Travel to station if required
            if station and cur_location != station:
                steps.append({
                    "id": f"step_{step_num}",
                    "action": "travel",
                    "target_bindings": {"destination": station},
                    "preconditions": None,
                    "expected_effects": None,
                    "status": "pending",
                    "estimated_duration": 5,
                    "estimated_fatigue": 2,
                })
                step_num += 1
                cur_location = station

            # Craft/cook step
            steps.append({
                "id": f"step_{step_num}",
                "action": gkind,
                "target_bindings": {"recipe": recipe_name},
                "preconditions": None,
                "expected_effects": None,
                "status": "pending",
                "estimated_duration": 10,
                "estimated_fatigue": 3,
            })

            return {"success": True, "steps": steps}

        elif gkind == "travel":
            dest = goal.get("destination")
            if not dest:
                return {"success": False, "reason": "missing_destination"}
            if cur_location != dest:
                steps.append({
                    "id": f"step_{step_num}",
                    "action": "travel",
                    "target_bindings": {"destination": dest},
                    "preconditions": None,
                    "expected_effects": None,
                    "status": "pending",
                    "estimated_duration": 5,
                    "estimated_fatigue": 2,
                })
            return {"success": True, "steps": steps}

        elif gkind == "satisfy_need":
            need = goal.get("need")
            if need == "hunger":
                # Eat available food, or obtain food then eat
                food_item = goal.get("item", "bread")
                if inventory.get(food_item, 0) > 0:
                    steps.append({
                        "id": f"step_{step_num}",
                        "action": "activity",
                        "target_bindings": {"name": f"eat_{food_item}"},
                        "preconditions": None,
                        "expected_effects": None,
                        "status": "pending",
                        "estimated_duration": 2,
                        "estimated_fatigue": 0,
                    })
                else:
                    # Take food from pantry or source
                    pantry = goal.get("pantry", "pantry")
                    steps.append({
                        "id": f"step_{step_num}",
                        "action": "take",
                        "target_bindings": {"item": food_item, "container": pantry},
                        "preconditions": None,
                        "expected_effects": None,
                        "status": "pending",
                        "estimated_duration": 1,
                        "estimated_fatigue": 0,
                    })
                    step_num += 1
                    steps.append({
                        "id": f"step_{step_num}",
                        "action": "activity",
                        "target_bindings": {"name": f"eat_{food_item}"},
                        "preconditions": None,
                        "expected_effects": None,
                        "status": "pending",
                        "estimated_duration": 2,
                        "estimated_fatigue": 0,
                    })
                return {"success": True, "steps": steps}
            elif need == "rest":
                home = goal.get("place", "home")
                if cur_location != home:
                    steps.append({
                        "id": f"step_{step_num}",
                        "action": "travel",
                        "target_bindings": {"destination": home},
                        "preconditions": None,
                        "expected_effects": None,
                        "status": "pending",
                        "estimated_duration": 5,
                        "estimated_fatigue": 1,
                    })
                    step_num += 1
                steps.append({
                    "id": f"step_{step_num}",
                    "action": "wait",
                    "target_bindings": {"duration": "long"},
                    "preconditions": None,
                    "expected_effects": None,
                    "status": "pending",
                    "estimated_duration": 30,
                    "estimated_fatigue": 0,
                })
                return {"success": True, "steps": steps}
            else:
                return {"success": False, "reason": f"unhandled_need:{need}"}

        elif gkind == "barter":
            partner = goal.get("partner")
            offer = goal.get("offer")
            want = goal.get("want")
            place = goal.get("place")
            if not partner or not offer or not want:
                return {"success": False, "reason": "missing_barter_parameters"}
            if place and cur_location != place:
                steps.append({
                    "id": f"step_{step_num}",
                    "action": "travel",
                    "target_bindings": {"destination": place},
                    "preconditions": None,
                    "expected_effects": None,
                    "status": "pending",
                    "estimated_duration": 5,
                    "estimated_fatigue": 2,
                })
                step_num += 1
            steps.append({
                "id": f"step_{step_num}",
                "action": "barter",
                "target_bindings": {"partner": partner, "offer": offer, "want": want},
                "preconditions": None,
                "expected_effects": None,
                "status": "pending",
                "estimated_duration": 2,
                "estimated_fatigue": 1,
            })
            return {"success": True, "steps": steps}

        else:
            return {"success": False, "reason": f"unsupported_goal_kind:{gkind}"}
