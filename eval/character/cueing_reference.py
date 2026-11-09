"""Independent original reference for associative memory cueing and the epistemic gate
(pass 3, stage 2). Python standard library only; nothing here calls a model.

Cueing (E3): a being keeps episodes with structural features (actions, entities, tags, places), a
valence and an optional emotion. `cue` matches the current situation against them by weighted
feature overlap (shared actions 3, entities 2, tags 1, places 1). A match tied to fear, shame or
a secret is returned to `avoid` (deflect, do not disclose); otherwise it is a `recall` candidate,
unless it was surfaced to this listener within the cooldown (`cooling`), so the character does not
bring up the same past event on consecutive turns. Only `surface` (the host confirming a line
was delivered) records that it came up.

Epistemic gate (F4): every fact a being holds carries how it knows it (`direct_witness`,
`trusted_report`, `unverified_rumor`, `inference`, `corrected_belief`). `frame_facts` returns
each fact with the framing a line must use (state, attribute, hedge, guess, correct). An active
`cover` story masks a true fact for a listener: the framing becomes `cover` with the lie, and
the true claim is withheld. `gate` checks a proposed reply word by word: any key word of a masked
fact (other than words of the cover story itself), or of a host-only `secret` the being does not
hold, is a leak, and the reply is refused with a deterministic fallback (the cover story, else the
host's fallback line).
"""
import pathlib
import sys
from typing import Any, Dict, List, Optional

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_bool, opt_uint, text, uint
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
SPEC = [("beings", 1024, 1_000_000), ("episodes", 64, 4096), ("facts", 64, 4096), ("covers", 16, 256),
        ("secrets", 64, 4096), ("listeners", 8, 64)]
FEATURES = [("actions", 3), ("entities", 2), ("tags", 1), ("places", 1)]
AVOID = {"fear", "shame"}
SOURCES = ["corrected_belief", "direct_witness", "inference", "trusted_report", "unverified_rumor"]
FRAMING = {"direct_witness": "state", "trusted_report": "attribute", "unverified_rumor": "hedge",
           "inference": "guess", "corrected_belief": "correct"}
MAX_LIST = 16
MAX_KEYS = 8
DEFAULT_FALLBACK = "I would rather not say."
ALNUM = set("abcdefghijklmnopqrstuvwxyz0123456789")


def sint(v: Any, lo: int, hi: int) -> int:
    need(isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi)
    return v


def blist(d: dict, k: str, hi: int) -> list:
    v = d.get(k)
    v = [] if v is None else v
    need(isinstance(v, list) and len(v) <= hi)
    return v


def idset(d: dict, k: str) -> List[str]:
    xs = [ident(x) for x in blist(d, k, MAX_LIST)]
    need(len(set(xs)) == len(xs))
    return sorted(xs)


def opt_ident(d: dict, k: str) -> Optional[str]:
    return None if d.get(k) is None else ident(d[k])


def features(v: Any) -> Dict[str, List[str]]:
    v = {} if v is None else v
    keys(v, [], [k for k, _ in FEATURES])
    return {k: idset(v, k) for k, _ in FEATURES}


def key_words(d: dict) -> List[str]:
    ws = blist(d, "keys", MAX_KEYS)
    need(len(ws) >= 1)
    out = []
    for w in ws:
        need(isinstance(w, str) and 1 <= len(w) <= 32 and all(ch in ALNUM for ch in w))
        out.append(w)
    need(len(set(out)) == len(out))
    return sorted(out)


def words(s: str) -> List[str]:
    """Lower-cased runs of ASCII letters and digits; everything else separates."""
    out, cur = [], []
    for ch in s:
        c = ch.lower() if "A" <= ch <= "Z" else ch
        if c in ALNUM:
            cur.append(c)
        elif cur:
            out.append("".join(cur))
            cur = []
    if cur:
        out.append("".join(cur))
    return out


class Cueing:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.beings: Dict[str, dict] = {}
        self.secrets: Dict[str, List[str]] = {}
        self.clock = 0

    def being(self, b: str) -> dict:
        need(b in self.beings or len(self.beings) < self.lim["beings"])
        return self.beings.setdefault(b, {"episodes": {}, "facts": {}, "covers": {}})

    def known(self, b: str) -> dict:
        need(b in self.beings)
        return self.beings[b]

    # ---------------------------------------------------------------- cueing

    def op_tick(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        self.clock = now
        return {"now": now}

    def op_episode(self, a: dict) -> dict:
        keys(a, ["being", "id", "features"], ["valence", "emotion", "secret"])
        b, i = ident(a["being"]), ident(a["id"])
        f = features(a["features"])
        need(any(f[k] for k, _ in FEATURES))
        val = 0 if a.get("valence") is None else sint(a["valence"], -100, 100)
        emo = opt_ident(a, "emotion")
        sec = opt_bool(a, "secret", False)
        x = self.being(b)
        need(i in x["episodes"] or len(x["episodes"]) < self.lim["episodes"])
        x["episodes"][i] = {"features": f, "valence": val, "emotion": emo, "secret": sec, "surfaced": {}}
        return {"being": b, "emotion": emo, "id": i, "secret": sec, "valence": val}

    def op_cue(self, a: dict) -> dict:
        keys(a, ["being", "listener", "situation"], ["cooldown", "min_score", "max"])
        b, lis = ident(a["being"]), ident(a["listener"])
        sit = features(a["situation"])
        cooldown = opt_uint(a, "cooldown", 0, MAX_AT, 24)
        min_score = opt_uint(a, "min_score", 1, 1000, 2)
        mx = opt_uint(a, "max", 1, 16, 2)
        x = self.known(b)
        found = []
        for i, e in x["episodes"].items():
            shared = {k: sorted(set(e["features"][k]) & set(sit[k])) for k, _ in FEATURES}
            score = sum(w * len(shared[k]) for k, w in FEATURES)
            if score < min_score:
                continue
            last = e["surfaced"].get(lis)
            if e["secret"] or e["emotion"] in AVOID:
                mode = "avoid"
            elif last is not None and self.clock - last < cooldown:
                mode = "cooling"
            else:
                mode = "recall"
            found.append({"emotion": e["emotion"], "id": i, "last_surfaced": last, "mode": mode, "score": score,
                          "shared": shared, "valence": e["valence"]})
        found.sort(key=lambda m: (-m["score"], -abs(m["valence"]), m["id"]))
        recall = [m for m in found if m["mode"] == "recall"]
        return {"avoid": [m for m in found if m["mode"] == "avoid"], "being": b,
                "cooling": [m for m in found if m["mode"] == "cooling"], "listener": lis, "recall": recall[:mx],
                "unused": len(recall[mx:])}

    def op_surface(self, a: dict) -> dict:
        keys(a, ["being", "listener", "episode"], [])
        b, lis, i = ident(a["being"]), ident(a["listener"]), ident(a["episode"])
        x = self.known(b)
        need(i in x["episodes"])
        s = x["episodes"][i]["surfaced"]
        if lis not in s and len(s) >= self.lim["listeners"]:
            old = min(s.items(), key=lambda kv: (kv[1], kv[0]))[0]
            del s[old]
        s[lis] = self.clock
        return {"at": self.clock, "being": b, "episode": i, "listener": lis}

    # ---------------------------------------------------------------- epistemic gate

    def op_fact(self, a: dict) -> dict:
        keys(a, ["being", "id", "claim", "source", "keys"], ["from", "previous", "confidence"])
        b, i = ident(a["being"]), ident(a["id"])
        claim = text(a["claim"], 160)
        src = ident(a["source"])
        need(src in SOURCES)
        frm = opt_ident(a, "from")
        need((frm is not None) == (src in ("trusted_report", "unverified_rumor")))
        prev = None if a.get("previous") is None else text(a["previous"], 160)
        need((prev is not None) == (src == "corrected_belief"))
        conf = opt_uint(a, "confidence", 0, 100, 100 if src == "direct_witness" else 60)
        ks = key_words(a)
        x = self.being(b)
        need(i in x["facts"] or len(x["facts"]) < self.lim["facts"])
        x["facts"][i] = {"claim": claim, "source": src, "from": frm, "previous": prev, "confidence": conf, "keys": ks}
        return {"being": b, "framing": FRAMING[src], "id": i, "source": src}

    def op_cover(self, a: dict) -> dict:
        keys(a, ["being", "id", "fact", "lie"], ["listeners"])
        b, i, f = ident(a["being"]), ident(a["id"]), ident(a["fact"])
        lie = text(a["lie"], 160)
        lis = idset(a, "listeners")
        x = self.known(b)
        need(f in x["facts"])
        need(i in x["covers"] or len(x["covers"]) < self.lim["covers"])
        x["covers"][i] = {"fact": f, "lie": lie, "listeners": lis}
        return {"being": b, "fact": f, "id": i, "listeners": lis}

    def op_uncover(self, a: dict) -> dict:
        keys(a, ["being", "id"], [])
        b, i = ident(a["being"]), ident(a["id"])
        x = self.known(b)
        need(i in x["covers"])
        del x["covers"][i]
        return {"being": b, "id": i, "removed": True}

    def op_secret(self, a: dict) -> dict:
        keys(a, ["id", "keys"], [])
        i = ident(a["id"])
        ks = key_words(a)
        need(i in self.secrets or len(self.secrets) < self.lim["secrets"])
        self.secrets[i] = ks
        return {"id": i, "keys": ks}

    def masks(self, x: dict, lis: str) -> Dict[str, dict]:
        """Fact id -> the first (by id) active cover masking it for this listener."""
        out: Dict[str, dict] = {}
        for i in sorted(x["covers"]):
            c = x["covers"][i]
            if (not c["listeners"] or lis in c["listeners"]) and c["fact"] not in out:
                out[c["fact"]] = {"cover": i, "lie": c["lie"]}
        return out

    def op_frame_facts(self, a: dict) -> dict:
        keys(a, ["being", "listener", "facts"], [])
        b, lis = ident(a["being"]), ident(a["listener"])
        ids = idset(a, "facts")
        x = self.known(b)
        need(all(i in x["facts"] for i in ids))
        m = self.masks(x, lis)
        out = []
        for i in ids:
            f = x["facts"][i]
            if i in m:
                out.append({"claim": m[i]["lie"], "confidence": None, "cover": m[i]["cover"], "framing": "cover",
                            "from": None, "id": i, "previous": None})
            else:
                out.append({"claim": f["claim"], "confidence": f["confidence"], "cover": None,
                            "framing": FRAMING[f["source"]], "from": f["from"], "id": i, "previous": f["previous"]})
        return {"being": b, "facts": out, "listener": lis}

    def op_gate(self, a: dict) -> dict:
        keys(a, ["being", "listener", "reply"], ["fallback"])
        b, lis = ident(a["being"]), ident(a["listener"])
        reply = text(a["reply"], 2000)
        fb = None if a.get("fallback") is None else text(a["fallback"], 160)
        x = self.known(b)
        said = set(words(reply))
        leaks = []
        m = self.masks(x, lis)
        for f in sorted(m):
            told = set(words(m[f]["lie"]))  # words of the cover story itself may be said
            for w in x["facts"][f]["keys"]:
                if w in said and w not in told:
                    leaks.append({"id": f, "kind": "cover", "word": w})
        for s in sorted(self.secrets):
            if s in x["facts"]:
                continue
            for w in self.secrets[s]:
                if w in said:
                    leaks.append({"id": s, "kind": "unknown", "word": w})
        if not leaks:
            return {"being": b, "fallback": None, "leaks": [], "listener": lis, "ok": True}
        cover = next((m[l["id"]]["lie"] for l in leaks if l["kind"] == "cover"), None)
        fallback = cover if cover is not None else fb if fb is not None else DEFAULT_FALLBACK
        return {"being": b, "fallback": fallback, "leaks": leaks, "listener": lis, "ok": False}

    def workflow(self, request: Any) -> dict:
        table = {"tick": self.op_tick, "episode": self.op_episode, "cue": self.op_cue, "surface": self.op_surface,
                 "fact": self.op_fact, "cover": self.op_cover, "uncover": self.op_uncover, "secret": self.op_secret,
                 "frame_facts": self.op_frame_facts, "gate": self.op_gate}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Cueing().workflow(request)
