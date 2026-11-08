"""Pure-Python reference for creeds and conscience (pass 2, stage 4 and B5, local kind
`creed`).

Written independently of src/character/creed.rs; creed_cases.py checks the two agree on
conformance cases and randomized workflows.
"""
from typing import Any, Dict, List, Optional, Tuple

from beliefs_speech_reference import Bad, blist, ident, keys, need, opt_bool, opt_uint, run_ops, text, uint
from diet_reference import FOOD_TAGS

CREED_KINDS = ["ideology", "philosophy", "religion", "tradition"]
STANCES = ["hostile", "tolerant", "wary"]
MAX_CREEDS = 32
MAX_BEINGS = 128
MAX_AT = 1_000_000_000_000
MAX_PERIOD = 1_000_000
LAX = 30
STANDING = 50


def read_tenet(v: Any, allow_sacred: bool) -> Dict[str, Any]:
    keys(v, ["id", "severity", "label"], ["action", "food_tag", "sacred_only"])
    action = None if v.get("action") is None else ident(v["action"])
    tag = None if v.get("food_tag") is None else ident(v["food_tag"])
    need((action is None) != (tag is None))
    need(tag is None or tag in FOOD_TAGS)
    sacred = opt_bool(v, "sacred_only", False)
    need(not sacred or (allow_sacred and action is not None))
    return {"action": action, "food_tag": tag, "id": ident(v["id"]), "label": text(v["label"], 80),
            "sacred_only": sacred, "severity": uint(v["severity"], 1, 100)}


def read_creed(v: Any) -> Dict[str, Any]:
    keys(v, ["id", "label", "kind"], ["tenets", "observances", "sacred", "rivals"])
    cid = ident(v["id"])
    kind = text(v["kind"], 16)
    need(kind in CREED_KINDS)
    tenets: List[Dict[str, Any]] = []
    for t in blist(v.get("tenets"), 16):
        t = read_tenet(t, True)
        need(all(x["id"] != t["id"] for x in tenets))
        tenets.append(t)
    obs: List[Dict[str, Any]] = []
    for o in blist(v.get("observances"), 8):
        keys(o, ["id", "action", "every", "window", "severity", "label"], ["offset"])
        oid = ident(o["id"])
        need(len(oid) <= 56 and all(x["id"] != oid for x in obs))
        every = uint(o["every"], 1, MAX_PERIOD)
        obs.append({"id": oid, "action": ident(o["action"]), "every": every, "window": uint(o["window"], 1, every),
                    "offset": opt_uint(o, "offset", 0, every - 1, 0), "severity": uint(o["severity"], 1, 100),
                    "label": text(o["label"], 80)})
    sacred = sorted({ident(s) for s in blist(v.get("sacred"), 16)})
    rivals: Dict[str, str] = {}
    for r in blist(v.get("rivals"), 8):
        keys(r, ["creed", "stance"], [])
        other = ident(r["creed"])
        stance = text(r["stance"], 16)
        need(other != cid and stance in STANCES and other not in rivals)
        rivals[other] = stance
    return {"id": cid, "label": text(v["label"], 80), "kind": kind, "tenets": tenets, "observances": obs,
            "sacred": sacred, "rivals": rivals}


def broken_by(t: Dict[str, Any], action: str, target: Optional[str], tags: set, sacred: List[str]) -> bool:
    if t["action"] is not None:
        return t["action"] == action and (not t["sacred_only"] or (target is not None and target in sacred))
    return action == "eat" and t["food_tag"] in tags


def period(o: Dict[str, Any], at: int) -> Optional[Tuple[int, bool]]:
    if at < o["offset"]:
        return None
    rel = at - o["offset"]
    return rel // o["every"], rel % o["every"] < o["window"]


def mood(being: str, delta: int, reason: str) -> Dict[str, Any]:
    return {"args": {"being": being, "reason": reason, "valence_delta": delta}, "engine": "affect", "op": "set_mood"}


def new_adherent() -> Dict[str, Any]:
    return {"creed": None, "devotion": 0, "joined_at": 0, "norms": [], "standing": {}, "observed": {}, "settled": {}}


class CreedRef:
    def __init__(self) -> None:
        self.creeds: Dict[str, Dict[str, Any]] = {}
        self.beings: Dict[str, Dict[str, Any]] = {}

    def run(self, request: Any) -> Dict[str, Any]:
        table = {"register_creed": self.register, "adhere": self.adhere, "set_norms": self.set_norms,
                 "judge": self.judge, "observe": self.observe, "due": self.due, "food_taboos": self.food_taboos,
                 "regard": self.regard, "assess": self.assess}

        def f(op, a):
            need(op in table)
            return table[op](a)
        return run_ops(request, f)

    def being(self, b: str) -> Dict[str, Any]:
        need(b in self.beings or len(self.beings) < MAX_BEINGS)
        return self.beings.setdefault(b, new_adherent())

    def register(self, a: Any) -> Dict[str, Any]:
        keys(a, ["creed"], [])
        cr = read_creed(a["creed"])
        is_new = cr["id"] not in self.creeds
        need(not is_new or len(self.creeds) < MAX_CREEDS)
        out = {"creed": cr["id"], "observances": len(cr["observances"]), "tenets": len(cr["tenets"]),
               "total": len(self.creeds) + (1 if is_new else 0)}
        self.creeds[cr["id"]] = cr
        return out

    def adhere(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being", "creed"], ["devotion", "at"])
        b = ident(a["being"])
        creed = None if a.get("creed") is None else ident(a["creed"])
        need(creed is None or creed in self.creeds)
        if creed is not None:
            devotion = opt_uint(a, "devotion", 0, 100, 50)
        else:
            need(a.get("devotion") is None)
            devotion = 0
        at = opt_uint(a, "at", 0, MAX_AT, 0)
        ad = self.being(b)
        prev = ad["creed"]
        ad.update({"creed": creed, "devotion": devotion, "joined_at": at, "observed": {}, "settled": {}})
        return {"being": b, "creed": creed, "devotion": devotion, "previous": prev}

    def set_norms(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being", "norms"], [])
        b = ident(a["being"])
        norms: List[Dict[str, Any]] = []
        need(isinstance(a["norms"], list) and len(a["norms"]) <= 16)
        for n in a["norms"]:
            n = read_tenet(n, False)
            need(all(x["id"] != n["id"] for x in norms))
            norms.append(n)
        ad = self.being(b)
        ad["norms"] = norms
        return {"being": b, "norms": len(norms)}

    def breaches(self, b: str, action: str, target: Optional[str], tags: set) -> List[Tuple[dict, str, int]]:
        ad = self.beings.get(b)
        if ad is None:
            return []
        out = []
        cr = self.creeds.get(ad["creed"]) if ad["creed"] is not None else None
        if cr is not None:
            for t in cr["tenets"]:
                if broken_by(t, action, target, tags, cr["sacred"]):
                    out.append((t, cr["id"], ad["devotion"]))
        for t in ad["norms"]:
            if broken_by(t, action, target, tags, []):
                out.append((t, "personal", 100))
        return out

    def judge(self, a: Any) -> Dict[str, Any]:
        keys(a, ["actor", "action"], ["target", "witnesses", "food_tags", "at"])
        actor = ident(a["actor"])
        action = ident(a["action"])
        target = None if a.get("target") is None else ident(a["target"])
        tags = set()
        for t in blist(a.get("food_tags"), 8):
            t = ident(t)
            need(t in FOOD_TAGS)
            tags.add(t)
        wits: List[str] = []
        for w in blist(a.get("witnesses"), 16):
            w = ident(w)
            need(w != actor and w not in wits)
            wits.append(w)
        opt_uint(a, "at", 0, MAX_AT, 0)
        mine = self.breaches(actor, action, target, tags)
        guilt = max([t["severity"] * w // 100 for t, _, w in mine] + [0])
        indignant, effects, by_creed = [], [], {}
        for w in sorted(wits):
            theirs = self.breaches(w, action, target, tags)
            level = max([t["severity"] * wt // 100 for t, _, wt in theirs] + [0])
            if level == 0:
                continue
            for t, src, wt in theirs:
                if src != "personal":
                    by_creed[src] = max(by_creed.get(src, 0), t["severity"] * wt // 100)
            indignant.append({"being": w, "indignation": level})
            effects.append({"args": {"being": w, "delta": level, "kind": "resentment",
                                     "reason": f"saw {actor} {action}", "subject": actor},
                            "engine": "affect", "op": "appraise"})
        shame = guilt // 2 if guilt > 0 and indignant else 0
        if guilt + shame > 0:
            effects.insert(0, mood(actor, -min(100, guilt + shame), "broke " + ", ".join(t["id"] for t, _, _ in mine)))
        standing = []
        if by_creed:
            ad = self.being(actor)
            for cr in sorted(by_creed):
                loss = max(1, by_creed[cr] // 5)
                ad["standing"][cr] = max(0, ad["standing"].get(cr, STANDING) - loss)
                standing.append({"creed": cr, "delta": -loss, "value": ad["standing"][cr]})
        return {"action": action, "actor": actor,
                "broken": [{"id": t["id"], "severity": t["severity"], "source": s, "weight": w} for t, s, w in mine],
                "effects": effects, "guilt": guilt, "indignant": indignant, "shame": shame, "standing": standing}

    def observe(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being", "observance"], ["at"])
        b = ident(a["being"])
        oid = ident(a["observance"])
        at = opt_uint(a, "at", 0, MAX_AT, 0)
        need(b in self.beings)
        ad = self.beings[b]
        need(ad["creed"] is not None and ad["creed"] in self.creeds)
        cr = self.creeds[ad["creed"]]
        found = [o for o in cr["observances"] if o["id"] == oid]
        need(len(found) == 1)
        pr = period(found[0], at)
        if pr is None or not pr[1]:
            result = "not_in_window"
        elif ad["observed"].get(oid) == pr[0]:
            result = "already_done"
        else:
            ad["observed"][oid] = pr[0]
            ad["devotion"] = min(100, ad["devotion"] + 1)
            ad["standing"][cr["id"]] = min(100, ad["standing"].get(cr["id"], STANDING) + 1)
            result = "done"
        return {"being": b, "devotion": ad["devotion"], "observance": oid, "result": result,
                "standing": ad["standing"].get(cr["id"], STANDING)}

    def due(self, a: Any) -> Dict[str, Any]:
        keys(a, ["at"], ["being"])
        at = uint(a["at"], 0, MAX_AT)
        only = None if a.get("being") is None else ident(a["being"])
        need(only is None or only in self.beings)
        due, missed, effects = [], [], []
        for name in sorted(self.beings):
            if only is not None and name != only:
                continue
            ad = self.beings[name]
            if ad["creed"] is None or ad["creed"] not in self.creeds:
                continue
            cr = self.creeds[ad["creed"]]
            devotion = ad["devotion"]
            for o in cr["observances"]:
                pr = period(o, at)
                if pr is None:
                    continue
                p, open_ = pr
                if open_ and ad["observed"].get(o["id"]) != p:
                    due.append({"being": name, "creed": cr["id"], "observance": o["id"], "motive": {
                        "action": o["action"], "id": f"observe_{o['id']}", "kind": "goal", "label": o["label"],
                        "reason": f"{cr['id']} observance {o['id']} is due",
                        "score": max(20, min(70, o["severity"] * devotion // 100))}})
                closed = (p - 1 if p >= 1 else None) if open_ else p
                if closed is None:
                    continue
                start = o["offset"] + closed * o["every"]
                if start < ad["joined_at"] or (o["id"] in ad["settled"] and ad["settled"][o["id"]] >= closed):
                    continue
                ad["settled"][o["id"]] = closed
                if ad["observed"].get(o["id"]) == closed:
                    continue
                guilt = o["severity"] * devotion // 100
                loss = max(1, o["severity"] // 10)
                ad["standing"][cr["id"]] = max(0, ad["standing"].get(cr["id"], STANDING) - loss)
                missed.append({"being": name, "guilt": guilt, "observance": o["id"], "period": closed,
                               "standing": ad["standing"][cr["id"]]})
                if guilt > 0:
                    effects.append(mood(name, -guilt, f"missed {o['id']}"))
        return {"at": at, "due": due, "effects": effects, "missed": missed}

    def food_taboos(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being"], [])
        b = ident(a["being"])
        tags = set()
        ad = self.beings.get(b)
        if ad is not None:
            cr = self.creeds.get(ad["creed"]) if ad["creed"] is not None else None
            if cr is not None and ad["devotion"] >= LAX:
                tags |= {t["food_tag"] for t in cr["tenets"] if t["food_tag"] is not None}
            tags |= {t["food_tag"] for t in ad["norms"] if t["food_tag"] is not None}
        return {"being": b, "forbids": sorted(tags)}

    def regard(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being", "other"], [])
        b, o = ident(a["being"]), ident(a["other"])
        need(b != o)
        mine = self.beings[b]["creed"] if b in self.beings else None
        theirs = self.beings[o]["creed"] if o in self.beings else None
        if mine is not None and theirs is not None:
            stance = "kin" if mine == theirs else self.creeds[mine]["rivals"].get(theirs, "neutral")
        else:
            stance = "neutral"
        return {"being": b, "creed": mine, "other": o, "other_creed": theirs, "stance": stance}

    def assess(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being"], [])
        b = ident(a["being"])
        need(b in self.beings)
        ad = self.beings[b]
        return {"being": b, "creed": ad["creed"], "devotion": ad["devotion"], "joined_at": ad["joined_at"],
                "norms": [{k: n[k] for k in ("action", "food_tag", "id", "label", "sacred_only", "severity")}
                          for n in ad["norms"]],
                "observed": [{"observance": k, "period": ad["observed"][k]} for k in sorted(ad["observed"])],
                "standing": [{"creed": k, "value": ad["standing"][k]} for k in sorted(ad["standing"])]}


def process_local(kind: str, args: Any) -> Dict[str, Any]:
    if kind == "creed":
        return CreedRef().run(args)
    raise Bad(f"unknown local kind {kind}")
