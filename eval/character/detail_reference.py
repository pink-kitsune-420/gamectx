"""Independent original reference for level of detail for minds (pass 2, D2).
Python standard library only; nothing here calls a model.

Each tracked being has needs that grow linearly (level and rate in millionths of full, a
threshold that makes the need urgent). Its detail level follows its distance from the focus:
`full` (distance <= near) is updated every tick, `coarse` (<= far) every `stride` ticks, and
`crowd` beings are frozen inside a named group and only summarized. Because growth is linear,
catching up after any gap gives exactly the levels and the exact urgent tick that a full update
would have; moving between levels never loses or invents state. Only the reported work differs:
full costs needs x ticks, coarse needs x strides, a crowd one unit per group per tick call.
"""
import pathlib
import sys
from typing import Any, Dict, List, Optional

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_uint, uint
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
M = 1_000_000
SPEC = [("beings", 1024, 1_000_000), ("needs", 8, 64), ("groups", 64, 65536)]
LEVELS = ["full", "coarse", "crowd"]


class Detail:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.beings: Dict[str, dict] = {}
        self.clock = 0
        self.near, self.far, self.stride = 20, 100, 10

    def level_of(self, distance: int) -> str:
        return "full" if distance <= self.near else "coarse" if distance <= self.far else "crowd"

    @staticmethod
    def urgent_at(n: dict, since: int) -> Optional[int]:
        """First tick > since at which the need reaches its threshold (None if never or already)."""
        if n["level"] >= n["threshold"] or n["rate"] == 0:
            return None
        return since + -(-(n["threshold"] - n["level"]) // n["rate"])

    def advance(self, b: str, now: int) -> List[dict]:
        """Bring a being's needs to `now`; return urgent events in (tick, need) order."""
        x = self.beings[b]
        events = []
        if now <= x["at"]:
            return events
        for nid in sorted(x["needs"]):
            n = x["needs"][nid]
            t = self.urgent_at(n, x["at"])
            if t is not None and t <= now:
                events.append({"at": t, "being": b, "need": nid})
            n["level"] = min(M, n["level"] + n["rate"] * (now - x["at"]))
        x["at"] = now
        return sorted(events, key=lambda e: (e["at"], e["need"]))

    def view(self, b: str) -> dict:
        x = self.beings[b]
        return {"at": x["at"], "being": b, "distance": x["distance"], "group": x["group"], "level": x["detail"],
                "needs": [{"id": k, "level": n["level"], "rate": n["rate"], "threshold": n["threshold"]}
                          for k, n in sorted(x["needs"].items())]}

    def place(self, b: str, distance: int, group: Optional[str]) -> None:
        x = self.beings[b]
        lvl = self.level_of(distance)
        need(lvl != "crowd" or group is not None)
        x["distance"], x["detail"] = distance, lvl
        x["group"] = group if lvl == "crowd" else None

    def groups(self) -> set:
        return {x["group"] for x in self.beings.values() if x["group"] is not None}

    # ---------------------------------------------------------------- ops

    def op_configure(self, a: dict) -> dict:
        keys(a, [], ["near", "far", "stride"])
        near = opt_uint(a, "near", 0, MAX_AT, self.near)
        far = opt_uint(a, "far", 0, MAX_AT, self.far)
        stride = opt_uint(a, "stride", 1, 1_000_000, self.stride)
        need(near <= far and not self.beings)
        self.near, self.far, self.stride = near, far, stride
        return {"far": far, "near": near, "stride": stride}

    def op_track(self, a: dict) -> dict:
        keys(a, ["being", "needs", "distance"], ["group"])
        b = ident(a["being"])
        rows = a["needs"]
        need(isinstance(rows, list) and 0 < len(rows) <= self.lim["needs"])
        needs = {}
        for r in rows:
            keys(r, ["id", "rate"], ["level", "threshold"])
            nid = ident(r["id"])
            need(nid not in needs)
            needs[nid] = {"level": opt_uint(r, "level", 0, M, 0), "rate": uint(r["rate"], 0, M),
                          "threshold": opt_uint(r, "threshold", 1, M, 700_000)}
        distance = uint(a["distance"], 0, MAX_AT)
        g = None if a.get("group") is None else ident(a["group"])
        need(b not in self.beings and len(self.beings) < self.lim["beings"])
        lvl = self.level_of(distance)
        need(lvl != "crowd" or g is not None)
        need(lvl != "crowd" or g in self.groups() or len(self.groups()) < self.lim["groups"])
        self.beings[b] = {"needs": needs, "at": self.clock, "distance": 0, "detail": "full", "group": None}
        self.place(b, distance, g)
        return self.view(b)

    def op_move(self, a: dict) -> dict:
        keys(a, ["being", "distance"], ["group"])
        b = ident(a["being"])
        distance = uint(a["distance"], 0, MAX_AT)
        g = None if a.get("group") is None else ident(a["group"])
        need(b in self.beings)
        x = self.beings[b]
        lvl = self.level_of(distance)
        need(lvl != "crowd" or g is not None)
        others = {y["group"] for k, y in self.beings.items() if k != b and y["group"] is not None}
        need(lvl != "crowd" or g in others or len(others) < self.lim["groups"])
        before = x["detail"]
        events = self.advance(b, self.clock)  # catch up exactly before the level changes
        self.place(b, distance, g)
        return {"events": events, "from": before, "to": x["detail"], **{"being": self.view(b)}}

    def op_tick(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        events: List[dict] = []
        work = {"coarse": 0, "crowd": 0, "full": 0}
        elapsed = now - self.clock
        for b in sorted(self.beings):
            x = self.beings[b]
            n = len(x["needs"])
            if x["detail"] == "full":
                work["full"] += n * elapsed
                events += self.advance(b, now)
            elif x["detail"] == "coarse":
                # Updated on stride boundaries only: catch up to the last boundary <= now.
                edge = now - now % self.stride
                if edge > x["at"]:
                    work["coarse"] += n * ((edge - x["at"] + self.stride - 1) // self.stride)
                    events += self.advance(b, edge)
        if elapsed:
            work["crowd"] = len(self.groups())
        self.clock = now
        return {"events": sorted(events, key=lambda e: (e["at"], e["being"], e["need"])), "now": now, "work": work}

    def op_assess(self, a: dict) -> dict:
        keys(a, ["being"], [])
        b = ident(a["being"])
        need(b in self.beings)
        x = self.beings[b]
        # A read-only view as of the clock, whatever the level (no state change).
        saved = {k: dict(n) for k, n in x["needs"].items()}, x["at"]
        self.advance(b, self.clock)
        v = self.view(b)
        x["needs"], x["at"] = saved
        return {**v, "lag": self.clock - x["at"]}

    def op_crowd(self, a: dict) -> dict:
        keys(a, ["group"], [])
        g = ident(a["group"])
        members = sorted(b for b, x in self.beings.items() if x["group"] == g)
        need(members)
        totals: Dict[str, List[int]] = {}
        urgent = 0
        for b in members:
            x = self.beings[b]
            for nid, n in x["needs"].items():
                lvl = min(M, n["level"] + n["rate"] * (self.clock - x["at"]))
                t = totals.setdefault(nid, [0, 0])
                t[0] += lvl
                t[1] += 1
                if lvl >= n["threshold"]:
                    urgent += 1
        return {"group": g, "members": len(members), "needs": [{"id": k, "mean": s // c, "count": c}
                                                               for k, (s, c) in sorted(totals.items())],
                "urgent": urgent}

    def workflow(self, request: Any) -> dict:
        table = {"configure": self.op_configure, "track": self.op_track, "move": self.op_move, "tick": self.op_tick,
                 "assess": self.op_assess, "crowd": self.op_crowd}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Detail().workflow(request)
