"""Independent original reference for slow personality change (pass 2, B4).
Python standard library only; nothing here calls a model.

An author gives a being traits (values and temperament dimensions such as trust or boldness) as
signed millionths with a hard `cap` on how far experience may move them from the authored base.
`experience` pushes a trait (-100..100) with a reason; pushes add up in a pressure that fades by
`fade` per tick toward 0. Each time the pressure reaches `threshold` either way, the trait shifts
by one `step` in that direction (never past base +/- cap; at the cap the pressure is spent
without change), the threshold is taken off the pressure, and each shift is recorded with the reasons since the last push that shifted it (the newest 8). Authors
can `lock` a trait or a whole being (pushes are then ignored and reported as locked) and
`revert` a trait to its base.
"""
import pathlib
import sys
from typing import Any, Dict, List

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_uint, uint
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
M = 1_000_000
SPEC = [("beings", 1024, 1_000_000), ("traits", 16, 64), ("history", 16, 256)]
REASONS_KEPT = 8


def sint(v: Any, lo: int, hi: int) -> int:
    need(isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi)
    return v


class Drift:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.beings: Dict[str, dict] = {}
        self.clock = 0

    def trait_view(self, b: str, t: str) -> dict:
        x = self.beings[b]["traits"][t]
        return {"base": x["base"], "being": b, "cap": x["cap"], "drift": x["value"] - x["base"], "fade": x["fade"],
                "history": [dict(h) for h in x["history"]], "locked": x["locked"] or self.beings[b]["locked"],
                "pressure": x["pressure"], "step": x["step"], "threshold": x["threshold"], "trait": t,
                "value": x["value"]}

    def fade_to(self, x: dict, now: int) -> None:
        gone = (now - x["at"]) * x["fade"]
        p = x["pressure"]
        x["pressure"] = max(0, p - gone) if p > 0 else min(0, p + gone)
        x["at"] = now

    # ---------------------------------------------------------------- ops

    def op_trait(self, a: dict) -> dict:
        keys(a, ["being", "trait", "base"], ["cap", "step", "threshold", "fade"])
        b, t = ident(a["being"]), ident(a["trait"])
        base = sint(a["base"], -M, M)
        cap = opt_uint(a, "cap", 0, 300_000, 150_000)
        step = opt_uint(a, "step", 1, 100_000, 10_000)
        thr = opt_uint(a, "threshold", 1, 100_000, 300)
        fade = opt_uint(a, "fade", 0, 100_000, 1)
        x = self.beings.get(b)
        need(x is not None or len(self.beings) < self.lim["beings"])
        need(x is None or t in x["traits"] or len(x["traits"]) < self.lim["traits"])
        x = self.beings.setdefault(b, {"traits": {}, "locked": False})
        x["traits"][t] = {"base": base, "value": base, "cap": cap, "step": step, "threshold": thr, "fade": fade,
                          "pressure": 0, "at": self.clock, "reasons": [], "history": [], "locked": False}
        return self.trait_view(b, t)

    def op_experience(self, a: dict) -> dict:
        keys(a, ["being", "trait", "push", "reason"], [])
        b, t = ident(a["being"]), ident(a["trait"])
        push = sint(a["push"], -100, 100)
        reason = ident(a["reason"])
        need(b in self.beings and t in self.beings[b]["traits"])
        x = self.beings[b]["traits"][t]
        if x["locked"] or self.beings[b]["locked"]:
            return {**self.trait_view(b, t), "shifts": []}
        self.fade_to(x, self.clock)
        x["pressure"] += push
        x["reasons"] = (x["reasons"] + [reason])[-REASONS_KEPT:]
        shifts = []
        spent = False
        while abs(x["pressure"]) >= x["threshold"]:
            sign = 1 if x["pressure"] > 0 else -1
            x["pressure"] -= sign * x["threshold"]
            lo, hi = x["base"] - x["cap"], x["base"] + x["cap"]
            new = max(max(lo, -M), min(min(hi, M), x["value"] + sign * x["step"]))
            delta = new - x["value"]
            if delta != 0:  # at the cap the pressure is spent without change
                x["value"] = new
                h = {"at": self.clock, "delta": delta, "reasons": list(x["reasons"]), "value": new}
                x["history"] = (x["history"] + [h])[-self.lim["history"]:]
                shifts.append(h)
            spent = True
        if spent:
            x["reasons"] = []
        return {**self.trait_view(b, t), "shifts": shifts}

    def op_tick(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        self.clock = now
        return {"now": now}

    def op_lock(self, a: dict) -> dict:
        keys(a, ["being", "locked"], ["trait"])
        b = ident(a["being"])
        need(isinstance(a["locked"], bool))
        locked = a["locked"]
        t = None if a.get("trait") is None else ident(a["trait"])
        need(b in self.beings and (t is None or t in self.beings[b]["traits"]))
        if t is None:
            self.beings[b]["locked"] = locked
        else:
            self.beings[b]["traits"][t]["locked"] = locked
        return {"being": b, "locked": locked, "trait": t}

    def op_revert(self, a: dict) -> dict:
        keys(a, ["being", "trait"], [])
        b, t = ident(a["being"]), ident(a["trait"])
        need(b in self.beings and t in self.beings[b]["traits"])
        x = self.beings[b]["traits"][t]
        x["value"], x["pressure"], x["reasons"], x["at"] = x["base"], 0, [], self.clock
        return self.trait_view(b, t)

    def op_assess(self, a: dict) -> dict:
        keys(a, ["being"], ["trait"])
        b = ident(a["being"])
        t = None if a.get("trait") is None else ident(a["trait"])
        need(b in self.beings and (t is None or t in self.beings[b]["traits"]))
        names = [t] if t is not None else sorted(self.beings[b]["traits"])
        views = []
        for n in names:
            x = self.beings[b]["traits"][n]
            saved = x["pressure"], x["at"]
            self.fade_to(x, self.clock)
            views.append(self.trait_view(b, n))
            x["pressure"], x["at"] = saved
        return {"being": b, "locked": self.beings[b]["locked"], "traits": views}

    def workflow(self, request: Any) -> dict:
        table = {"trait": self.op_trait, "experience": self.op_experience, "tick": self.op_tick,
                 "lock": self.op_lock, "revert": self.op_revert, "assess": self.op_assess}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Drift().workflow(request)
