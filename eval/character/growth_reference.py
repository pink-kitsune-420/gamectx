"""Pure-Python reference for pass 2 stage 6: goals from experience (A4, local kind
`aspiration`), habits (A5, `habit`) and lessons (A6, `lesson`), with host-set limits.

Written independently of src/character/{limits,aspiration,habit,lesson}.rs;
growth_cases.py checks they agree on conformance cases and randomized workflows.
"""
from typing import Any, Dict, List, Optional, Tuple

from beliefs_speech_reference import Bad, blist, ident, keys, need, opt_uint, run_ops, text, uint

MAX_AT = 1_000_000_000_000


# ---------------------------------------------------------------- limits

class Limits:
    def __init__(self, spec: List[Tuple[str, int, int]]) -> None:
        self.spec = spec
        self.values = {name: default for name, default, _ in spec}

    def apply(self, v: Any) -> None:
        keys(v, [], [n for n, _, _ in self.spec])
        nxt = dict(self.values)
        for name, _, ceiling in self.spec:
            if v.get(name) is not None:
                nxt[name] = uint(v[name], 1, ceiling)
        self.values = nxt

    def view(self) -> Dict[str, int]:
        return {n: self.values[n] for n, _, _ in self.spec}


def run_limited(request: Any, limits: Limits, f) -> Dict[str, Any]:
    keys(request, ["ops"], ["limits"])
    supplied = "limits" in request
    if request.get("limits") is not None:
        limits.apply(request["limits"])
    out = run_ops({"ops": request["ops"]}, f)
    return {"limits": limits.view(), **out} if supplied else out


def table_runner(table: Dict[str, Any]):
    def f(op, a):
        need(op in table)
        return table[op](a)
    return f


# ---------------------------------------------------------------- A4 aspiration

ASP_LIMITS = [("beings", 1024, 1_000_000), ("pressures", 64, 4096), ("goals", 16, 256)]
KINDS = ["admiration", "loss", "need", "opportunity", "threat"]
TEMPLATES = {
    "need": ("stock_{}", "Keep a store against {}", "stock_up"),
    "threat": ("guard_{}", "Guard against {}", "secure_home"),
    "admiration": ("learn_{}", "Learn {}", "practice"),
    "loss": ("replace_{}", "Replace the lost {}", "seek"),
    "opportunity": ("pursue_{}", "Pursue {}", "pursue"),
}


def asp_kind(v: Any) -> str:
    k = text(v, 16)
    need(k in KINDS)
    return k


def asp_subject(v: Any) -> str:
    s = ident(v)
    need(len(s) <= 48)
    return s


def goal_view(gid: str, g: Dict[str, Any]) -> Dict[str, Any]:
    return {"action": g["action"], "formed_at": g["formed_at"], "id": gid, "label": g["label"], "origin": g["origin"],
            "priority": g["priority"], "reason": g["reason"], "target": g["target"]}


class AspireRef:
    def __init__(self) -> None:
        self.beings: Dict[str, Dict[str, Any]] = {}
        self.limits = Limits(ASP_LIMITS)

    def run(self, request: Any) -> Dict[str, Any]:
        return run_limited(request, self.limits, table_runner(
            {"experience": self.experience, "propose": self.propose, "drop": self.drop, "goals": self.goals,
             "assess": self.assess}))

    def drive(self, being: str) -> Dict[str, Any]:
        need(being in self.beings or len(self.beings) < self.limits.values["beings"])
        return self.beings.setdefault(being, {"pressures": {}, "goals": {}})

    def experience(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being", "kind", "subject"], ["weight", "at"])
        being, kind, subject = ident(a["being"]), asp_kind(a["kind"]), asp_subject(a["subject"])
        weight = opt_uint(a, "weight", 1, 100, 50)
        at = opt_uint(a, "at", 0, MAX_AT, 0)
        idf, labelf, action = TEMPLATES[kind]
        gid, label = idf.format(subject), labelf.format(subject)
        d = self.drive(being)
        base = {"being": being, "blocked": False, "kind": kind, "subject": subject}
        if gid in d["goals"]:
            g = d["goals"][gid]
            before = g["priority"]
            g["priority"] = min(90, g["priority"] + weight // 10)
            return {**base, "goal": goal_view(gid, g), "pressure": None,
                    "result": "reinforced" if g["priority"] > before else "steady"}
        ps = d["pressures"]
        key = f"{kind}:{subject}"
        if key not in ps and len(ps) >= self.limits.values["pressures"]:
            del ps[min(ps, key=lambda k: (ps[k]["total"], ps[k]["last_at"], k))]
        p = ps.setdefault(key, {"total": 0, "count": 0, "last_at": at})
        if at > p["last_at"] and at - p["last_at"] > 200:
            p["total"], p["count"] = 0, 0
        p["total"] = min(1000, p["total"] + weight)
        p["count"] += 1
        p["last_at"] = max(p["last_at"], at)
        pv = {"count": p["count"], "last_at": p["last_at"], "total": p["total"]}
        if p["total"] < 100:
            return {**base, "goal": None, "pressure": pv, "result": "building"}
        if len(d["goals"]) >= self.limits.values["goals"]:
            return {**base, "blocked": True, "goal": None, "pressure": pv, "result": "building"}
        del ps[key]
        g = {"label": label, "action": action, "target": subject, "priority": min(90, 40 + p["total"] // 10),
             "origin": "template", "reason": f"{p['count']} {kind} experiences about {subject}", "formed_at": at}
        d["goals"][gid] = g
        return {**base, "goal": goal_view(gid, g), "pressure": pv, "result": "formed"}

    def propose(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being", "goal", "priority", "cites"], ["at"])
        being = ident(a["being"])
        g = a["goal"]
        keys(g, ["id", "label", "action"], ["target"])
        gid, label, action = ident(g["id"]), text(g["label"], 80), ident(g["action"])
        target = None if g.get("target") is None else ident(g["target"])
        priority = uint(a["priority"], 1, 100)
        need(isinstance(a["cites"], list) and 1 <= len(a["cites"]) <= 4)
        cites: List[str] = []
        for ci in a["cites"]:
            keys(ci, ["kind", "subject"], [])
            k = f"{asp_kind(ci['kind'])}:{asp_subject(ci['subject'])}"
            need(k not in cites)
            cites.append(k)
        at = opt_uint(a, "at", 0, MAX_AT, 0)
        d = self.drive(being)
        evidence = sum(d["pressures"][k]["total"] for k in cites if k in d["pressures"])
        if gid in d["goals"]:
            verdict = "exists"
        elif any(k not in d["pressures"] for k in cites):
            verdict = "uncited"
        elif evidence < 50:
            verdict = "weak_evidence"
        elif len(d["goals"]) >= self.limits.values["goals"]:
            verdict = "no_room"
        else:
            verdict = "accepted"
        if verdict != "accepted":
            return {"accepted": False, "being": being, "evidence": evidence, "goal": None, "reason": verdict}
        goal = {"label": label, "action": action, "target": target, "priority": min(priority, 60),
                "origin": "proposal", "reason": "proposed from " + ", ".join(cites), "formed_at": at}
        d["goals"][gid] = goal
        return {"accepted": True, "being": being, "evidence": evidence, "goal": goal_view(gid, goal),
                "reason": "capped" if priority > 60 else "accepted"}

    def drop(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being", "goal"], [])
        being, gid = ident(a["being"]), ident(a["goal"])
        d = self.beings.get(being)
        dropped = d is not None and d["goals"].pop(gid, None) is not None
        return {"being": being, "dropped": dropped, "goal": gid}

    def goals(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being"], [])
        being = ident(a["being"])
        gs = self.beings.get(being, {"goals": {}})["goals"]
        order = sorted(gs, key=lambda k: (-gs[k]["priority"], k))
        return {"being": being, "goals": [{"action": gs[k]["action"], "id": k, "label": gs[k]["label"],
                                           "priority": gs[k]["priority"]} for k in order]}

    def assess(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being"], [])
        being = ident(a["being"])
        need(being in self.beings)
        d = self.beings[being]
        pressures = []
        for k in sorted(d["pressures"]):
            p = d["pressures"][k]
            kind, _, subject = k.partition(":")
            pressures.append({"count": p["count"], "kind": kind, "last_at": p["last_at"], "subject": subject,
                              "total": p["total"]})
        return {"being": being, "goals": [goal_view(k, d["goals"][k]) for k in sorted(d["goals"])],
                "pressures": pressures}


# ---------------------------------------------------------------- A5 habit

HAB_LIMITS = [("beings", 1024, 1_000_000), ("entries", 32, 512), ("slots", 32, 512)]


def slot_of(v: Any) -> str:
    s = ident(v)
    need(len(s) <= 24)
    return s


def action_of(v: Any) -> str:
    s = ident(v)
    need(len(s) <= 32)
    return s


def log(e: Dict[str, Any], at: int, delta: int, reason: str) -> None:
    e["history"].append((at, delta, reason))
    if len(e["history"]) > 8:
        e["history"].pop(0)


def change(delta: int, entry: str, reason: str, state: str, strength: int) -> Dict[str, Any]:
    return {"delta": delta, "entry": entry, "reason": reason, "state": state, "strength": strength}


def weaken(r: Dict[str, Any], eid: str, at: int, reason: str) -> Optional[Dict[str, Any]]:
    e = r["entries"].get(eid)
    if e is None:
        return None
    step, floor = (10, 0) if e["habit"] else (5, 30)
    nxt = max(floor, max(0, e["strength"] - step))
    delta = nxt - e["strength"]
    if delta == 0:
        return change(0, eid, reason, "steady", e["strength"])
    e["strength"] = nxt
    log(e, at, delta, reason)
    state = "faded" if e["habit"] and nxt == 0 else "weakened"
    if state == "faded":
        del r["entries"][eid]
    return change(delta, eid, reason, state, nxt)


class HabitRef:
    def __init__(self) -> None:
        self.beings: Dict[str, Dict[str, Any]] = {}
        self.limits = Limits(HAB_LIMITS)

    def run(self, request: Any) -> Dict[str, Any]:
        return run_limited(request, self.limits, table_runner(
            {"set_routine": self.set_routine, "choose": self.choose, "skip": self.skip, "routine": self.routine,
             "assess": self.assess}))

    def mine(self, being: str) -> Dict[str, Any]:
        need(being in self.beings or len(self.beings) < self.limits.values["beings"])
        return self.beings.setdefault(being, {"entries": {}, "tallies": {}})

    def set_routine(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being", "entries"], [])
        being = ident(a["being"])
        mx = self.limits.values["entries"]
        need(isinstance(a["entries"], list) and len(a["entries"]) <= mx)
        authored: Dict[str, Dict[str, Any]] = {}
        for e in a["entries"]:
            keys(e, ["id", "slot", "action"], ["strength"])
            eid = ident(e["id"])
            need(not eid.startswith("habit_") and eid not in authored)
            ent = {"slot": slot_of(e["slot"]), "action": action_of(e["action"]),
                   "strength": opt_uint(e, "strength", 30, 100, 80), "habit": False, "history": []}
            need(not any(x["slot"] == ent["slot"] and x["action"] == ent["action"] for x in authored.values()))
            authored[eid] = ent
        r = self.mine(being)
        habits = {k: v for k, v in r["entries"].items() if v["habit"]}
        need(len(habits) + len(authored) <= mx)
        r["entries"] = {**habits, **authored}
        return {"authored": len(authored), "being": being, "entries": len(r["entries"])}

    def choose(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being", "slot", "action"], ["at"])
        being, slot, action = ident(a["being"]), slot_of(a["slot"]), action_of(a["action"])
        at = opt_uint(a, "at", 0, MAX_AT, 0)
        r = self.mine(being)
        changes = []
        for eid in sorted(k for k, e in r["entries"].items() if e["slot"] == slot):
            e = r["entries"][eid]
            if e["action"] == action:
                nxt = min(100, e["strength"] + 5)
                delta = nxt - e["strength"]
                if delta:
                    e["strength"] = nxt
                    log(e, at, delta, "chosen")
                changes.append(change(delta, eid, "chosen", "kept" if delta else "steady", nxt))
            else:
                changes.append(weaken(r, eid, at, f"chose {action} instead"))
        t = r["tallies"]
        if slot not in t and len(t) >= self.limits.values["slots"]:
            del t[min(t, key=lambda k: (t[k][1], k))]
        if slot in t and t[slot][0] == action:
            t[slot] = (action, t[slot][1] + 1)
        else:
            t[slot] = (action, 1)
        streak = t[slot][1]
        has = any(e["slot"] == slot and e["action"] == action for e in r["entries"].values())
        if streak >= 3 and not has:
            eid = f"habit_{slot}_{action}"
            reason = f"chose {action} {streak} times in a row"
            if len(r["entries"]) >= self.limits.values["entries"]:
                changes.append(change(0, eid, reason, "blocked", 0))
            else:
                e = {"slot": slot, "action": action, "strength": 40, "habit": True, "history": []}
                log(e, at, 40, reason)
                r["entries"][eid] = e
                changes.append(change(40, eid, reason, "formed", 40))
        return {"action": action, "being": being, "changes": changes, "slot": slot, "streak": streak}

    def skip(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being", "entry"], ["at", "reason"])
        being, eid = ident(a["being"]), ident(a["entry"])
        at = opt_uint(a, "at", 0, MAX_AT, 0)
        reason = "skipped" if a.get("reason") is None else text(a["reason"], 80)
        need(being in self.beings)
        out = weaken(self.beings[being], eid, at, reason)
        need(out is not None)
        return out

    def routine(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being"], ["slot"])
        being = ident(a["being"])
        slot = None if a.get("slot") is None else slot_of(a["slot"])
        es = self.beings.get(being, {"entries": {}})["entries"]
        ids = sorted((k for k in es if slot is None or es[k]["slot"] == slot), key=lambda k: (-es[k]["strength"], k))
        return {"being": being, "entries": [{"action": es[k]["action"], "id": k,
                                             "label": f"{es[k]['action']} ({es[k]['slot']})",
                                             "priority": es[k]["strength"], "slot": es[k]["slot"]} for k in ids]}

    def assess(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being"], [])
        being = ident(a["being"])
        need(being in self.beings)
        r = self.beings[being]
        entries = []
        for k in sorted(r["entries"]):
            e = r["entries"][k]
            entries.append({"action": e["action"],
                            "history": [{"at": at, "delta": d, "reason": why} for at, d, why in e["history"]],
                            "id": k, "origin": "habit" if e["habit"] else "authored", "slot": e["slot"],
                            "strength": e["strength"]})
        return {"being": being, "entries": entries,
                "tallies": [{"action": r["tallies"][s][0], "slot": s, "streak": r["tallies"][s][1]}
                            for s in sorted(r["tallies"])]}


# ---------------------------------------------------------------- A6 lesson

LES_LIMITS = [("beings", 1024, 1_000_000), ("topics", 64, 4096), ("episodes", 16, 256)]


def verdict_of(eps: List[Dict[str, Any]]) -> Optional[str]:
    good = sum(1 for e in eps if e["good"])
    total = len(eps)
    bad = total - good
    if total < 3:
        return None
    if bad * 3 >= total * 2:
        return "avoid"
    if good * 3 >= total * 2:
        return "seek"
    return None


def lesson_view(t: Dict[str, Any], about: str, activity: str) -> Optional[Dict[str, Any]]:
    v = t["verdict"]
    if v is None:
        return None
    eps = t["episodes"]
    good = sum(1 for e in eps if e["good"])
    total = len(eps)
    bad = total - good
    want = v == "seek"
    majority = good if want else bad
    return {"about": about, "activity": activity, "bad": bad, "cites": [e["id"] for e in eps if e["good"] == want],
            "confidence": majority * 100 // total, "good": good,
            "summary": f"{activity} with {about} went {'well' if want else 'badly'} {majority} of {total} times",
            "verdict": v}


class LessonRef:
    def __init__(self) -> None:
        self.beings: Dict[str, Dict[Tuple[str, str], Dict[str, Any]]] = {}
        self.limits = Limits(LES_LIMITS)

    def run(self, request: Any) -> Dict[str, Any]:
        return run_limited(request, self.limits, table_runner(
            {"outcome": self.outcome, "advice": self.advice, "forget": self.forget, "assess": self.assess}))

    def outcome(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being", "about", "activity", "result", "episode"], ["at"])
        being, about, activity = ident(a["being"]), ident(a["about"]), ident(a["activity"])
        res = text(a["result"], 8)
        need(res in ("good", "bad"))
        good = res == "good"
        episode = ident(a["episode"])
        at = opt_uint(a, "at", 0, MAX_AT, 0)
        need(being in self.beings or len(self.beings) < self.limits.values["beings"])
        topics = self.beings.setdefault(being, {})
        key = (about, activity)
        if key not in topics and len(topics) >= self.limits.values["topics"]:
            del topics[min(topics, key=lambda k: (len(topics[k]["episodes"]), topics[k]["last_at"], k))]
        t = topics.setdefault(key, {"episodes": [], "verdict": None, "last_at": 0})
        need(all(e["id"] != episode for e in t["episodes"]))
        t["episodes"].append({"id": episode, "at": at, "good": good})
        while len(t["episodes"]) > self.limits.values["episodes"]:
            t["episodes"].pop(0)
        t["last_at"] = max(t["last_at"], at)
        prev = t["verdict"]
        t["verdict"] = verdict_of(t["episodes"])
        v = t["verdict"]
        if prev is None:
            chg = "noted" if v is None else "learned"
        elif v is None:
            chg = "doubted"
        elif prev != v:
            chg = "revised"
        else:
            chg = "confirmed" if (v == "seek") == good else "contradicted"
        return {"about": about, "activity": activity, "being": being, "change": chg,
                "lesson": lesson_view(t, about, activity), "previous": prev}

    def advice(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being"], ["about", "activity"])
        being = ident(a["being"])
        about = None if a.get("about") is None else ident(a["about"])
        activity = None if a.get("activity") is None else ident(a["activity"])
        out = []
        for (ab, ac), t in sorted(self.beings.get(being, {}).items()):
            if (about is not None and about != ab) or (activity is not None and activity != ac):
                continue
            lv = lesson_view(t, ab, ac)
            if lv is None:
                continue
            q = lv["confidence"] // 4
            out.append({"about": ab, "activity": ac, "bias": -q if t["verdict"] == "avoid" else q,
                        "summary": lv["summary"], "verdict": t["verdict"]})
        return {"being": being, "lessons": out}

    def forget(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being", "about", "activity"], [])
        being, about, activity = ident(a["being"]), ident(a["about"]), ident(a["activity"])
        topics = self.beings.get(being)
        forgot = topics is not None and topics.pop((about, activity), None) is not None
        return {"about": about, "activity": activity, "being": being, "forgot": forgot}

    def assess(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being"], [])
        being = ident(a["being"])
        need(being in self.beings)
        out = []
        for (ab, ac), t in sorted(self.beings[being].items()):
            out.append({"about": ab, "activity": ac,
                        "episodes": [{"at": e["at"], "episode": e["id"], "result": "good" if e["good"] else "bad"}
                                     for e in t["episodes"]],
                        "lesson": lesson_view(t, ab, ac)})
        return {"being": being, "topics": out}


def process_local(kind: str, args: Any) -> Dict[str, Any]:
    if kind == "aspiration":
        return AspireRef().run(args)
    if kind == "habit":
        return HabitRef().run(args)
    if kind == "lesson":
        return LessonRef().run(args)
    raise Bad(f"unknown local kind {kind}")
