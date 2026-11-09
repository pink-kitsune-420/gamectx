"""Independent original reference for bounded rationality and fallible memory (pass 2, A7/A8).
Python standard library only; nothing here calls a model.

A7: each being has cognition settings from a temperament (plan depth, horizon, impulsiveness,
memory span, slip rate on unfamiliar steps). `decide` is seeded and deterministic: an impulse
roll may skip planning and act on the top motive; otherwise the plan keeps at most `plan_depth`
steps due within the horizon, and unfamiliar steps may slip.

A8: memories fade on `tick` by importance and memory span (importance 100 never fades) and are
forgotten at zero. `recall` rehearses a memory; a hazy one may come back wrong (a shifted day, or
a person merged in from another memory), always reported with the truth beside it so the host
knows what really happened.

All rates and strengths are fixed-point millionths (integers).
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
MASK = (1 << 64) - 1
SPEC = [("beings", 1024, 1_000_000), ("memories", 256, 65536), ("people", 8, 64), ("motives", 16, 256),
        ("steps", 16, 256)]
TEMPERAMENTS = {
    # plan_depth, horizon (ticks), impulsiveness (0..100), memory_span (ticks), slip (millionths)
    "careful": (6, 240, 10, 2000, 50_000),
    "steady": (4, 120, 25, 1000, 100_000),
    "impulsive": (2, 30, 70, 500, 200_000),
    "dreamy": (3, 60, 40, 300, 250_000),
}
SETTINGS = ["plan_depth", "horizon", "impulsiveness", "memory_span", "slip"]
BOUNDS = {"plan_depth": (1, 8), "horizon": (1, 10_000), "impulsiveness": (0, 100), "memory_span": (1, 1_000_000),
          "slip": (0, M)}
VIVID, CLEAR = 900_000, 600_000
REHEARSE = 100_000


def fnv1a(s: str) -> int:
    h = 0xCBF29CE484222325
    for b in s.encode("utf-8"):
        h ^= b
        h = (h * 0x100000001B3) & MASK
    return h


def clarity(strength: int) -> str:
    return "vivid" if strength >= VIVID else "clear" if strength >= CLEAR else "hazy"


class Cognition:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.profiles: Dict[str, dict] = {}
        self.memories: Dict[str, Dict[str, dict]] = {}
        self.clock = 0

    def settings(self, being: str) -> dict:
        return self.profiles.get(being) or dict(zip(["temperament"] + SETTINGS, ["steady", *TEMPERAMENTS["steady"]]))

    def room(self, being: str) -> None:
        known = being in self.profiles or being in self.memories
        need(known or len(set(self.profiles) | set(self.memories)) < self.lim["beings"])

    # ---------------------------------------------------------------- A7

    def op_profile(self, a: dict) -> dict:
        keys(a, ["being"], ["temperament"] + SETTINGS)
        b = ident(a["being"])
        t = a.get("temperament")
        t = "steady" if t is None else ident(t)
        need(t in TEMPERAMENTS)
        p = dict(zip(["temperament"] + SETTINGS, [t, *TEMPERAMENTS[t]]))
        for k in SETTINGS:
            lo, hi = BOUNDS[k]
            p[k] = opt_uint(a, k, lo, hi, p[k])
        self.room(b)
        self.profiles[b] = p
        return {"being": b, **p}

    def op_decide(self, a: dict) -> dict:
        keys(a, ["being", "seed", "motives"], ["steps", "at"])
        b = ident(a["being"])
        seed = uint(a["seed"], 0, MAX_AT)
        at = opt_uint(a, "at", 0, MAX_AT, 0)
        ms = a["motives"]
        need(isinstance(ms, list) and 0 < len(ms) <= self.lim["motives"])
        motives = []
        for m in ms:
            keys(m, ["id", "score"], [])
            motives.append((ident(m["id"]), uint(m["score"], 0, 100)))
        need(len({m[0] for m in motives}) == len(motives))
        st = a.get("steps")
        st = [] if st is None else st
        need(isinstance(st, list) and len(st) <= self.lim["steps"])
        steps = []
        for s in st:
            keys(s, ["id"], ["familiar", "eta"])
            steps.append((ident(s["id"]), opt_bool(s, "familiar", True), opt_uint(s, "eta", 0, MAX_AT, at)))
        need(len({s[0] for s in steps}) == len(steps))
        p = self.settings(b)
        top = sorted(motives, key=lambda m: (-m[1], m[0]))[0][0]
        roll = fnv1a(f"{seed}:{b}:impulse") % 100
        if roll < p["impulsiveness"]:
            return {"being": b, "beyond_horizon": [], "dropped": 0, "mode": "impulse", "motive": top, "plan": [],
                    "roll": roll, "slipped": None}
        near = [s for s in steps if s[2] <= at + p["horizon"]]
        beyond = [s[0] for s in steps if s[2] > at + p["horizon"]]
        kept = near[:p["plan_depth"]]
        plan = []
        slipped = None
        for sid, familiar, _ in kept:
            slip = not familiar and fnv1a(f"{seed}:{b}:{sid}") % M < p["slip"]
            if slip and slipped is None:
                slipped = sid
            plan.append({"id": sid, "slip": slip})
        return {"being": b, "beyond_horizon": beyond, "dropped": len(near) - len(kept), "mode": "plan", "motive": top,
                "plan": plan, "roll": roll, "slipped": slipped}

    # ---------------------------------------------------------------- A8

    def mem(self, a: dict) -> tuple:
        b = ident(a["being"])
        mid = ident(a["id"])
        m = self.memories.get(b, {}).get(mid)
        need(m is not None)
        return b, mid, m

    @staticmethod
    def facts(m: dict) -> dict:
        return {"about": m["about"], "day": m["day"], "people": list(m["people"]), "what": m["what"]}

    def view(self, mid: str, m: dict) -> dict:
        return {"id": mid, **self.facts(m), "clarity": clarity(m["strength"]), "importance": m["importance"],
                "strength": m["strength"]}

    def op_remember(self, a: dict) -> dict:
        keys(a, ["being", "id", "about", "what", "day"], ["people", "importance", "at"])
        b, mid = ident(a["being"]), ident(a["id"])
        about, what = ident(a["about"]), ident(a["what"])
        day = uint(a["day"], 0, MAX_AT)
        ppl = a.get("people")
        ppl = [] if ppl is None else ppl
        need(isinstance(ppl, list) and len(ppl) <= self.lim["people"])
        people = [ident(x) for x in ppl]
        need(len(set(people)) == len(people))
        imp = opt_uint(a, "importance", 0, 100, 50)
        at = opt_uint(a, "at", 0, MAX_AT, 0)
        book = self.memories.get(b, {})
        need(mid not in book)
        self.room(b)
        evicted = None
        if len(book) >= self.lim["memories"]:
            evicted = min(book, key=lambda k: (book[k]["strength"], book[k]["importance"], k))
        book = self.memories.setdefault(b, {})
        if evicted is not None:
            del book[evicted]
        book[mid] = {"about": about, "what": what, "day": day, "people": people, "importance": imp, "strength": M,
                     "last": max(at, self.clock)}
        return {"being": b, "evicted": evicted, "memory": self.view(mid, book[mid])}

    def op_tick(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        changed = []
        forgotten = []
        for b in sorted(self.memories):
            span = self.settings(b)["memory_span"]
            book = self.memories[b]
            for mid in sorted(book):
                m = book[mid]
                if m["importance"] >= 100 or now <= m["last"]:
                    continue
                elapsed = min(now - m["last"], 100 * span)
                loss = elapsed * M * (100 - m["importance"]) // (100 * span)
                before = clarity(m["strength"])
                m["strength"] = max(0, m["strength"] - loss)
                m["last"] = now
                if m["strength"] == 0:
                    del book[mid]
                    forgotten.append({"being": b, "id": mid})
                elif clarity(m["strength"]) != before:
                    changed.append({"being": b, "clarity": clarity(m["strength"]), "id": mid})
            if not book:
                del self.memories[b]
        self.clock = now
        return {"changed": changed, "forgotten": forgotten, "now": now}

    def op_recall(self, a: dict) -> dict:
        keys(a, ["being", "id", "seed"], ["errors"])
        b, mid, m = self.mem(a)
        seed = uint(a["seed"], 0, MAX_AT)
        errors = opt_bool(a, "errors", True)
        truth = self.facts(m)
        told = self.facts(m)
        kind = None
        if errors and m["strength"] < CLEAR:
            roll = fnv1a(f"{seed}:{b}:{mid}") % M
            if roll < CLEAR - m["strength"]:
                others = sorted({p for k, o in self.memories[b].items() if k != mid for p in o["people"]}
                                - set(m["people"]))
                if (roll // 7) % 2 == 1 and m["people"] and others:
                    kind = "merged_person"
                    told["people"][0] = others[roll % len(others)]
                else:
                    kind = "wrong_day"
                    shift = 1 + roll % 3
                    told["day"] = m["day"] + shift if (roll // 3) % 2 == 0 or m["day"] < shift else m["day"] - shift
        m["strength"] = min(M, m["strength"] + REHEARSE)
        return {"being": b, "clarity": clarity(m["strength"]), "distorted": kind is not None, "id": mid, "kind": kind,
                "strength": m["strength"], "told": told, "truth": truth}

    def op_forget(self, a: dict) -> dict:
        keys(a, ["being", "id"], [])
        b, mid, _ = self.mem(a)
        del self.memories[b][mid]
        if not self.memories[b]:
            del self.memories[b]
        return {"being": b, "forgot": mid}

    def op_assess(self, a: dict) -> dict:
        keys(a, ["being"], [])
        b = ident(a["being"])
        book = self.memories.get(b, {})
        mems = [self.view(k, m) for k, m in sorted(book.items(), key=lambda kv: (-kv[1]["strength"], kv[0]))]
        return {"being": b, "memories": mems, "profile": self.settings(b)}

    def workflow(self, request: Any) -> dict:
        table = {"profile": self.op_profile, "decide": self.op_decide, "remember": self.op_remember,
                 "tick": self.op_tick, "recall": self.op_recall, "forget": self.op_forget, "assess": self.op_assess}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Cognition().workflow(request)
