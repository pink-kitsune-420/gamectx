"""Independent original reference for life cycle and threat response (pass 2, C1/C2).
Python standard library only; nothing here calls a model.

C1: beings are born with a lifespan (ticks), grow from child (< 22.5% of lifespan) to adult
(< 75%) to elder, pair up, have children, own belongings and die (of age on `age`, or by `die`).
A death passes belongings to heirs (living children by birth, else the partner, else living
parents) and leaves grief for the bereaved.

C2: `threat` appraises danger in millionths from the threat's danger, relative strength and
injury, then scores fight, flee, hide, call for help and freeze from temperament, allies, exits
and cover. Safety is a need: a threat lowers it, and it recovers by one per tick on `age`.

All scores are signed fixed-point millionths (integers). Grief and fear are proposals for the
affect engine.
"""
import pathlib
import sys
from typing import Any, Dict, List, Optional

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_bool, opt_uint, uint
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
M = 1_000_000
SPEC = [("beings", 1024, 1_000_000), ("belongings", 64, 4096), ("children", 16, 256)]
ADULT_AT, ELDER_AT = 225_000, 750_000
GRIEF = {"partner": 60, "child": 50, "parent": 40}
RESPONSES = ["fight", "flee", "hide", "call_for_help", "freeze"]
TEMPERS = {
    "bold": {"fight": 250_000, "flee": -100_000, "hide": -100_000, "call_for_help": 0, "freeze": -200_000},
    "steady": {"fight": 0, "flee": 0, "hide": 0, "call_for_help": 0, "freeze": 0},
    "cautious": {"fight": -150_000, "flee": 100_000, "hide": 150_000, "call_for_help": 100_000, "freeze": 0},
    "timid": {"fight": -300_000, "flee": 50_000, "hide": 100_000, "call_for_help": 50_000, "freeze": 250_000},
}
IGNORE_BELOW = 200_000
UNSAFE_BELOW = 40


def stage(age: int, lifespan: int) -> str:
    f = age * M // lifespan
    return "child" if f < ADULT_AT else "adult" if f < ELDER_AT else "elder"


class Survival:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.beings: Dict[str, dict] = {}
        self.clock = 0

    def get(self, b: str, alive: bool = True) -> dict:
        x = self.beings.get(b)
        need(x is not None and (not alive or x["alive"]))
        return x

    def view(self, b: str) -> dict:
        x = self.beings[b]
        return {"alive": x["alive"], "being": b, "belongings": list(x["belongings"]), "born": x["born"],
                "cause": x["cause"], "children": list(x["children"]), "died": x["died"], "lifespan": x["lifespan"],
                "parents": list(x["parents"]), "partner": x["partner"], "safety": x["safety"], "stage": x["stage"]}

    # ---------------------------------------------------------------- C1

    def op_born(self, a: dict) -> dict:
        keys(a, ["being", "lifespan"], ["parents", "at"])
        b = ident(a["being"])
        life = uint(a["lifespan"], 1, MAX_AT)
        at = opt_uint(a, "at", 0, MAX_AT, self.clock)
        ps = a.get("parents")
        ps = [] if ps is None else ps
        need(isinstance(ps, list) and len(ps) <= 2)
        parents = [ident(p) for p in ps]
        need(len(set(parents)) == len(parents) and b not in parents and b not in self.beings)
        need(len(self.beings) < self.lim["beings"])
        for p in parents:
            px = self.get(p)
            need(px["stage"] != "child" and len(px["children"]) < self.lim["children"])
        for p in parents:
            self.beings[p]["children"].append(b)
        self.beings[b] = {"born": at, "lifespan": life, "parents": parents, "partner": None, "children": [],
                          "belongings": [], "alive": True, "died": None, "cause": None, "safety": 100,
                          "stage": stage(max(0, self.clock - at), life), "last": max(at, self.clock)}
        return self.view(b)

    def op_pair(self, a: dict) -> dict:
        keys(a, ["a", "b"], [])
        x, y = ident(a["a"]), ident(a["b"])
        need(x != y)
        bx, by = self.get(x), self.get(y)
        need(bx["stage"] != "child" and by["stage"] != "child" and bx["partner"] is None and by["partner"] is None)
        bx["partner"], by["partner"] = y, x
        return {"a": x, "b": y}

    def op_own(self, a: dict) -> dict:
        keys(a, ["being", "thing"], [])
        b, t = ident(a["being"]), ident(a["thing"])
        x = self.get(b)
        need(t not in x["belongings"] and len(x["belongings"]) < self.lim["belongings"])
        x["belongings"].append(t)
        return {"being": b, "belongings": list(x["belongings"])}

    def death(self, b: str, at: int, cause: str) -> dict:
        x = self.beings[b]
        kids = sorted((c for c in x["children"] if self.beings[c]["alive"]),
                      key=lambda c: (self.beings[c]["born"], c))
        partner = x["partner"] if x["partner"] is not None and self.beings[x["partner"]]["alive"] else None
        parents = sorted(p for p in x["parents"] if self.beings[p]["alive"])
        heirs = kids or ([partner] if partner else parents)
        estate = []
        i = 0
        for thing in x["belongings"]:
            to = None
            for k in range(len(heirs)):
                h = heirs[(i + k) % len(heirs)]
                hx = self.beings[h]
                if thing not in hx["belongings"] and len(hx["belongings"]) < self.lim["belongings"]:
                    to = h
                    i = (i + k + 1) % len(heirs)
                    break
            if to is not None:
                self.beings[to]["belongings"].append(thing)
            estate.append({"thing": thing, "to": to})
        bereaved = []
        if partner:
            bereaved.append((partner, "partner"))
        bereaved += [(c, "child") for c in kids] + [(p, "parent") for p in parents]
        effects = [{"engine": "affect", "op": "appraise",
                    "args": {"being": who, "subject": b, "kind": "emotion", "delta": -GRIEF[rel], "reason": "grief"}}
                   for who, rel in sorted(bereaved)]
        if partner:
            self.beings[partner]["partner"] = None
        x.update(alive=False, died=at, cause=cause, belongings=[], partner=None)
        return {"being": b, "bereaved": [{"being": w, "as": r} for w, r in sorted(bereaved)], "cause": cause,
                "died": at, "effects": effects, "estate": estate}

    def op_die(self, a: dict) -> dict:
        keys(a, ["being", "cause"], ["at"])
        b, cause = ident(a["being"]), ident(a["cause"])
        at = opt_uint(a, "at", 0, MAX_AT, self.clock)
        self.get(b)
        return self.death(b, at, cause)

    def op_age(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        events, deaths = [], []
        dying = []
        for b in sorted(self.beings):
            x = self.beings[b]
            if not x["alive"]:
                continue
            if now > x["last"]:
                x["safety"] = min(100, x["safety"] + (now - x["last"]))
                x["last"] = now
            age = max(0, now - x["born"])
            if age >= x["lifespan"]:
                dying.append(b)
                continue
            s = stage(age, x["lifespan"])
            if s != x["stage"]:
                events.append({"being": b, "event": "came_of_age" if s == "adult" else "grew_old"})
                x["stage"] = s
        for b in dying:
            if self.beings[b]["alive"]:
                deaths.append(self.death(b, now, "old_age"))
        self.clock = now
        return {"deaths": deaths, "events": events, "now": now}

    # ---------------------------------------------------------------- C2

    def op_threat(self, a: dict) -> dict:
        keys(a, ["being", "threat", "danger"], ["temperament", "strength", "threat_strength", "injury", "allies",
                                                "exit", "cover"])
        b, th = ident(a["being"]), ident(a["threat"])
        danger = uint(a["danger"], 0, 100)
        t = a.get("temperament")
        t = "steady" if t is None else ident(t)
        need(t in TEMPERS)
        st = opt_uint(a, "strength", 0, 100, 50)
        ts = opt_uint(a, "threat_strength", 0, 100, 50)
        inj = opt_uint(a, "injury", 0, 100, 0)
        allies = opt_uint(a, "allies", 0, 1000, 0)
        exit_ = opt_bool(a, "exit", True)
        cover = opt_bool(a, "cover", False)
        x = self.get(b)
        d = max(0, min(M, danger * 10_000 + (ts - st) * 3_000 + inj * 2_000))
        helpers = min(allies, 5)
        tm = TEMPERS[t]
        scores = {
            "fight": 300_000 + (st - ts) * 5_000 + helpers * 100_000 - inj * 4_000 + tm["fight"],
            "flee": (400_000 if exit_ else 0) + d // 2 - inj * 3_000 + tm["flee"],
            "hide": (450_000 if cover else 0) + d // 3 + tm["hide"],
            "call_for_help": (250_000 + helpers * 50_000 if allies else 0) + tm["call_for_help"],
            "freeze": d // 2 - 200_000 + inj * 2_000 + tm["freeze"],
        }
        if d < IGNORE_BELOW:
            response = "ignore"
            effects = []
        else:
            response = max(RESPONSES, key=lambda r: (scores[r], -RESPONSES.index(r)))
            x["safety"] = min(x["safety"], 100 - d // 10_000)
            effects = [{"engine": "affect", "op": "appraise",
                        "args": {"being": b, "subject": th, "kind": "fear", "delta": d // 20_000, "reason": response}}]
        return {"being": b, "danger": d, "effects": effects, "response": response, "safety": x["safety"],
                "scores": [{"response": r, "score": scores[r]} for r in RESPONSES], "threat": th,
                "unsafe": x["safety"] < UNSAFE_BELOW}

    def op_assess(self, a: dict) -> dict:
        keys(a, ["being"], [])
        b = ident(a["being"])
        self.get(b, alive=False)
        return self.view(b)

    def workflow(self, request: Any) -> dict:
        table = {"born": self.op_born, "pair": self.op_pair, "own": self.op_own, "age": self.op_age,
                 "die": self.op_die, "threat": self.op_threat, "assess": self.op_assess}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Survival().workflow(request)
