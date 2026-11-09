"""Independent original reference for status, roles and jobs (pass 2, B7).
Python standard library only; nothing here calls a model.

A role (guard, baker, elder) has a status 0..100, optional seats, and duties that recur every
`every` ticks from tick `at`. Beings are assigned roles. `tick` proposes one task per duty that
came due since the holder last ran it (reporting how many earlier times were `missed` during a
gap) and lists roles with fewer holders than seats. A being's status is its highest role status
plus earned standing (`esteem`, -50..50), clamped to 0..100. Status feeds conflicts (`contest`:
an edge in signed millionths and who defers) and persuasion (`persuade`: a chance adjusted by the
status edge and a shared role). Tasks are proposals for the task engine.
"""
import pathlib
import sys
from typing import Any, Dict, List, Optional

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, uint
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
M = 1_000_000
SPEC = [("roles", 64, 4096), ("beings", 1024, 1_000_000), ("duties", 8, 64), ("held", 4, 16)]
DEFER_AT = 20
EDGE_CONTEST = 10_000
EDGE_PERSUADE = 5_000
SHARED_ROLE = 50_000


def sint(v: Any, lo: int, hi: int) -> int:
    need(isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi)
    return v


def fired(at: int, every: int, x: int) -> int:
    """Number of due times at + k*every that are <= x."""
    return 0 if x < at else (x - at) // every + 1


class Roles:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.roles: Dict[str, dict] = {}
        self.beings: Dict[str, dict] = {}
        self.clock = 0

    def being(self, b: str, create: bool = False) -> dict:
        if b not in self.beings:
            need(create and len(self.beings) < self.lim["beings"])
            self.beings[b] = {"held": {}, "standing": 0}
        return self.beings[b]

    def status(self, b: str) -> int:
        x = self.beings[b]
        top = max((self.roles[r]["status"] for r in x["held"]), default=0)
        return max(0, min(100, top + x["standing"]))

    def view(self, b: str) -> dict:
        x = self.beings[b]
        return {"being": b, "roles": sorted(x["held"]), "standing": x["standing"], "status": self.status(b)}

    def holders(self, r: str) -> List[str]:
        return sorted(b for b, x in self.beings.items() if r in x["held"])

    # ---------------------------------------------------------------- ops

    def op_role(self, a: dict) -> dict:
        keys(a, ["id", "status"], ["duties", "seats"])
        r = ident(a["id"])
        st = uint(a["status"], 0, 100)
        seats = None if a.get("seats") is None else uint(a["seats"], 1, 1_000_000)
        rows = [] if a.get("duties") is None else a["duties"]
        need(isinstance(rows, list) and len(rows) <= self.lim["duties"])
        duties = {}
        for d in rows:
            keys(d, ["id", "action", "every"], ["at", "place", "priority"])
            did = ident(d["id"])
            need(did not in duties)
            duties[did] = {"action": ident(d["action"]), "every": uint(d["every"], 1, MAX_AT),
                           "at": 0 if d.get("at") is None else uint(d["at"], 0, MAX_AT),
                           "place": None if d.get("place") is None else ident(d["place"]),
                           "priority": 50 if d.get("priority") is None else uint(d["priority"], 0, 100)}
        need(r in self.roles or len(self.roles) < self.lim["roles"])
        self.roles[r] = {"status": st, "seats": seats, "duties": duties}
        return {"duties": sorted(duties), "id": r, "seats": seats, "status": st}

    def op_assign(self, a: dict) -> dict:
        keys(a, ["being", "role"], [])
        b, r = ident(a["being"]), ident(a["role"])
        need(r in self.roles)
        role = self.roles[r]
        x = self.beings.get(b)
        need(x is None or r not in x["held"])
        need(x is not None or len(self.beings) < self.lim["beings"])
        need(x is None or len(x["held"]) < self.lim["held"])
        need(role["seats"] is None or len(self.holders(r)) < role["seats"])
        x = self.being(b, create=True)
        x["held"][r] = self.clock
        return self.view(b)

    def op_vacate(self, a: dict) -> dict:
        keys(a, ["being", "role"], [])
        b, r = ident(a["being"]), ident(a["role"])
        need(b in self.beings and r in self.beings[b]["held"])
        del self.beings[b]["held"][r]
        return self.view(b)

    def op_esteem(self, a: dict) -> dict:
        keys(a, ["being", "delta", "reason"], [])
        b = ident(a["being"])
        delta = sint(a["delta"], -20, 20)
        reason = ident(a["reason"])
        x = self.being(b, create=True)
        x["standing"] = max(-50, min(50, x["standing"] + delta))
        return {**self.view(b), "reason": reason}

    def op_tick(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        tasks, effects = [], []
        for b in sorted(self.beings):
            x = self.beings[b]
            for r in sorted(x["held"]):
                since = x["held"][r]
                for did, d in sorted(self.roles[r]["duties"].items()):
                    n = fired(d["at"], d["every"], now) - fired(d["at"], d["every"], since)
                    if n <= 0:
                        continue
                    last = d["at"] + (fired(d["at"], d["every"], now) - 1) * d["every"]
                    tid = f"{r}-{did}-{last}"
                    tasks.append({"action": d["action"], "at": last, "being": b, "duty": did, "id": tid,
                                  "missed": n - 1, "place": d["place"], "priority": d["priority"], "role": r})
                    step = {"id": did, "action": d["action"]}
                    if d["place"] is not None:
                        step["target_bindings"] = {"place": d["place"]}
                    effects.append({"engine": "task", "op": "propose",
                                    "args": {"id": tid, "owner": b, "steps": [step]}})
                x["held"][r] = now
        self.clock = now
        unstaffed = []
        for r in sorted(self.roles):
            seats = self.roles[r]["seats"]
            h = len(self.holders(r))
            if seats is not None and h < seats:
                unstaffed.append({"holders": h, "role": r, "seats": seats})
        return {"effects": effects, "now": now, "tasks": tasks, "unstaffed": unstaffed}

    def op_contest(self, a: dict) -> dict:
        keys(a, ["a", "b"], [])
        x, y = ident(a["a"]), ident(a["b"])
        need(x != y and x in self.beings and y in self.beings)
        sx, sy = self.status(x), self.status(y)
        defers = y if sx - sy >= DEFER_AT else x if sy - sx >= DEFER_AT else None
        return {"a": x, "b": y, "defers": defers, "edge": (sx - sy) * EDGE_CONTEST, "status_a": sx, "status_b": sy}

    def op_persuade(self, a: dict) -> dict:
        keys(a, ["speaker", "listener", "chance"], [])
        s, l = ident(a["speaker"]), ident(a["listener"])
        base = uint(a["chance"], 0, M)
        need(s != l and s in self.beings and l in self.beings)
        edge = (self.status(s) - self.status(l)) * EDGE_PERSUADE
        shared = sorted(set(self.beings[s]["held"]) & set(self.beings[l]["held"]))
        bonus = SHARED_ROLE if shared else 0
        return {"base": base, "chance": max(0, min(M, base + edge + bonus)), "listener": l, "shared_roles": shared,
                "speaker": s, "status_edge": edge}

    def op_assess(self, a: dict) -> dict:
        keys(a, ["being"], [])
        b = ident(a["being"])
        need(b in self.beings)
        return self.view(b)

    def op_staff(self, a: dict) -> dict:
        keys(a, ["role"], [])
        r = ident(a["role"])
        need(r in self.roles)
        h = self.holders(r)
        return {"holders": h, "role": r, "seats": self.roles[r]["seats"], "status": self.roles[r]["status"]}

    def workflow(self, request: Any) -> dict:
        table = {"role": self.op_role, "assign": self.op_assign, "vacate": self.op_vacate, "esteem": self.op_esteem,
                 "tick": self.op_tick, "contest": self.op_contest, "persuade": self.op_persuade,
                 "assess": self.op_assess, "staff": self.op_staff}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Roles().workflow(request)
