"""Independent original reference for living culture: community idioms and rumor mutation (pass 4,
stage 4). Python standard library only; nothing here calls a model.

Members have a community, a status, openness, carefulness, credulity and plausible links (beings a
rumor about them could be shifted to). Idioms belong to a community and have a prestige. Hearing an
idiom from a speaker who uses it (its community, or a strength at or above the use threshold)
raises the listener's strength by `status x openness x (100 + prestige) / 20` millionths; strength
fades by 1,000 per tick since it was last raised. Idioms at or above 500,000 are usable in speech.

A witness seeds a rumor: the host's truth (never rewritten) and the witness's own version. Telling
copies the speaker's version to the listener, perhaps mutated: a seeded FNV-1a hash of
`"<seed>:<speaker>:<topic>:<tick>"` decides whether to mutate (chance per mille
`(100 - carefulness) x credulity / 10`) and which applicable operator to apply, in the fixed order
drop_detail, exaggerate, shift_attribution, merge (with one of the first 16 other topics the speaker
holds, in id order), soften. Every version keeps a truth distance
(operators applied, plus numeric drift from the truth in millionths), a hop count and the last eight
links of its provenance chain. A listener who already holds a version keeps it unless it trusts the
teller (trust >= 50), so a trusted witness retelling the original is a correction.
"""
import pathlib
import sys
from typing import Any, Dict, List, Optional

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_uint, text, uint
from cognition_reference import fnv1a
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
FULL = 1_000_000
USE = 500_000
FADE = 1_000
MAX_AMOUNT = 1_000_000_000_000
CHAIN = 8
MERGE_POOL = 16
SPEC = [("beings", 1024, 1_000_000), ("idioms", 64, 100_000), ("repertoire", 32, 1024), ("topics", 256, 1_000_000),
        ("beliefs", 4096, 1_000_000)]
SOFTER = {"stole": "borrowed", "struck": "pushed", "lied": "misspoke", "broke": "damaged", "threatened": "warned",
          "cheated": "miscounted"}
OPERATORS = ["drop_detail", "exaggerate", "shift_attribution", "merge", "soften"]


def id_list(v: Any, n: int) -> List[str]:
    if v is None:
        return []
    need(isinstance(v, list) and len(v) <= n)
    out = [ident(x) for x in v]
    need(len(set(out)) == len(out))
    return out


class Culture:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.members: Dict[str, dict] = {}
        self.idioms: Dict[str, dict] = {}
        self.truths: Dict[str, dict] = {}
        self.beliefs: Dict[str, Dict[str, dict]] = {}  # being -> topic -> version
        self.holders: Dict[str, set] = {}  # topic -> beings
        self.count = 0
        self.clock = 0

    def member(self, b: str) -> dict:
        need(b in self.members)
        return self.members[b]

    def strength(self, entry: dict) -> int:
        return max(0, entry["strength"] - FADE * (self.clock - entry["at"]))

    def op_tick(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        self.clock = now
        return {"now": now}

    def op_member(self, a: dict) -> dict:
        keys(a, ["being", "community", "status", "openness", "carefulness", "credulity"], ["links"])
        b, com = ident(a["being"]), ident(a["community"])
        prof = {k: uint(a[k], 0, 100) for k in ("status", "openness", "carefulness", "credulity")}
        links = id_list(a.get("links"), 8)
        need(b not in links)
        need(b in self.members or len(self.members) < self.lim["beings"])
        old = self.members.get(b)
        prof.update({"community": com, "links": sorted(links), "repertoire": {} if old is None else old["repertoire"]})
        self.members[b] = prof
        return {"being": b, "carefulness": prof["carefulness"], "community": com, "credulity": prof["credulity"],
                "links": prof["links"], "openness": prof["openness"], "status": prof["status"]}

    def op_idiom(self, a: dict) -> dict:
        keys(a, ["id", "community", "phrase", "meaning", "prestige"], [])
        iid, com = ident(a["id"]), ident(a["community"])
        phrase, meaning = text(a["phrase"], 60), text(a["meaning"], 80)
        prestige = uint(a["prestige"], 0, 100)
        need(iid in self.idioms or len(self.idioms) < self.lim["idioms"])
        self.idioms[iid] = {"community": com, "phrase": phrase, "meaning": meaning, "prestige": prestige}
        return {"community": com, "id": iid, "prestige": prestige}

    def op_expose(self, a: dict) -> dict:
        keys(a, ["listener", "speaker", "idiom"], [])
        lis, spk, iid = ident(a["listener"]), ident(a["speaker"]), ident(a["idiom"])
        need(lis != spk and iid in self.idioms)
        L, S = self.member(lis), self.member(spk)
        idm = self.idioms[iid]
        rep = L["repertoire"]
        cur = self.strength(rep[iid]) if iid in rep else 0
        se = S["repertoire"].get(iid)
        uses = S["community"] == idm["community"] or (se is not None and self.strength(se) >= USE)
        if not uses:
            return {"adopted": cur >= USE, "gain": 0, "heard": False, "idiom": iid, "listener": lis, "strength": cur}
        gain = S["status"] * L["openness"] * (100 + idm["prestige"]) // 20
        new = min(FULL, cur + gain)
        live = {k: e for k, e in rep.items() if k == iid or self.strength(e) > 0}
        need(iid in live or len(live) < self.lim["repertoire"])
        live[iid] = {"strength": new, "at": self.clock}
        L["repertoire"] = live
        return {"adopted": new >= USE, "gain": gain, "heard": True, "idiom": iid, "listener": lis, "strength": new}

    def op_speech(self, a: dict) -> dict:
        keys(a, ["being"], [])
        b = ident(a["being"])
        rep = self.member(b)["repertoire"]
        rows = [(self.strength(e), k) for k, e in rep.items() if k in self.idioms]
        usable = sorted([r for r in rows if r[0] >= USE], key=lambda r: (-r[0], r[1]))
        fading = sorted([r for r in rows if 0 < r[0] < USE], key=lambda r: (-r[0], r[1]))
        return {"being": b, "fading": [{"id": k, "strength": s} for s, k in fading],
                "usable": [{"id": k, "phrase": self.idioms[k]["phrase"], "strength": s} for s, k in usable]}

    def drift(self, topic: str, amount: Optional[int]) -> int:
        t = self.truths[topic]["amount"]
        if t is None or amount is None:
            return 0
        return abs(amount - t) * FULL // t

    def view(self, being: str, topic: str, v: dict) -> dict:
        return {"agent": v["agent"], "amount": v["amount"], "being": being, "chain": list(v["chain"]),
                "details": list(v["details"]), "distance": {"drift": self.drift(topic, v["amount"]), "ops": v["ops"]},
                "hearsay": v["hops"] > 0, "hops": v["hops"], "last_mutation": v["last"], "merged": list(v["merged"]),
                "place": v["place"], "topic": topic, "verb": v["verb"]}

    def op_seed(self, a: dict) -> dict:
        keys(a, ["topic", "witness", "verb", "agent"], ["place", "details", "amount"])
        topic, w = ident(a["topic"]), ident(a["witness"])
        verb, agent = ident(a["verb"]), ident(a["agent"])
        place = None if a.get("place") is None else ident(a["place"])
        details = id_list(a.get("details"), 4)
        amount = None if a.get("amount") is None else uint(a["amount"], 1, 1_000_000_000)
        need(topic not in self.truths and len(self.truths) < self.lim["topics"])
        self.member(w)
        need(self.count < self.lim["beliefs"])
        truth = {"agent": agent, "amount": amount, "details": details, "place": place, "verb": verb, "witness": w}
        self.truths[topic] = truth
        v = {"agent": agent, "amount": amount, "chain": [w], "details": list(details), "hops": 0, "last": None,
             "merged": [], "ops": 0, "place": place, "verb": verb}
        self.beliefs.setdefault(w, {})[topic] = v
        self.holders[topic] = {w}
        self.count += 1
        return self.view(w, topic, v)

    def op_tell(self, a: dict) -> dict:
        keys(a, ["speaker", "listener", "topic"], ["seed", "fondness", "trust"])
        spk, lis, topic = ident(a["speaker"]), ident(a["listener"]), ident(a["topic"])
        seed = opt_uint(a, "seed", 0, MAX_AT, 0)
        fond = opt_uint(a, "fondness", 0, 100, 0)
        trust = opt_uint(a, "trust", 0, 100, 50)
        need(spk != lis)
        S, L = self.member(spk), self.member(lis)
        need(topic in self.beliefs.get(spk, {}))
        old = self.beliefs.get(lis, {}).get(topic)
        before = None if old is None else {"drift": self.drift(topic, old["amount"]), "ops": old["ops"]}
        if old is not None and trust < 50:
            return {"before": before, "belief": self.view(lis, topic, old), "kept": True, "mutated": None}
        need(old is not None or self.count < self.lim["beliefs"])
        src = self.beliefs[spk][topic]
        v = {"agent": src["agent"], "amount": src["amount"], "chain": (src["chain"] + [lis])[-CHAIN:],
             "details": list(src["details"]), "hops": src["hops"] + 1, "last": src["last"],
             "merged": list(src["merged"]), "ops": src["ops"], "place": src["place"], "verb": src["verb"]}
        h = fnv1a(f"{seed}:{spk}:{topic}:{self.clock}")
        chance = (100 - S["carefulness"]) * L["credulity"] // 10
        mutated = None
        if h % 1000 < chance:
            links = self.members[v["agent"]]["links"] if v["agent"] in self.members else []
            others = sorted(t for t in self.beliefs[spk] if t != topic and t not in v["merged"])[:MERGE_POOL]
            cand = []
            if v["details"]:
                cand.append("drop_detail")
            if v["amount"] is not None:
                cand.append("exaggerate")
            if links:
                cand.append("shift_attribution")
            if others and len(v["merged"]) < 4:
                cand.append("merge")
            if v["verb"] in SOFTER and fond >= 60:
                cand.append("soften")
            if cand:
                mutated = cand[(h >> 10) % len(cand)]
                k = h >> 20
                if mutated == "drop_detail":
                    del v["details"][k % len(v["details"])]
                elif mutated == "exaggerate":
                    pct = 1 + k % 50
                    v["amount"] = min(MAX_AMOUNT, v["amount"] + max(1, v["amount"] * pct // 100))
                elif mutated == "shift_attribution":
                    v["agent"] = links[k % len(links)]
                elif mutated == "merge":
                    v["merged"] = sorted(v["merged"] + [others[k % len(others)]])
                else:
                    v["verb"] = SOFTER[v["verb"]]
                v["ops"] += 1
                v["last"] = mutated
        if old is None:
            self.count += 1
        self.beliefs.setdefault(lis, {})[topic] = v
        self.holders[topic].add(lis)
        return {"before": before, "belief": self.view(lis, topic, v), "kept": False, "mutated": mutated}

    def op_belief(self, a: dict) -> dict:
        keys(a, ["being", "topic"], [])
        b, topic = ident(a["being"]), ident(a["topic"])
        need(topic in self.beliefs.get(b, {}))
        return self.view(b, topic, self.beliefs[b][topic])

    def op_spread(self, a: dict) -> dict:
        keys(a, ["topic"], [])
        topic = ident(a["topic"])
        need(topic in self.truths)
        vs = [self.beliefs[b][topic] for b in sorted(self.holders[topic])]
        t = self.truths[topic]
        return {"holders": len(vs), "max_drift": max(self.drift(topic, v["amount"]) for v in vs),
                "max_ops": max(v["ops"] for v in vs), "topic": topic, "total_ops": sum(v["ops"] for v in vs),
                "truth": {"agent": t["agent"], "amount": t["amount"], "details": list(t["details"]),
                          "place": t["place"], "verb": t["verb"], "witness": t["witness"]}}

    def workflow(self, request: Any) -> dict:
        table = {"tick": self.op_tick, "member": self.op_member, "idiom": self.op_idiom, "expose": self.op_expose,
                 "speech": self.op_speech, "seed": self.op_seed, "tell": self.op_tell, "belief": self.op_belief,
                 "spread": self.op_spread}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Culture().workflow(request)
