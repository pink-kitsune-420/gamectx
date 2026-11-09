"""Independent original reference for off-screen conversation between characters (pass 2, B6).
Python standard library only; nothing here calls a model.

Beings hold topics they know (a value, a weight 0..100, maybe secret) and topics they want to
learn. When two beings `meet`, a seeded roll picks who speaks first and they take turns: each
turn the speaker tells the listener the best topic it can share (wanted topics first), and a
topic both hold with different values becomes a dispute. Secrets are shared only at warmth 60 or
more. The exchange is summarized as a record with no model call; listeners learn what they were
told, warmth rises when wanted things are shared and falls in quarrels. Learned facts and
feelings are proposals for the beliefs, relationship and affect engines.
"""
import pathlib
import sys
from typing import Any, Dict, List, Optional, Tuple

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_bool, opt_uint, uint
from cognition_reference import fnv1a
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
SPEC = [("beings", 1024, 1_000_000), ("topics", 64, 4096), ("wants", 16, 256), ("bonds", 4096, 1_000_000),
        ("records", 256, 1_000_000), ("turns", 6, 32)]
SECRET_WARMTH = 60
WANT_WARMTH = 5
QUARREL_BELOW = 20
QUARREL_COST = 10
ALLY_AT = 60


def sint(v: Any, lo: int, hi: int) -> int:
    need(isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi)
    return v


def pair(a: str, b: str) -> Tuple[str, str]:
    return (a, b) if a < b else (b, a)


class Chatter:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.minds: Dict[str, dict] = {}  # being -> {"knows": {topic: [value, weight, secret]}, "wants": {topic: w}}
        self.bonds: Dict[Tuple[str, str], int] = {}
        self.records: List[dict] = []
        self.seq = 0

    def mind(self, b: str) -> dict:
        if b not in self.minds:
            need(len(self.minds) < self.lim["beings"])
            self.minds[b] = {"knows": {}, "wants": {}}
        return self.minds[b]

    def known(self, a: dict) -> Tuple[str, dict]:
        b = ident(a["being"])
        need(b in self.minds)
        return b, self.minds[b]

    def mind_view(self, b: str) -> dict:
        m = self.minds[b]
        return {"being": b,
                "knows": [{"secret": s, "topic": t, "value": v, "weight": w}
                          for t, (v, w, s) in sorted(m["knows"].items())],
                "wants": [{"topic": t, "weight": w} for t, w in sorted(m["wants"].items())]}

    def op_know(self, a: dict) -> dict:
        keys(a, ["being", "topic", "value"], ["weight", "secret"])
        b, t, v = ident(a["being"]), ident(a["topic"]), ident(a["value"])
        w = opt_uint(a, "weight", 0, 100, 50)
        s = opt_bool(a, "secret", False)
        cur = self.minds.get(b)
        need(cur is not None or len(self.minds) < self.lim["beings"])
        kn = cur["knows"] if cur is not None else {}
        need(t in kn or len(kn) < self.lim["topics"])
        m = self.mind(b)
        m["knows"][t] = [v, w, s]
        m["wants"].pop(t, None)
        return {"being": b, "secret": s, "topic": t, "topics": len(m["knows"]), "value": v, "wants": len(m["wants"]),
                "weight": w}

    def op_want(self, a: dict) -> dict:
        keys(a, ["being", "topic"], ["weight"])
        b, t = ident(a["being"]), ident(a["topic"])
        w = opt_uint(a, "weight", 1, 100, 50)
        need(b in self.minds or len(self.minds) < self.lim["beings"])
        cur = self.minds.get(b, {"knows": {}, "wants": {}})
        need(t not in cur["knows"] and (t in cur["wants"] or len(cur["wants"]) < self.lim["wants"]))
        m = self.mind(b)
        m["wants"][t] = w
        return {"being": b, "topic": t, "topics": len(m["knows"]), "wants": len(m["wants"]), "weight": w}

    def op_bond(self, a: dict) -> dict:
        keys(a, ["a", "b", "warmth"], [])
        x, y = ident(a["a"]), ident(a["b"])
        w = sint(a["warmth"], -100, 100)
        k = pair(x, y)
        need(x != y and (k in self.bonds or len(self.bonds) < self.lim["bonds"]))
        self.bonds[k] = w
        return {"a": k[0], "b": k[1], "warmth": w}

    def offer(self, speaker: str, listener: str, warmth: int, said: set) -> Optional[tuple]:
        sk, lm = self.minds[speaker]["knows"], self.minds[listener]
        best = None
        for t, (v, w, s) in sk.items():
            if t in said or (s and warmth < SECRET_WARMTH):
                continue
            if t in lm["knows"] and lm["knows"][t][0] == v:
                continue
            score = lm["wants"].get(t, 0) * 2 + w
            key = (-score, t)
            if best is None or key < best[0]:
                best = (key, t, v, w, s)
        return best

    def op_meet(self, a: dict) -> dict:
        keys(a, ["a", "b", "seed"], ["turns", "at"])
        x, y = ident(a["a"]), ident(a["b"])
        seed = uint(a["seed"], 0, MAX_AT)
        turns = opt_uint(a, "turns", 1, self.lim["turns"], self.lim["turns"])
        at = opt_uint(a, "at", 0, MAX_AT, 0)
        need(x != y and x in self.minds and y in self.minds)
        k = pair(x, y)
        before = self.bonds.get(k, 0)
        first = x if fnv1a(f"{seed}:{x}:{y}") % 2 == 0 else y
        second = y if first == x else x
        warmth = before
        said: set = set()
        log, effects = [], []
        disputes = shared_wants = 0
        quiet = 0
        learned: List[tuple] = []  # (listener, topic, value, weight, secret)
        room = {b: self.lim["topics"] - len(self.minds[b]["knows"]) for b in (x, y)}
        for i in range(turns):
            sp, li = (first, second) if i % 2 == 0 else (second, first)
            o = self.offer(sp, li, warmth, said)
            if o is None:
                log.append({"kind": "small_talk", "speaker": sp, "topic": None, "value": None})
                quiet += 1
                if quiet >= 2:
                    break
                continue
            quiet = 0
            _, t, v, w, s = o
            said.add(t)
            lk = self.minds[li]["knows"]
            if t in lk:
                disputes += 1
                log.append({"kind": "dispute", "speaker": sp, "topic": t, "value": v})
                continue
            wanted = t in self.minds[li]["wants"]
            took = room[li] > 0
            if took:
                room[li] -= 1
                learned.append((li, t, v, w // 2, s))
                effects.append({"engine": "beliefs", "op": "learn",
                                "args": {"being": li, "source": "told", "about": t, "relation": "is", "value": v,
                                         "from": sp}})
            if wanted and took:
                shared_wants += 1
                warmth = min(100, warmth + WANT_WARMTH)
                effects.append({"engine": "relationship", "op": "appraise_action",
                                "args": {"actor": sp, "target": li, "action": "help", "success": True}})
            log.append({"kind": "tell", "speaker": sp, "topic": t, "value": v})
        outcome = "small_talk"
        if disputes and before < QUARREL_BELOW:
            outcome = "quarrel"
            warmth = max(-100, warmth - QUARREL_COST * disputes)
            for p, q in ((x, y), (y, x)):
                effects.append({"engine": "affect", "op": "appraise",
                                "args": {"being": p, "subject": q, "kind": "resentment", "delta": min(100, QUARREL_COST * disputes),
                                         "reason": "quarrel"}})
        elif any(e["kind"] == "tell" for e in log):
            outcome = "alliance" if shared_wants and warmth >= ALLY_AT else "gossip"
        need(warmth == before or k in self.bonds or len(self.bonds) < self.lim["bonds"])
        for li, t, v, w, s in learned:
            self.minds[li]["knows"][t] = [v, w, s]
            self.minds[li]["wants"].pop(t, None)
        if warmth != before:
            self.bonds[k] = warmth
        self.seq += 1
        rec = {"id": self.seq, "a": x, "b": y, "at": at, "first": first, "outcome": outcome, "turns": log,
               "warmth_after": warmth, "warmth_before": before}
        self.records.append(rec)
        if len(self.records) > self.lim["records"]:
            del self.records[: len(self.records) - self.lim["records"]]
        return {"effects": effects, "record": rec}

    def op_log(self, a: dict) -> dict:
        keys(a, [], ["being", "limit"])
        b = None if a.get("being") is None else ident(a["being"])
        n = opt_uint(a, "limit", 1, 64, 16)
        rows = [r for r in self.records if b is None or b in (r["a"], r["b"])]
        return {"records": rows[-n:][::-1]}

    def op_assess(self, a: dict) -> dict:
        keys(a, ["being"], [])
        b, _ = self.known(a)
        bonds = [{"other": q if p == b else p, "warmth": w} for (p, q), w in sorted(self.bonds.items()) if b in (p, q)]
        return {**self.mind_view(b), "bonds": bonds}

    def workflow(self, request: Any) -> dict:
        table = {"know": self.op_know, "want": self.op_want, "bond": self.op_bond, "meet": self.op_meet,
                 "log": self.op_log, "assess": self.op_assess}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Chatter().workflow(request)
