"""Independent original reference for life projects (pass 4, stage 5). Python standard library
only; nothing here calls a model.

A project has an owner, an aim, a stake (how much of the owner's identity rides on it, 0-100), a
horizon (a deadline tick) and ordered milestones, each with a condition the host can check (a task
outcome, a fact, an item or a relationship level), an expected duration and an optional
alternative. Morale starts at 700,000 millionths. Progress comes only from confirmed outcomes: a
success completes the current milestone (+100,000 morale); a failure (-200,000) or a setback
(severity x 3,000) lowers morale and makes the owner decide, by a fixed rule over

    score = morale / 10,000 + stake / 2 + sunk

where sunk = min(30, time / 50 + money / 100) of what was invested, counted only while there have
been fewer than four failures in a row (sunk cost is honest but bounded): 80 or more persist; 55
or more adapt (swap the current milestone for its unused alternative) or else persist; 35 or more
pause; below 35 abandon. Completion and abandonment name what they feed (drift, lesson,
aspiration; an abandoned project suggests a new aim can grow from it).

For scheduling, `weight` gives an active project's motive score (stake scaled by urgency: how much
expected work remains against the time left before the horizon) and the task slot for its current
milestone. `offscreen` resolves the current milestone coarsely from elapsed time and host luck.
"""
import pathlib
import sys
from typing import Any, Dict, List, Optional

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_uint, text, uint
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
FULL = 1_000_000
START = 700_000
SPEC = [("projects", 256, 1_000_000), ("milestones", 8, 32)]
CONDITIONS = ["task", "fact", "item", "relationship"]
FEEDS = ["drift", "lesson", "aspiration"]


def condition(v: Any) -> dict:
    keys(v, ["kind", "ref"], ["level"])
    need(isinstance(v["kind"], str) and v["kind"] in CONDITIONS)
    c = {"kind": v["kind"], "ref": ident(v["ref"])}
    if v["kind"] == "relationship":
        c["level"] = uint(v.get("level"), 0, 100)
    else:
        need(v.get("level") is None)
    return c


def step(v: Any) -> dict:
    keys(v, ["id", "condition", "expected"], [])
    return {"id": ident(v["id"]), "condition": condition(v["condition"]), "expected": uint(v["expected"], 1, MAX_AT)}


class Projects:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.projects: Dict[str, dict] = {}
        self.clock = 0

    def get(self, pid: str) -> dict:
        need(pid in self.projects)
        return self.projects[pid]

    def sunk(self, p: dict) -> int:
        if p["failures"] >= 4:
            return 0
        return min(30, p["time"] // 50 + p["money"] // 100)

    def score(self, p: dict) -> int:
        return p["morale"] // 10_000 + p["stake"] // 2 + self.sunk(p)

    def current(self, p: dict) -> Optional[dict]:
        return p["milestones"][p["at"]] if p["at"] < len(p["milestones"]) else None

    def view(self, pid: str, p: dict) -> dict:
        cur = self.current(p)
        return {"adapted": p["adapted"], "aim": p["aim"], "current": None if cur is None else cur["id"],
                "done": [m["id"] for m in p["milestones"][:p["at"]]], "failures": p["failures"],
                "horizon": p["horizon"], "id": pid, "money": p["money"], "morale": p["morale"], "owner": p["owner"],
                "remaining": [m["id"] for m in p["milestones"][p["at"]:]], "score": self.score(p),
                "stake": p["stake"], "status": p["status"], "sunk": self.sunk(p), "time": p["time"]}

    def decide(self, pid: str, p: dict) -> dict:
        """The owner's choice after a failure or setback."""
        s = self.score(p)
        cur = self.current(p)
        if s >= 80:
            d = "persist"
        elif s >= 55:
            d = "adapt" if cur is not None and cur.get("alternative") is not None else "persist"
        elif s >= 35:
            d = "pause"
        else:
            d = "abandon"
        out = {"decision": d, "feeds": [], "grows_from": None, "replaced": None}
        if d == "adapt":
            alt = cur.pop("alternative")
            out["replaced"] = cur["id"]
            p["milestones"][p["at"]] = alt
            p["adapted"] += 1
        elif d == "pause":
            p["status"] = "paused"
        elif d == "abandon":
            p["status"] = "abandoned"
            out["feeds"] = list(FEEDS)
            out["grows_from"] = {"aim": p["aim"], "project": pid}
        return out

    def op_tick(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        self.clock = now
        return {"now": now}

    def op_propose(self, a: dict) -> dict:
        keys(a, ["id", "owner", "aim", "stake", "horizon", "milestones"], [])
        pid, owner = ident(a["id"]), ident(a["owner"])
        aim = text(a["aim"], 80)
        stake = uint(a["stake"], 0, 100)
        horizon = uint(a["horizon"], self.clock, MAX_AT)
        rows = a["milestones"]
        need(isinstance(rows, list) and 1 <= len(rows) <= self.lim["milestones"])
        ms = []
        for r in rows:
            need(isinstance(r, dict))
            alt = r.get("alternative")
            m = step({k: v for k, v in r.items() if k != "alternative"})
            if alt is not None:
                m["alternative"] = step(alt)
            ms.append(m)
        ids = [m["id"] for m in ms] + [m["alternative"]["id"] for m in ms if "alternative" in m]
        need(len(set(ids)) == len(ids))
        need(pid not in self.projects and len(self.projects) < self.lim["projects"])
        self.projects[pid] = {"owner": owner, "aim": aim, "stake": stake, "horizon": horizon, "milestones": ms,
                              "at": 0, "morale": START, "time": 0, "money": 0, "failures": 0, "adapted": 0,
                              "status": "active"}
        return self.view(pid, self.projects[pid])

    def op_show(self, a: dict) -> dict:
        keys(a, ["project"], [])
        pid = ident(a["project"])
        return self.view(pid, self.get(pid))

    def op_invest(self, a: dict) -> dict:
        keys(a, ["project"], ["time", "money"])
        pid = ident(a["project"])
        t = opt_uint(a, "time", 0, 1_000_000, 0)
        m = opt_uint(a, "money", 0, 1_000_000_000, 0)
        p = self.get(pid)
        need(p["status"] in ("active", "paused"))
        p["time"] = min(MAX_AT, p["time"] + t)
        p["money"] = min(MAX_AT, p["money"] + m)
        return self.view(pid, p)

    def op_outcome(self, a: dict) -> dict:
        keys(a, ["project", "milestone", "success"], [])
        pid, mid = ident(a["project"]), ident(a["milestone"])
        need(isinstance(a["success"], bool))
        p = self.get(pid)
        need(p["status"] == "active")
        cur = self.current(p)
        need(cur is not None and cur["id"] == mid)
        if a["success"]:
            p["morale"] = min(FULL, p["morale"] + 100_000)
            p["failures"] = 0
            p["at"] += 1
            done = p["at"] == len(p["milestones"])
            if done:
                p["status"] = "completed"
            out = {"decision": "complete" if done else None, "feeds": list(FEEDS) if done else [],
                   "grows_from": None, "replaced": None}
        else:
            p["morale"] = max(0, p["morale"] - 200_000)
            p["failures"] += 1
            out = self.decide(pid, p)
        out["project"] = self.view(pid, p)
        return out

    def op_setback(self, a: dict) -> dict:
        keys(a, ["project", "severity"], [])
        pid = ident(a["project"])
        sev = uint(a["severity"], 1, 100)
        p = self.get(pid)
        need(p["status"] == "active")
        p["morale"] = max(0, p["morale"] - sev * 3_000)
        out = self.decide(pid, p)
        out["project"] = self.view(pid, p)
        return out

    def op_encourage(self, a: dict) -> dict:
        keys(a, ["project", "amount"], [])
        pid = ident(a["project"])
        amount = uint(a["amount"], 0, 100)
        p = self.get(pid)
        need(p["status"] in ("active", "paused"))
        p["morale"] = min(FULL, p["morale"] + amount * 5_000)
        return self.view(pid, p)

    def op_resume(self, a: dict) -> dict:
        keys(a, ["project"], [])
        pid = ident(a["project"])
        p = self.get(pid)
        need(p["status"] == "paused")
        p["status"] = "active"
        return self.view(pid, p)

    def op_weight(self, a: dict) -> dict:
        keys(a, ["project"], [])
        pid = ident(a["project"])
        p = self.get(pid)
        cur = self.current(p)
        if p["status"] != "active" or cur is None:
            return {"motive": None, "project": pid, "slot": None, "urgency": 0}
        need_ticks = sum(m["expected"] for m in p["milestones"][p["at"]:])
        left = p["horizon"] - self.clock if p["horizon"] > self.clock else 0
        urgency = 100 if need_ticks >= left else need_ticks * 100 // left
        score = p["stake"] * (50 + urgency // 2) // 100
        return {"motive": {"id": "project:" + pid, "score": score}, "project": pid,
                "slot": {"condition": dict(cur["condition"]), "expected": cur["expected"], "milestone": cur["id"],
                         "owner": p["owner"]},
                "urgency": urgency}

    def op_offscreen(self, a: dict) -> dict:
        keys(a, ["project", "elapsed", "luck"], [])
        pid = ident(a["project"])
        elapsed = uint(a["elapsed"], 0, MAX_AT)
        luck = uint(a["luck"], 0, 100)
        p = self.get(pid)
        cur = self.current(p)
        if p["status"] != "active" or cur is None:
            return {"project": pid, "resolved": None, "result": None}
        if luck < 20:
            success = False
        elif luck >= 50 and elapsed >= cur["expected"]:
            success = True
        else:
            return {"project": pid, "resolved": None, "result": None}
        r = self.op_outcome({"project": pid, "milestone": cur["id"], "success": success})
        return {"project": pid, "resolved": "success" if success else "failure", "result": r}

    def workflow(self, request: Any) -> dict:
        table = {"tick": self.op_tick, "propose": self.op_propose, "show": self.op_show, "invest": self.op_invest,
                 "outcome": self.op_outcome, "setback": self.op_setback, "encourage": self.op_encourage,
                 "resume": self.op_resume, "weight": self.op_weight, "offscreen": self.op_offscreen}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Projects().workflow(request)
