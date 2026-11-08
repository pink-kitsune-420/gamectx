"""Pure-Python reference for subjective world knowledge (pass 2, A1, local kind `beliefs`)
and communication as action (pass 2, B1, local kind `speech_act`).

Written independently of src/character/beliefs.rs and src/character/speech_act.rs; the
differential runner in beliefs_speech_cases.py checks the two agree on conformance cases
and randomized workflows.
"""
from typing import Any, Dict, List, Optional, Tuple

MAX_OPS = 64
MAX_BELIEFS = 128
MAX_BEINGS = 64
MAX_LIST = 64
MAX_AT = 1_000_000_000_000
SOURCES = ["authored", "told", "saw", "did"]
DEFAULT_CONF = {"did": 100, "saw": 90, "authored": 80, "told": 60}

ACTS = ["ask", "command", "comfort", "inform", "persuade", "request", "threaten", "warn"]
INTENTS = ["alarm", "get_help", "learn", "reassure", "share"]
MAX_PAIRS = 256
MAX_WANTS = 32
MAX_WANT_BEINGS = 64
SURE = 50
IDENT = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_:-")


class Bad(ValueError):
    pass


def need(cond: bool) -> None:
    if not cond:
        raise Bad("invalid or out-of-bounds runtime value")


def keys(v: Any, req: List[str], optional: List[str]) -> dict:
    need(isinstance(v, dict))
    need(all(k in v for k in req))
    need(all(k in req or k in optional for k in v))
    return v


def ident(v: Any) -> str:
    need(isinstance(v, str) and 0 < len(v) <= 64 and all(ch in IDENT for ch in v))
    return v


def text(v: Any, n: int) -> str:
    need(isinstance(v, str) and 0 < len(v) <= n)
    need(not any(ord(ch) < 32 or 127 <= ord(ch) <= 159 for ch in v))
    return v


def uint(v: Any, lo: int, hi: int) -> int:
    need(isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi)
    return v


def opt_uint(d: dict, k: str, lo: int, hi: int, default: int) -> int:
    return default if d.get(k) is None else uint(d[k], lo, hi)


def opt_bool(d: dict, k: str, default: bool) -> bool:
    v = d.get(k)
    if v is None:
        return default
    need(isinstance(v, bool))
    return v


def blist(v: Any, n: int) -> list:
    if v is None:
        return []
    need(isinstance(v, list) and len(v) <= n)
    return v


def rank(s: str) -> int:
    return SOURCES.index(s) + 1 if s in SOURCES else 0


Triple = Tuple[str, str, str]


def triple(v: dict) -> Triple:
    t = (ident(v.get("about")), ident(v.get("relation")), ident(v.get("value")))
    need(len(key(t)) <= 64)
    return t


def key(t: Triple) -> str:
    return f"{t[0]}:{t[1]}:{t[2]}"


def view(t: Triple, b: dict) -> dict:
    return {
        "about": t[0], "at": b["at"], "confidence": b["confidence"], "from": b["from"],
        "holds": b["holds"], "key": key(t), "relation": t[1], "source": b["source"], "value": t[2],
    }


def run_ops(request: Any, f) -> dict:
    keys(request, ["ops"], [])
    ops = request["ops"]
    need(isinstance(ops, list) and 0 < len(ops) <= MAX_OPS)
    out = []
    for item in ops:
        keys(item, ["op"], ["args"])
        name = text(item["op"], 32)
        args = item.get("args")
        out.append(f(name, {} if args is None else args))
    return {"version": 1, "results": out}


from limits_reference import Limits, limited  # noqa: E402


class BeliefsRef:
    def __init__(self) -> None:
        self.limits = Limits([("beings", 64, 1_000_000), ("beliefs", 128, 4096)])
        self.beings: Dict[str, Dict[Triple, dict]] = {}

    def get(self, being: str, t: Triple) -> Optional[dict]:
        return self.beings.get(being, {}).get(t)

    def room(self, being: str) -> None:
        need(being in self.beings or len(self.beings) < self.limits["beings"])

    def learn(self, being: str, t: Triple, b: dict) -> Tuple[str, dict]:
        self.room(being)
        m = self.beings.setdefault(being, {})
        old = m.get(t)
        if old is None:
            if len(m) >= self.limits["beliefs"]:
                victim = min(m, key=lambda k: (m[k]["confidence"], m[k]["at"], k))
                del m[victim]
            m[t] = dict(b)
            res = "new"
        elif old["holds"] == b["holds"]:
            old["confidence"] = max(old["confidence"], b["confidence"])
            if rank(b["source"]) > rank(old["source"]):
                old["source"], old["from"] = b["source"], b["from"]
            old["at"] = max(old["at"], b["at"])
            res = "reinforced"
        elif rank(b["source"]) >= rank(old["source"]):
            m[t] = dict(b)
            res = "revised"
        else:
            res = "kept"
        return res, m[t]

    def learn_view(self, being: str, t: Triple, b: dict) -> dict:
        res, now = self.learn(being, t, b)
        return {"belief": view(t, now), "key": key(t), "result": res}

    def dispatch(self, op: str, a: Any):
        fn = {
            "init": self.init, "learn": self.op_learn, "outcome": self.outcome, "query": self.query,
            "known_actions": self.known_actions, "gaps": self.gaps,
        }.get(op)
        return None if fn is None else fn(a)

    def run(self, request: Any) -> dict:
        return limited(request, self.limits, self.run_inner)

    def run_inner(self, request: Any) -> dict:
        def f(op, a):
            r = self.dispatch(op, a)
            need(r is not None)
            return r
        return run_ops(request, f)

    @staticmethod
    def read(v: dict, default_source: str, frm, at: int) -> Tuple[Triple, dict]:
        t = triple(v)
        src = v.get("source")
        if src is None:
            need(default_source != "")
            src = default_source
        else:
            src = text(src, 16)
        need(rank(src) > 0)
        return t, {
            "holds": opt_bool(v, "holds", True), "source": src,
            "confidence": opt_uint(v, "confidence", 0, 100, DEFAULT_CONF[src]), "from": frm, "at": at,
        }

    def init(self, a: Any) -> dict:
        keys(a, ["being"], ["beliefs"])
        being = ident(a["being"])
        fresh = []
        for b in blist(a.get("beliefs"), MAX_LIST):
            keys(b, ["about", "relation", "value"], ["holds", "source", "confidence"])
            fresh.append(self.read(b, "authored", None, 0))
        self.room(being)
        self.beings[being] = {}
        for t, b in fresh:
            self.learn(being, t, b)
        return {"being": being, "count": len(self.beings[being])}

    def op_learn(self, a: Any) -> dict:
        keys(a, ["being", "about", "relation", "value", "source"], ["holds", "confidence", "from", "at"])
        being = ident(a["being"])
        frm = None if a.get("from") is None else ident(a["from"])
        at = opt_uint(a, "at", 0, MAX_AT, 0)
        t, b = self.read(a, "", frm, at)
        out = self.learn_view(being, t, b)
        out["being"] = being
        return out

    def outcome(self, a: Any) -> dict:
        keys(a, ["being", "action", "success"], ["relied_on", "at"])
        being = ident(a["being"])
        action = ident(a["action"])
        success = a["success"]
        need(isinstance(success, bool))
        at = opt_uint(a, "at", 0, MAX_AT, 0)
        relied = []
        for r in blist(a.get("relied_on"), 8):
            keys(r, ["about", "relation", "value"], [])
            relied.append(triple(r))
        can = (being, "can", action)
        need(len(key(can)) <= 64)

        def did(holds):
            return {"holds": holds, "source": "did", "confidence": 100, "from": None, "at": at}
        updates = []
        if success:
            updates.append(self.learn_view(being, can, did(True)))
        for t in relied:
            updates.append(self.learn_view(being, t, did(success)))
        return {"action": action, "being": being, "success": success, "updates": updates}

    def query(self, a: Any) -> dict:
        keys(a, ["being"], ["about", "relation", "value", "min_confidence"])
        being = ident(a["being"])
        filt = [None if a.get(k) is None else ident(a[k]) for k in ("about", "relation", "value")]
        floor = opt_uint(a, "min_confidence", 0, 100, 0)
        m = self.beings.get(being, {})
        out = [view(t, m[t]) for t in sorted(m)
               if all(f is None or f == t[i] for i, f in enumerate(filt)) and m[t]["confidence"] >= floor]
        return {"being": being, "beliefs": out, "count": len(out)}

    def known_actions(self, a: Any) -> dict:
        keys(a, ["being", "actions"], [])
        being = ident(a["being"])
        need(isinstance(a["actions"], list) and len(a["actions"]) <= MAX_LIST)
        known, unknown = [], []
        for act in a["actions"]:
            act = ident(act)
            t = (being, "can", act)
            need(len(key(t)) <= 64)
            b = self.get(being, t)
            (known if b is not None and b["holds"] else unknown).append(act)
        return {"being": being, "known": known, "unknown": unknown}

    def gaps(self, a: Any) -> dict:
        keys(a, ["being", "needs"], ["min_confidence"])
        being = ident(a["being"])
        floor = opt_uint(a, "min_confidence", 0, 100, 50)
        need(isinstance(a["needs"], list) and len(a["needs"]) <= 16)
        needs, gaps = [], 0
        for n in a["needs"]:
            keys(n, ["about", "relation", "value"], [])
            t = triple(n)
            b = self.get(being, t)
            if b is None:
                status, conf = "unknown", None
            elif b["confidence"] < floor:
                status, conf = "low_confidence", b["confidence"]
            else:
                status, conf = ("known_true" if b["holds"] else "known_false"), b["confidence"]
            gaps += status in ("unknown", "low_confidence")
            needs.append({"confidence": conf, "key": key(t), "status": status})
        return {"being": being, "gaps": gaps, "needs": needs}


def mind_knows(subject, other, fact, status, source, at):
    return {"args": {"at": at, "category": "knows", "fact": fact, "other": other, "source": source,
                     "status": status, "subject": subject}, "kind": "mind_record"}


def feels(holder, toward, emotion, delta):
    return {"delta": delta, "emotion": emotion, "holder": holder, "kind": "feels", "toward": toward}


class SpeechRef:
    def __init__(self) -> None:
        self.limits = Limits([("pairs", 256, 1_000_000), ("want_beings", 64, 1_000_000), ("wants", 32, 1024), ("beings", 64, 1_000_000), ("beliefs", 128, 4096)])
        self.beliefs = BeliefsRef()
        self.pairs: Dict[Tuple[str, str], Dict[str, int]] = {}
        self.wants: Dict[str, Dict[str, Dict[str, Any]]] = {}

    def pair(self, s: str, l: str) -> Dict[str, int]:
        return dict(self.pairs.get((s, l), {"trust": 50, "authority": 0}))

    def pair_room(self, s: str, l: str) -> None:
        need((s, l) in self.pairs or len(self.pairs) < self.limits["pairs"])

    def run(self, request: Any) -> dict:
        return limited(request, self.limits, self.run_inner)

    def run_inner(self, request: Any) -> dict:
        self.beliefs.limits.copy_shared(self.limits)
        def f(op, a):
            if op == "relate":
                return self.relate(a)
            if op == "check":
                act = read_act(a)
                failed = self.preconditions(act)
                return {"act": act["act"], "failed": failed, "listener": act["listener"],
                        "ok": not failed, "speaker": act["speaker"]}
            if op == "perform":
                return self.perform(a)
            if op == "plan":
                return self.plan(a)
            if op == "assess":
                return self.assess(a)
            r = self.beliefs.dispatch(op, a)
            need(r is not None)
            return r
        return run_ops(request, f)

    def relate(self, a: Any) -> dict:
        keys(a, ["speaker", "listener"], ["trust", "authority"])
        s, l = ident(a["speaker"]), ident(a["listener"])
        need(s != l)
        cur = self.pair(s, l)
        nxt = {"trust": opt_uint(a, "trust", 0, 100, cur["trust"]),
               "authority": opt_uint(a, "authority", 0, 100, cur["authority"])}
        self.pair_room(s, l)
        self.pairs[(s, l)] = nxt
        return {"authority": nxt["authority"], "listener": l, "speaker": s, "trust": nxt["trust"]}

    def preconditions(self, a: dict) -> List[str]:
        p = self.pair(a["speaker"], a["listener"])
        mine = None if a["topic"] is None else self.beliefs.get(a["speaker"], a["topic"])
        act = a["act"]
        if act in ("inform", "warn"):
            if mine is None:
                return ["speaker_does_not_know"]
            if mine["confidence"] < SURE:
                return ["speaker_unsure"]
        elif act == "ask" and mine is not None and mine["confidence"] >= SURE:
            return ["already_known"]
        elif act == "command" and p["authority"] < 50:
            return ["no_authority"]
        elif act == "comfort" and p["trust"] < 30:
            return ["not_trusted"]
        return []

    def add_want(self, holder: str, goal: str, priority: int, frm: str) -> None:
        need(holder in self.wants or len(self.wants) < self.limits["want_beings"])
        ws = self.wants.setdefault(holder, {})
        if goal in ws:
            if priority >= ws[goal]["priority"]:
                ws[goal] = {"priority": priority, "from": frm}
        else:
            if len(ws) >= self.limits["wants"]:
                victim = min(ws, key=lambda g: (ws[g]["priority"], g))
                del ws[victim]
            ws[goal] = {"priority": priority, "from": frm}

    def set_trust(self, a: dict, value: int, effects: list) -> None:
        s, l = a["speaker"], a["listener"]
        old = self.pair(s, l)
        self.pairs[(s, l)] = {"trust": value, "authority": old["authority"]}
        effects.append({"delta": value - old["trust"], "holder": l, "kind": "trust", "toward": s, "value": value})

    def perform(self, args: Any) -> dict:
        a = read_act(args)
        failed = self.preconditions(a)
        effects: list = []
        if failed:
            outcome = "blocked"
        else:
            if a["act"] in ("threaten", "comfort"):
                self.pair_room(a["speaker"], a["listener"])
            outcome = self.apply(a, effects)
        return {"act": a["act"], "effects": effects, "failed": failed, "listener": a["listener"],
                "outcome": outcome, "performed": not failed, "speaker": a["speaker"]}

    def apply(self, a: dict, effects: list) -> str:
        s, l, act, at = a["speaker"], a["listener"], a["act"], a["at"]
        p = self.pair(s, l)
        if act in ("inform", "warn"):
            t = a["topic"]
            mine = dict(self.beliefs.get(s, t))
            conf = max(1, mine["confidence"] * p["trust"] // 100)
            res, _ = self.beliefs.learn(l, t, {"holds": mine["holds"], "source": "told", "confidence": conf,
                                              "from": s, "at": at})
            effects.append({"holder": l, "key": key(t), "kind": "belief", "result": res})
            effects.append(mind_knows(s, l, key(t), True, "inferred", at))
            if act == "warn":
                effects.append(feels(l, t[0], "fear", conf // 2))
                return "warned"
            return "informed"
        if act == "ask":
            t = a["topic"]
            theirs = self.beliefs.get(l, t)
            if theirs is None:
                effects.append(mind_knows(s, l, key(t), False, "told", at))
                return "no_answer"
            theirs = dict(theirs)
            conf = max(1, theirs["confidence"] * self.pair(l, s)["trust"] // 100)
            res, _ = self.beliefs.learn(s, t, {"holds": theirs["holds"], "source": "told", "confidence": conf,
                                              "from": l, "at": at})
            effects.append({"holder": s, "key": key(t), "kind": "belief", "result": res})
            effects.append(mind_knows(l, s, key(t), True, "inferred", at))
            return "answered"
        if act in ("request", "command", "persuade", "threaten"):
            if act == "request":
                complied, prio = p["trust"] >= 40, p["trust"]
            elif act == "command":
                complied, prio = True, p["authority"]
            elif act == "persuade":
                prio = (p["trust"] + a["strength"]) // 2
                complied = prio >= 50
            else:
                fear = min(100, 30 + a["strength"] // 2)
                effects.append(feels(l, s, "fear", fear))
                self.set_trust(a, max(0, p["trust"] - 20), effects)
                complied, prio = fear >= 50, fear
            if not complied:
                return "refused"
            self.add_want(l, a["goal"], prio, s)
            effects.append({"from": s, "goal": a["goal"], "holder": l, "kind": "wants", "priority": prio})
            effects.append({"args": {"at": at, "category": "wants", "goal": a["goal"], "other": l,
                                     "priority": prio, "source": "inferred", "subject": s},
                            "kind": "mind_record"})
            return "complied"
        effects.append(feels(l, s, "fear", -(p["trust"] // 2)))
        effects.append(feels(l, s, "gratitude", p["trust"] // 4))
        self.set_trust(a, min(100, p["trust"] + 5), effects)
        return "comforted"

    def plan(self, a: Any) -> dict:
        keys(a, ["speaker", "intent", "candidates"], ["topic", "goal", "strength"])
        s = ident(a["speaker"])
        intent = text(a["intent"], 16)
        need(intent in INTENTS)
        raw = a["candidates"]
        need(isinstance(raw, list) and len(raw) <= 16)
        cands: List[str] = []
        for x in raw:
            x = ident(x)
            need(x != s and x not in cands)
            cands.append(x)
        need(len(cands) > 0)
        cands.sort()
        wants_topic = intent in ("learn", "share", "alarm")
        topic = None
        if a.get("topic") is not None:
            need(wants_topic)
            keys(a["topic"], ["about", "relation", "value"], [])
            topic = triple(a["topic"])
        else:
            need(not wants_topic)
        if a.get("goal") is not None:
            need(intent == "get_help")
            ident(a["goal"])
        else:
            need(intent != "get_help")
        need(a.get("strength") is None or intent == "get_help")
        strength = opt_uint(a, "strength", 0, 100, 50)

        def step(act, who, e, likely):
            return {"act": act, "expected": e, "likely": likely, "listener": who}
        mine = None if topic is None else self.beliefs.get(s, topic)
        steps, reason = [], "ok"
        if intent == "learn":
            if mine is not None and mine["confidence"] >= SURE:
                reason = "already_known"
            else:
                who = min(cands, key=lambda c: (-self.pair(c, s)["trust"], c))
                t = self.pair(who, s)["trust"]
                steps.append(step("ask", who, t, t >= SURE))
        elif intent in ("share", "alarm"):
            if mine is None:
                reason = "speaker_does_not_know"
            elif mine["confidence"] < SURE:
                reason = "speaker_unsure"
            else:
                act = "inform" if intent == "share" else "warn"
                for c in cands:
                    e = max(1, mine["confidence"] * self.pair(s, c)["trust"] // 100)
                    steps.append(step(act, c, e, e >= SURE))
        elif intent == "get_help":
            best = None
            for c in cands:
                p = self.pair(s, c)
                if p["authority"] >= 50:
                    cand = ("command", p["authority"], True)
                elif p["trust"] >= 40:
                    cand = ("request", p["trust"], True)
                else:
                    sc = (p["trust"] + strength) // 2
                    cand = ("persuade", sc, sc >= 50)
                if best is None or cand[1] > best[2]:
                    best = (c, cand[0], cand[1], cand[2])
            steps.append(step(best[1], best[0], best[2], best[3]))
        else:
            ok = [c for c in cands if self.pair(s, c)["trust"] >= 30]
            if ok:
                who = min(ok, key=lambda c: (-self.pair(s, c)["trust"], c))
                steps.append(step("comfort", who, self.pair(s, who)["trust"], True))
            else:
                reason = "no_trusted_listener"
        return {"count": len(steps), "intent": intent, "reason": reason, "speaker": s, "steps": steps}

    def assess(self, a: Any) -> dict:
        keys(a, ["being"], [])
        being = ident(a["being"])
        trusts = [{"authority": p["authority"], "speaker": sp, "trust": p["trust"]}
                  for (sp, l), p in sorted(self.pairs.items()) if l == being]
        ws = self.wants.get(being, {})
        wants = [{"from": ws[g]["from"], "goal": g, "priority": ws[g]["priority"]} for g in sorted(ws)]
        return {"being": being, "beliefs": len(self.beliefs.beings.get(being, {})), "trusts": trusts,
                "wants": wants}


def read_act(a: Any) -> dict:
    keys(a, ["speaker", "listener", "act"], ["topic", "goal", "strength", "at"])
    s, l = ident(a["speaker"]), ident(a["listener"])
    act = text(a["act"], 16)
    need(s != l and act in ACTS)
    needs_topic = act in ("inform", "warn", "ask")
    needs_goal = act in ("request", "command", "persuade", "threaten")
    topic = None
    if a.get("topic") is not None:
        need(needs_topic)
        keys(a["topic"], ["about", "relation", "value"], [])
        topic = triple(a["topic"])
    else:
        need(not needs_topic)
    goal = None
    if a.get("goal") is not None:
        need(needs_goal)
        goal = ident(a["goal"])
    else:
        need(not needs_goal)
    need(a.get("strength") is None or act in ("persuade", "threaten"))
    return {"speaker": s, "listener": l, "act": act, "topic": topic, "goal": goal,
            "strength": opt_uint(a, "strength", 0, 100, 50), "at": opt_uint(a, "at", 0, MAX_AT, 0)}


def process_local(kind: str, args: Any) -> dict:
    if kind == "beliefs":
        return BeliefsRef().run(args)
    if kind == "speech_act":
        return SpeechRef().run(args)
    raise Bad(f"unknown local kind {kind}")
