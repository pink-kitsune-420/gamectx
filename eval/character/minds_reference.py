"""Independent original reference for mind architectures (pass 4, stage 2). Python standard library
only; nothing here calls a model.

A mind architecture says what kind of mind a being has: where its memory lives (`individual`,
`shared` in a pool, or `none`), which affect channels exist, whether it has language (`full`,
`signals`, `none`), whether it can deceive (`none`, `concealment`, `full`), how deep its theory
of mind goes (0 others are objects, 1 others want and know, 2 others model me), its subjective
time scale (millionths, 1,000,000 = human) and an optional fixation that pins its top motive.

Five minds are built in (`human`, `hive_tender`, `steward_intelligence`, `herd_animal`,
`hearth_spirit`), all original. `assign` gives a being a mind (unassigned beings are human).
`admit` checks one engine request a host (or a model proposal) wants to run for a being, before
it reaches the engine: it refuses what the mind cannot do (a cover story from a being that
cannot lie or cannot model others, speech from a being without language, memory for a mindless
being), strips inputs for affect channels the mind lacks, routes shared memory to the pool, and
stretches time windows by the time scale. Existing engines are untouched. `fixate` pins a fixed
obsession at the top of a motive list until `release`; `hear` says how much of an utterance a
being takes in (words, tone, or nothing).
"""
import pathlib
import sys
from typing import Any, Dict, List, Optional

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_uint, uint
from growth_reference import run_limited
from limits_reference import Limits
from pragmatics_reference import ACTS

MAX_AT = 1_000_000_000_000
HUMAN_SCALE = 1_000_000
SPEC = [("minds", 16, 4096), ("beings", 1024, 1_000_000)]
CHANNELS = ["affection", "anger", "fear", "guilt", "joy", "resentment", "shame", "shock"]
MEMORY = ["individual", "shared", "none"]
LANGUAGE = ["full", "signals", "none"]
DECEPTION = ["none", "concealment", "full"]
KINDS = ["cueing", "pragmatics", "embodiment"]
BUILTIN = {
    "human": {"memory": "individual", "affect": list(CHANNELS), "language": "full", "deception": "full",
              "theory_of_mind": 2, "time_scale": HUMAN_SCALE, "fixation": None},
    "hive_tender": {"memory": "shared", "affect": ["fear", "joy"], "language": "signals", "deception": "none",
                    "theory_of_mind": 1, "time_scale": HUMAN_SCALE, "fixation": None},
    "steward_intelligence": {"memory": "individual", "affect": ["shock"], "language": "full",
                             "deception": "concealment", "theory_of_mind": 2, "time_scale": 100_000, "fixation": None},
    "herd_animal": {"memory": "individual", "affect": ["affection", "anger", "fear", "shock"], "language": "signals",
                    "deception": "none", "theory_of_mind": 0, "time_scale": HUMAN_SCALE, "fixation": None},
    "hearth_spirit": {"memory": "individual", "affect": ["affection", "anger", "guilt", "resentment", "shame"],
                      "language": "full", "deception": "concealment", "theory_of_mind": 1, "time_scale": 20_000_000,
                      "fixation": {"goal": "keep_the_household", "strength": 90}},
}
TONE = {"threat": "hostile", "accusation": "hostile", "greeting": "friendly", "compliment": "friendly",
        "apology": "friendly", "offer_or_transfer": "friendly", "request_help": "pleading", "farewell": "parting"}
MEMORY_OPS = ["episode", "cue", "surface", "fact", "cover", "uncover", "frame_facts", "gate"]
SPEECH_OPS = ["parse", "validate", "agenda", "open", "interrupt", "resume", "close", "threads", "stance"]
WORD_OPS = ["parse", "validate", "agenda"]
# Engine inputs that carry an affect channel: (kind, op) -> [(arg, channel)].
AFFECT_ARGS = {("embodiment", "react"): [("fear", "fear"), ("anger", "anger"), ("shock", "shock"),
                                         ("guilt", "guilt")],
               ("embodiment", "voice"): [("fear", "fear"), ("affection", "affection")]}
ATTITUDE_ARGS = [("affection", "affection"), ("resentment", "resentment"), ("fear", "fear")]


def scale(v: int, s: int) -> int:
    return max(1, v * s // HUMAN_SCALE)


class Minds:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.minds: Dict[str, dict] = {}
        self.beings: Dict[str, dict] = {}
        self.clock = 0

    def mind(self, mid: str) -> dict:
        if mid in BUILTIN:
            return BUILTIN[mid]
        need(mid in self.minds)
        return self.minds[mid]

    def of(self, b: str) -> dict:
        x = self.beings.get(b)
        return {"mind": "human", "pool": None, "released": False} if x is None else x

    def op_tick(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        self.clock = now
        return {"now": now}

    def op_define(self, a: dict) -> dict:
        keys(a, ["id", "memory", "affect", "language", "deception", "theory_of_mind"], ["time_scale", "fixation"])
        mid = ident(a["id"])
        need(mid not in BUILTIN)
        m = {}
        for k, opts in (("memory", MEMORY), ("language", LANGUAGE), ("deception", DECEPTION)):
            need(isinstance(a[k], str) and a[k] in opts)
            m[k] = a[k]
        aff = a["affect"]
        need(isinstance(aff, list) and len(aff) <= len(CHANNELS))
        chans = [ident(x) for x in aff]
        need(len(set(chans)) == len(chans) and all(c in CHANNELS for c in chans))
        m["affect"] = sorted(chans)
        m["theory_of_mind"] = uint(a["theory_of_mind"], 0, 2)
        m["time_scale"] = opt_uint(a, "time_scale", 1, 1_000_000_000, HUMAN_SCALE)
        fx = a.get("fixation")
        if fx is not None:
            keys(fx, ["goal", "strength"], [])
            fx = {"goal": ident(fx["goal"]), "strength": uint(fx["strength"], 1, 100)}
        m["fixation"] = fx
        need(mid in self.minds or len(self.minds) < self.lim["minds"])
        # A redefinition keeps every assigned being consistent: pooled exactly when memory is shared.
        need(all((st["pool"] is not None) == (m["memory"] == "shared")
                 for st in self.beings.values() if st["mind"] == mid))
        self.minds[mid] = m
        return {"id": mid}

    def op_show(self, a: dict) -> dict:
        keys(a, ["id"], [])
        mid = ident(a["id"])
        m = self.mind(mid)
        return {"affect": list(m["affect"]), "builtin": mid in BUILTIN, "deception": m["deception"],
                "fixation": None if m["fixation"] is None else dict(m["fixation"]), "id": mid,
                "language": m["language"], "memory": m["memory"], "theory_of_mind": m["theory_of_mind"],
                "time_scale": m["time_scale"]}

    def op_assign(self, a: dict) -> dict:
        keys(a, ["being", "mind"], ["pool"])
        b, mid = ident(a["being"]), ident(a["mind"])
        m = self.mind(mid)
        pool = None if a.get("pool") is None else ident(a["pool"])
        need((pool is not None) == (m["memory"] == "shared"))
        need(b in self.beings or len(self.beings) < self.lim["beings"])
        self.beings[b] = {"mind": mid, "pool": pool, "released": False}
        return {"being": b, "mind": mid, "pool": pool}

    def op_admit(self, a: dict) -> dict:
        keys(a, ["being", "kind", "op", "args"], [])
        b, kind, op = ident(a["being"]), ident(a["kind"]), ident(a["op"])
        need(kind in KINDS)
        args = a["args"]
        need(isinstance(args, dict))
        st = self.of(b)
        m = self.mind(st["mind"])
        out = dict(args)
        reasons: List[str] = []
        stripped: List[str] = []
        target = b
        if kind == "cueing":
            if op in MEMORY_OPS and m["memory"] == "none":
                reasons.append("no_memory")
            if op == "cover":
                if m["deception"] != "full":
                    reasons.append("cannot_lie")
                if m["theory_of_mind"] < 2:
                    reasons.append("no_theory_of_mind")
            if m["memory"] == "shared" and "being" in out:
                out["being"] = st["pool"]
                target = st["pool"]
            if op == "episode" and isinstance(out.get("emotion"), str) and out["emotion"] not in m["affect"]:
                del out["emotion"]
                stripped.append("emotion")
            if op == "cue":
                cd = out.get("cooldown")
                cd = 24 if cd is None else cd
                if isinstance(cd, int) and not isinstance(cd, bool) and 0 <= cd <= MAX_AT:
                    out["cooldown"] = scale(cd, m["time_scale"])
        elif kind == "pragmatics":
            if op in SPEECH_OPS and m["language"] == "none":
                reasons.append("no_language")
            if op in WORD_OPS and m["language"] == "signals":
                reasons.append("signals_only")
            if op == "resume":
                tph = out.get("ticks_per_hour")
                if isinstance(tph, int) and not isinstance(tph, bool) and 1 <= tph <= MAX_AT:
                    out["ticks_per_hour"] = scale(tph, m["time_scale"])
            if op == "stance" and m["theory_of_mind"] == 0 and out.get("bystanders") is not None:
                del out["bystanders"]
                stripped.append("bystanders")
        else:
            if op == "voice" and m["language"] == "none":
                reasons.append("no_language")
            if m["deception"] == "none" and ((op == "react" and out.get("lying") is True)
                                             or (op in ("tension", "voice") and out.get("deceiving") is True)):
                reasons.append("cannot_lie")
            for arg, ch in AFFECT_ARGS.get((kind, op), []):
                if ch not in m["affect"] and arg in out:
                    del out[arg]
                    stripped.append(arg)
            if op == "react" and m["theory_of_mind"] == 0 and "bystander_untrusted" in out:
                del out["bystander_untrusted"]
                stripped.append("bystander_untrusted")
            if op == "tension" and isinstance(out.get("attitudes"), list):
                rows = []
                for x in out["attitudes"]:
                    if isinstance(x, dict):
                        x = dict(x)
                        for arg, ch in ATTITUDE_ARGS:
                            if ch not in m["affect"] and arg in x:
                                del x[arg]
                                if "attitudes." + arg not in stripped:
                                    stripped.append("attitudes." + arg)
                    rows.append(x)
                out["attitudes"] = rows
        allowed = not reasons
        return {"allowed": allowed, "args": out if allowed else None, "being": b, "kind": kind,
                "mind": st["mind"], "op": op, "reasons": sorted(reasons), "stripped": sorted(stripped),
                "target": target}

    def op_fixate(self, a: dict) -> dict:
        keys(a, ["being", "motives"], [])
        b = ident(a["being"])
        rows = a["motives"]
        need(isinstance(rows, list) and len(rows) <= 16)
        ms, seen = [], set()
        for x in rows:
            keys(x, ["id", "score"], [])
            mid = ident(x["id"])
            need(mid not in seen)
            seen.add(mid)
            ms.append({"id": mid, "score": uint(x["score"], 0, 100)})
        st = self.of(b)
        fx = self.mind(st["mind"])["fixation"]
        if fx is None or st["released"]:
            return {"being": b, "fixated": False, "motives": ms, "pinned": None}
        top = max([x["score"] for x in ms] + [0])
        pinned = {"id": fx["goal"], "score": max(fx["strength"], top)}
        return {"being": b, "fixated": True, "motives": [pinned] + [x for x in ms if x["id"] != fx["goal"]],
                "pinned": fx["goal"]}

    def op_release(self, a: dict) -> dict:
        keys(a, ["being"], [])
        b = ident(a["being"])
        need(b in self.beings and self.mind(self.beings[b]["mind"])["fixation"] is not None)
        self.beings[b]["released"] = True
        return {"being": b, "released": True}

    def op_hear(self, a: dict) -> dict:
        keys(a, ["being", "acts"], [])
        b = ident(a["being"])
        rows = a["acts"]
        need(isinstance(rows, list) and 1 <= len(rows) <= 4)
        acts = [ident(x) for x in rows]
        need(all(x in ACTS for x in acts))
        lang = self.mind(self.of(b)["mind"])["language"]
        if lang == "full":
            return {"being": b, "tones": [TONE.get(x, "neutral") for x in acts], "understood": "words", "acts": acts}
        if lang == "signals":
            return {"being": b, "tones": [TONE.get(x, "neutral") for x in acts], "understood": "tone", "acts": []}
        return {"being": b, "tones": [], "understood": "nothing", "acts": []}

    def workflow(self, request: Any) -> dict:
        table = {"tick": self.op_tick, "define": self.op_define, "show": self.op_show, "assign": self.op_assign,
                 "admit": self.op_admit, "fixate": self.op_fixate, "release": self.op_release, "hear": self.op_hear}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Minds().workflow(request)
