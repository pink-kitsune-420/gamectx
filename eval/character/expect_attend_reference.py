"""Pure-Python reference for expectations and surprise (pass 2 A3, local kind `expectation`)
and attention and graded noticing (pass 2 A2, local kind `attention`).

Written independently of src/character/expectation.rs and src/character/attention.rs;
expect_attend_cases.py checks they agree on conformance cases and randomized workflows.
"""
from typing import Any, Dict, List, Optional, Tuple

from beliefs_speech_reference import Bad, blist, ident, keys, need, opt_bool, opt_uint, run_ops, text, uint

MAX_AT = 1_000_000_000_000
EXP_BEINGS = 64
MAX_EXPECTATIONS = 32
MAX_SUBJECTS = 64
MAX_GRACE = 1_000_000

MAX_EVENTS = 16
MAX_NOTICE_BEINGS = 64
MAX_PROFILES = 64
MAX_SEED = 2 ** 53 - 1
MASK = 2 ** 64 - 1


# ---------------------------------------------------------------- expectations (A3)

def set_mood(being: str, delta: int, reason: str) -> Dict[str, Any]:
    return {"args": {"being": being, "reason": reason, "valence_delta": delta}, "engine": "affect", "op": "set_mood"}


def resentment(being: str, subject: str, delta: int, reason: str) -> Dict[str, Any]:
    return {"args": {"being": being, "delta": delta, "kind": "resentment", "reason": reason, "subject": subject},
            "engine": "affect", "op": "appraise"}


class ExpectRef:
    def __init__(self) -> None:
        # being -> {"exp": {id: dict}, "rel": {about: [kept, broken]}}
        self.beings: Dict[str, Dict[str, Any]] = {}

    def run(self, request: Any) -> Dict[str, Any]:
        table = {"expect": self.expect, "tick": self.tick, "happen": self.happen, "cancel": self.cancel,
                 "assess": self.assess}

        def f(op, a):
            need(op in table)
            return table[op](a)
        return run_ops(request, f)

    @staticmethod
    def confidence(mind: Dict[str, Any], about: str) -> int:
        kept, broken = mind["rel"].get(about, [0, 0])
        return max(10, min(95, 70 + 10 * kept - 15 * broken))

    @staticmethod
    def score(mind: Dict[str, Any], about: str, kept: bool) -> None:
        rel = mind["rel"]
        if about not in rel and len(rel) >= MAX_SUBJECTS:
            victim = min(rel, key=lambda k: (rel[k][0] + rel[k][1], k))
            del rel[victim]
        entry = rel.setdefault(about, [0, 0])
        entry[0 if kept else 1] += 1

    def targets(self, a: Dict[str, Any]) -> List[str]:
        if a.get("being") is not None:
            b = ident(a["being"])
            need(b in self.beings)
            return [b]
        return sorted(self.beings)

    def expect(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being", "id", "about", "event", "due", "importance"],
             ["grace", "kind", "relies", "confidence", "label"])
        being, eid = ident(a["being"]), ident(a["id"])
        about, event = ident(a["about"]), ident(a["event"])
        due = uint(a["due"], 0, MAX_AT)
        importance = uint(a["importance"], 1, 100)
        grace = opt_uint(a, "grace", 0, MAX_GRACE, 0)
        kind = "hope" if a.get("kind") is None else text(a["kind"], 8)
        need(kind in ("hope", "dread"))
        relies = opt_bool(a, "relies", False)
        conf = None if a.get("confidence") is None else uint(a["confidence"], 1, 100)
        label = f"{about} {event}" if a.get("label") is None else text(a["label"], 80)
        need(being in self.beings or len(self.beings) < EXP_BEINGS)
        mind = self.beings.setdefault(being, {"exp": {}, "rel": {}})
        exp = mind["exp"]
        if eid not in exp and len(exp) >= MAX_EXPECTATIONS:
            broken = [k for k in exp if exp[k]["state"] == "broken"]
            need(bool(broken))
            del exp[min(broken, key=lambda k: (exp[k]["due"], k))]
        if conf is None:
            conf = self.confidence(mind, about)
        exp[eid] = {"about": about, "event": event, "due": due, "grace": grace, "importance": importance,
                    "dread": kind == "dread", "relies": relies, "confidence": conf, "label": label, "state": "open"}
        return {"being": being, "confidence": conf, "due_by": due + grace, "id": eid, "open": len(exp)}

    def tick(self, a: Any) -> Dict[str, Any]:
        keys(a, ["at"], ["being"])
        at = uint(a["at"], 0, MAX_AT)
        changes: List[Dict[str, Any]] = []
        effects: List[Dict[str, Any]] = []
        for name in self.targets(a):
            mind = self.beings[name]
            gone, scores = [], []
            for eid in sorted(mind["exp"]):
                e = mind["exp"][eid]
                if e["state"] == "broken" or at <= e["due"]:
                    continue
                surprise = e["importance"] * e["confidence"] // 100
                why = "expected " + e["label"]
                if at > e["due"] + e["grace"]:
                    scores.append(e["about"])
                    if e["dread"]:
                        gone.append(eid)
                        if surprise // 4:
                            effects.append(set_mood(name, surprise // 4, why))
                        outcome = "relieved"
                    else:
                        e["state"] = "broken"
                        hurt = max(1, surprise // 2)
                        effects.append(set_mood(name, -hurt, why))
                        if e["relies"]:
                            effects.append(resentment(name, e["about"], hurt, why))
                        outcome = "disappointed"
                elif e["state"] == "open" and not e["dread"]:
                    e["state"] = "worried"
                    if surprise // 4:
                        effects.append(set_mood(name, -(surprise // 4), why))
                    outcome = "worried"
                else:
                    continue
                changes.append({"about": e["about"], "being": name, "event": e["event"], "id": eid,
                                "outcome": outcome, "surprise": surprise})
            for eid in gone:
                del mind["exp"][eid]
            for about in scores:
                self.score(mind, about, False)
        return {"at": at, "changes": changes, "effects": effects}

    def happen(self, a: Any) -> Dict[str, Any]:
        keys(a, ["about", "event", "at"], ["being"])
        about, event = ident(a["about"]), ident(a["event"])
        at = uint(a["at"], 0, MAX_AT)
        resolved: List[Dict[str, Any]] = []
        effects: List[Dict[str, Any]] = []
        for name in self.targets(a):
            mind = self.beings[name]
            for eid in sorted(k for k, e in mind["exp"].items() if e["about"] == about and e["event"] == event):
                e = mind["exp"].pop(eid)
                why = "expected " + e["label"]
                surprise = 0
                if e["state"] == "broken":
                    effects.append(set_mood(name, max(1, e["importance"] // 4), why))
                    outcome = "late"
                    surprise = e["importance"] * e["confidence"] // 100
                elif e["dread"]:
                    self.score(mind, about, True)
                    effects.append(set_mood(name, -max(1, e["importance"] // 2), why))
                    outcome = "dread_met"
                else:
                    self.score(mind, about, True)
                    effects.append(set_mood(name, max(1, e["importance"] // 10), why))
                    outcome = "kept"
                resolved.append({"being": name, "early": at < e["due"], "id": eid, "outcome": outcome,
                                 "surprise": surprise})
        return {"about": about, "at": at, "effects": effects, "event": event, "resolved": resolved}

    def cancel(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being", "id"], [])
        being, eid = ident(a["being"]), ident(a["id"])
        mind = self.beings.get(being)
        removed = mind is not None and mind["exp"].pop(eid, None) is not None
        return {"being": being, "id": eid, "removed": removed}

    def assess(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being"], [])
        being = ident(a["being"])
        need(being in self.beings)
        mind = self.beings[being]
        exps = []
        for eid in sorted(mind["exp"]):
            e = mind["exp"][eid]
            exps.append({"about": e["about"], "confidence": e["confidence"], "due": e["due"], "event": e["event"],
                         "grace": e["grace"], "id": eid, "importance": e["importance"],
                         "kind": "dread" if e["dread"] else "hope", "label": e["label"], "relies": e["relies"],
                         "state": e["state"]})
        rel = [{"about": k, "broken": v[1], "confidence": self.confidence(mind, k), "kept": v[0]}
               for k, v in sorted(mind["rel"].items())]
        return {"being": being, "expectations": exps, "reliability": rel}


# ---------------------------------------------------------------- attention (A2)

def fnv1a(s: str) -> int:
    h = 0xCBF29CE484222325
    for byte in s.encode("utf-8"):
        h ^= byte
        h = (h * 0x100000001B3) & MASK
    return h


DEFAULT_PROFILE = {"alertness": 50, "perception": 0, "cares": [], "capacity": 3}


def profile_view(being: str, p: Dict[str, Any]) -> Dict[str, Any]:
    return {"alertness": p["alertness"], "being": being, "capacity": p["capacity"], "cares": list(p["cares"]),
            "perception": p["perception"]}


class AttendRef:
    def __init__(self) -> None:
        self.profiles: Dict[str, Dict[str, Any]] = {}

    def run(self, request: Any) -> Dict[str, Any]:
        table = {"profile": self.profile, "notice": self.notice, "assess": self.assess}

        def f(op, a):
            need(op in table)
            return table[op](a)
        return run_ops(request, f)

    def profile(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being"], ["alertness", "perception", "cares", "capacity"])
        being = ident(a["being"])
        cares = sorted({ident(x) for x in blist(a.get("cares"), 16)})
        p = {"alertness": opt_uint(a, "alertness", 0, 100, 50), "perception": opt_uint(a, "perception", 0, 100, 0),
             "cares": cares, "capacity": opt_uint(a, "capacity", 1, MAX_EVENTS, 3)}
        need(being in self.profiles or len(self.profiles) < MAX_PROFILES)
        self.profiles[being] = p
        return profile_view(being, p)

    def notice(self, a: Any) -> Dict[str, Any]:
        keys(a, ["seed", "events", "beings"], ["at"])
        seed = uint(a["seed"], 0, MAX_SEED)
        at = opt_uint(a, "at", 0, MAX_AT, 0)
        need(isinstance(a["events"], list) and 1 <= len(a["events"]) <= MAX_EVENTS)
        need(isinstance(a["beings"], list) and 1 <= len(a["beings"]) <= MAX_NOTICE_BEINGS)
        events: List[Dict[str, Any]] = []
        for e in a["events"]:
            keys(e, ["id", "kind", "place", "salience"], ["noise", "subject"])
            eid = ident(e["id"])
            need(all(x["id"] != eid for x in events))
            events.append({"id": eid, "kind": ident(e["kind"]), "place": ident(e["place"]),
                           "salience": uint(e["salience"], 0, 100), "noise": opt_uint(e, "noise", 0, 100, 0),
                           "subject": None if e.get("subject") is None else ident(e["subject"])})
        by: List[List[str]] = [[] for _ in events]
        out = []
        seen = set()
        for b in a["beings"]:
            keys(b, ["id", "place"], ["near", "light", "busy", "asleep", "sight", "hearing"])
            bid = ident(b["id"])
            need(bid not in seen)
            seen.add(bid)
            place = ident(b["place"])
            near: Dict[str, int] = {}
            for n in blist(b.get("near"), 16):
                keys(n, ["place", "distance"], [])
                p = ident(n["place"])
                need(p != place and p not in near)
                near[p] = uint(n["distance"], 1, 1000)
            light = opt_uint(b, "light", 0, 100, 100)
            busy = opt_uint(b, "busy", 0, 100, 0)
            asleep = opt_bool(b, "asleep", False)
            sight = opt_bool(b, "sight", True)
            hearing = opt_bool(b, "hearing", True)
            prof = self.profiles.get(bid, DEFAULT_PROFILE)
            hits: List[Tuple[int, int]] = []
            missed: List[Tuple[int, str, int]] = []
            for i, e in enumerate(events):
                if e["place"] == place:
                    dist = 0
                elif e["place"] in near:
                    dist = near[e["place"]]
                else:
                    missed.append((i, "out_of_range", 0))
                    continue
                visual = e["salience"] * light // 100 if sight and not asleep else 0
                audible = e["noise"] if hearing else 0
                base = max(visual, audible)
                cares = e["kind"] in prof["cares"] or (e["subject"] is not None and e["subject"] in prof["cares"])
                raw = (base + prof["alertness"] // 5 + prof["perception"] // 5 + (25 if cares else 0)
                       - (0 if asleep else busy // 3) - dist // 5)
                score = max(0, min(100, raw))
                if asleep:
                    score //= 2
                if base == 0:
                    score = 0
                roll = fnv1a(f"{seed}:{bid}:{e['id']}") % 100
                if score == 0:
                    missed.append((i, "imperceptible", 0))
                elif roll >= score:
                    missed.append((i, "overlooked", score))
                else:
                    hits.append((score, i))
            hits.sort(key=lambda h: (-h[0], events[h[1]]["id"]))
            noticed = []
            for n, (score, i) in enumerate(hits):
                if n >= prof["capacity"]:
                    missed.append((i, "over_capacity", score))
                    continue
                by[i].append(bid)
                noticed.append({"clarity": "clear" if score >= 60 else "vague", "event": events[i]["id"],
                                "score": score})
            missed.sort(key=lambda m: m[0])
            out.append({"being": bid, "missed": [{"event": events[i]["id"], "reason": r, "score": s}
                                                 for i, r, s in missed], "noticed": noticed})
        return {"at": at, "beings": out,
                "events": [{"event": e["id"], "noticed_by": by[i]} for i, e in enumerate(events)], "seed": seed}

    def assess(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being"], [])
        being = ident(a["being"])
        return profile_view(being, self.profiles.get(being, DEFAULT_PROFILE))


def process_local(kind: str, args: Any) -> Dict[str, Any]:
    if kind == "expectation":
        return ExpectRef().run(args)
    if kind == "attention":
        return AttendRef().run(args)
    raise Bad(f"unknown local kind {kind}")
