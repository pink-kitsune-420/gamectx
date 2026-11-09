"""Independent original reference for chemistry, banter and pacing (pass 4, stage 6). Python
standard library only; nothing here calls a model.

A social style is a humour kind (dry, warm, absurd, cutting, none), a tempo (fast, measured, slow),
formality and assertiveness (0-100) and up to eight values. Chemistry between two styled beings is
a fixed function: 50 plus humour fit (a symmetric table), tempo fit (+10 equal, -15 fast with
slow), minus a tenth of the formality gap, assertiveness (-10 when both are 70 or more, +5 when
they differ by 40 or more), +5 per shared value (at most +20) and the pair's history (+4 per
landmark up to +20, +3 per favor up to +15, -8 per breach down to -40), clamped to 0-100. It
gives cooperativeness and initiative biases of (chemistry - 50) / 2 and the percent chance a
playful move lands.

Running jokes are shared moments of a pair with a freshness (millionths): a callback spends
300,000, time restores 2,000 per tick, and a joke used five times is retired. `banter` picks one
move by a fixed first-match rule from the pacing advice, the speaker's mood, the pair's chemistry
and its fresh jokes (a seeded FNV-1a roll over `"<seed>:<speaker>:<listener>:<tick>"` picks among
jokes at least half fresh); a host or model may propose a move instead, and a tease below the
rapport floor (chemistry 60) appraises as an insult.

A scene keeps tension (millionths), the ticks of its last eight beats, recent novelty and
engagement, open threads and a silence budget (3). `pace` advises: `end` when no thread is open,
recent novelty averages under 30 and engagement is falling; `pause` when the last beat moved
tension by 300,000 or more and silence is left; `slow` when tension is 800,000 or more or six or
more of the last beats fell within ten ticks; `escalate` when tension is under 200,000 and recent
novelty averages 50 or more; otherwise `steady`.
"""
import pathlib
import sys
from typing import Any, Dict, List, Optional, Tuple

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_uint, text, uint
from cognition_reference import fnv1a
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
FULL = 1_000_000
SPEC = [("beings", 1024, 1_000_000), ("pairs", 4096, 1_000_000), ("jokes", 8, 64), ("scenes", 64, 100_000)]
HUMOUR = ["dry", "warm", "absurd", "cutting", "none"]
TEMPO = ["fast", "measured", "slow"]
FIT = {("dry", "dry"): 15, ("dry", "warm"): 10, ("absurd", "dry"): 15, ("cutting", "dry"): 5, ("dry", "none"): 0,
       ("warm", "warm"): 20, ("absurd", "warm"): 15, ("cutting", "warm"): -20, ("none", "warm"): 5,
       ("absurd", "absurd"): 20, ("absurd", "cutting"): -10, ("absurd", "none"): -5,
       ("cutting", "cutting"): 10, ("cutting", "none"): -15, ("none", "none"): 0}
MOVES = ["tease", "callback", "escalate", "deflate", "disguised_compliment", "rapport_check", "exit"]
ADVICE = ["end", "pause", "slow", "escalate", "steady"]
BEATS = ["line", "action", "silence", "reveal", "conflict"]
FLOOR = 60
SPEND = 300_000
RESTORE = 2_000
RETIRE = 5
WINDOW = 8


def pair_key(a: str, b: str) -> Tuple[str, str]:
    return (a, b) if a < b else (b, a)


def clamp(v: int, lo: int, hi: int) -> int:
    return max(lo, min(hi, v))


class Chemistry:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.styles: Dict[str, dict] = {}
        self.pairs: Dict[Tuple[str, str], dict] = {}
        self.scenes: Dict[str, dict] = {}
        self.clock = 0

    def style(self, b: str) -> dict:
        need(b in self.styles)
        return self.styles[b]

    def pair(self, a: str, b: str) -> Optional[dict]:
        return self.pairs.get(pair_key(a, b))

    def chemistry(self, a: str, b: str) -> dict:
        sa, sb = self.style(a), self.style(b)
        h = self.pair(a, b) or {"landmarks": 0, "favors": 0, "breaches": 0}
        humour = FIT[tuple(sorted((sa["humour"], sb["humour"])))]
        ta, tb = sa["tempo"], sb["tempo"]
        tempo = 10 if ta == tb else -15 if {ta, tb} == {"fast", "slow"} else 0
        formality = -(abs(sa["formality"] - sb["formality"]) // 10)
        if sa["assertiveness"] >= 70 and sb["assertiveness"] >= 70:
            assertive = -10
        elif abs(sa["assertiveness"] - sb["assertiveness"]) >= 40:
            assertive = 5
        else:
            assertive = 0
        values = min(20, 5 * len(set(sa["values"]) & set(sb["values"])))
        history = (min(20, 4 * h["landmarks"]) + min(15, 3 * h["favors"]) - min(40, 8 * h["breaches"]))
        chem = clamp(50 + humour + tempo + formality + assertive + values + history, 0, 100)
        return {"assertiveness": assertive, "chemistry": chem, "formality": formality, "history": history,
                "humour": humour, "tempo": tempo, "values": values}

    def freshness(self, j: dict) -> int:
        if j["retired"]:
            return 0
        return min(FULL, j["freshness"] + RESTORE * (self.clock - j["at"]))

    def op_tick(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        self.clock = now
        return {"now": now}

    def op_style(self, a: dict) -> dict:
        keys(a, ["being", "humour", "tempo", "formality", "assertiveness"], ["values"])
        b = ident(a["being"])
        need(isinstance(a["humour"], str) and a["humour"] in HUMOUR)
        need(isinstance(a["tempo"], str) and a["tempo"] in TEMPO)
        formality, assertive = uint(a["formality"], 0, 100), uint(a["assertiveness"], 0, 100)
        vals = [] if a.get("values") is None else a["values"]
        need(isinstance(vals, list) and len(vals) <= 8)
        vals = [ident(x) for x in vals]
        need(len(set(vals)) == len(vals))
        need(b in self.styles or len(self.styles) < self.lim["beings"])
        self.styles[b] = {"humour": a["humour"], "tempo": a["tempo"], "formality": formality,
                          "assertiveness": assertive, "values": sorted(vals)}
        return {"assertiveness": assertive, "being": b, "formality": formality, "humour": a["humour"],
                "tempo": a["tempo"], "values": sorted(vals)}

    def ensure_pair(self, a: str, b: str) -> dict:
        k = pair_key(a, b)
        if k not in self.pairs:
            need(len(self.pairs) < self.lim["pairs"])
            self.pairs[k] = {"landmarks": 0, "favors": 0, "breaches": 0, "jokes": {}}
        return self.pairs[k]

    def op_history(self, a: dict) -> dict:
        keys(a, ["a", "b"], ["landmarks", "favors", "breaches"])
        x, y = ident(a["a"]), ident(a["b"])
        need(x != y)
        counts = {k: opt_uint(a, k, 0, 1000, 0) for k in ("landmarks", "favors", "breaches")}
        p = self.ensure_pair(x, y)
        p.update(counts)
        k = pair_key(x, y)
        return {"a": k[0], "b": k[1], **counts}

    def op_pair(self, a: dict) -> dict:
        keys(a, ["a", "b"], [])
        x, y = ident(a["a"]), ident(a["b"])
        need(x != y)
        c = self.chemistry(x, y)
        k = pair_key(x, y)
        p = self.pair(x, y)
        jokes = [] if p is None else [{"freshness": self.freshness(j), "id": jid, "retired": j["retired"],
                                       "uses": j["uses"]} for jid, j in sorted(p["jokes"].items())]
        bias = (c["chemistry"] - 50) // 2
        return {"a": k[0], "b": k[1], "chemistry": c["chemistry"], "components": c,
                "cooperativeness_bias": bias, "initiative_bias": bias, "jokes": jokes,
                "playful_lands_percent": c["chemistry"]}

    def op_joke(self, a: dict) -> dict:
        keys(a, ["a", "b", "id", "moment"], [])
        x, y = ident(a["a"]), ident(a["b"])
        need(x != y)
        jid, moment = ident(a["id"]), text(a["moment"], 80)
        self.style(x), self.style(y)
        k = pair_key(x, y)
        p = self.pairs.get(k)
        need(p is not None or len(self.pairs) < self.lim["pairs"])
        js = {} if p is None else p["jokes"]
        need(jid not in js and len(js) < self.lim["jokes"])
        p = self.ensure_pair(x, y)
        p["jokes"][jid] = {"moment": moment, "freshness": FULL, "at": self.clock, "uses": 0, "retired": False}
        return {"a": k[0], "b": k[1], "freshness": FULL, "id": jid, "moment": moment}

    def op_banter(self, a: dict) -> dict:
        keys(a, ["speaker", "listener", "mood", "pacing"], ["seed", "move"])
        s, l = ident(a["speaker"]), ident(a["listener"])
        need(s != l)
        mood = a["mood"]
        need(isinstance(mood, int) and not isinstance(mood, bool) and -100 <= mood <= 100)
        need(isinstance(a["pacing"], str) and a["pacing"] in ADVICE)
        pacing = a["pacing"]
        seed = opt_uint(a, "seed", 0, MAX_AT, 0)
        proposed = a.get("move")
        need(proposed is None or (isinstance(proposed, str) and proposed in MOVES))
        chem = self.chemistry(s, l)["chemistry"]
        p = self.pair(s, l)
        jokes = {} if p is None else p["jokes"]
        fresh = sorted(jid for jid, j in jokes.items() if self.freshness(j) >= FULL // 2)
        if proposed is not None:
            move = proposed
            need(move != "callback" or fresh)
        elif pacing == "end":
            move = "exit"
        elif mood < -30:
            move = "rapport_check" if chem >= 40 else "exit"
        elif fresh and pacing != "slow":
            move = "callback"
        elif pacing == "escalate" and chem >= FLOOR:
            move = "escalate"
        elif self.style(s)["humour"] in ("dry", "cutting") and chem >= FLOOR:
            move = "tease"
        elif pacing in ("slow", "pause"):
            move = "deflate"
        elif self.style(s)["humour"] == "warm":
            move = "disguised_compliment"
        else:
            move = "rapport_check"
        joke = None
        if move == "callback":
            jid = fresh[fnv1a(f"{seed}:{s}:{l}:{self.clock}") % len(fresh)]
            j = jokes[jid]
            fr = max(0, self.freshness(j) - SPEND)
            j["freshness"], j["at"] = fr, self.clock
            j["uses"] += 1
            j["retired"] = j["uses"] >= RETIRE
            joke = {"freshness": self.freshness(j), "id": jid, "moment": j["moment"], "retired": j["retired"],
                    "uses": j["uses"]}
        if move == "tease":
            lands = chem >= FLOOR
            appraisal = "playful" if lands else "insult"
        elif move in ("escalate", "callback"):
            lands = chem >= 50
            appraisal = "playful" if lands else "awkward"
        elif move == "disguised_compliment":
            lands, appraisal = True, "warm"
        else:
            lands, appraisal = True, "neutral"
        return {"appraisal": appraisal, "chemistry": chem, "joke": joke, "lands": lands, "listener": l, "move": move,
                "proposed": proposed is not None, "speaker": s}

    def op_scene(self, a: dict) -> dict:
        keys(a, ["id"], ["tension", "silence"])
        sid = ident(a["id"])
        tension = opt_uint(a, "tension", 0, FULL, 0)
        silence = opt_uint(a, "silence", 0, 16, 3)
        need(sid not in self.scenes and len(self.scenes) < self.lim["scenes"])
        self.scenes[sid] = {"tension": tension, "last_delta": 0, "silence": silence, "beats": [], "threads": 0}
        return self.advice(sid)

    def op_beat(self, a: dict) -> dict:
        keys(a, ["scene", "kind", "tension", "novelty", "engagement", "threads_open"], [])
        sid = ident(a["scene"])
        need(sid in self.scenes)
        need(isinstance(a["kind"], str) and a["kind"] in BEATS)
        d = a["tension"]
        need(isinstance(d, int) and not isinstance(d, bool) and -100 <= d <= 100)
        nov, eng = uint(a["novelty"], 0, 100), uint(a["engagement"], 0, 100)
        threads = uint(a["threads_open"], 0, 16)
        sc = self.scenes[sid]
        need(a["kind"] != "silence" or sc["silence"] > 0)
        before = sc["tension"]
        sc["tension"] = clamp(before + d * 10_000, 0, FULL)
        sc["last_delta"] = abs(sc["tension"] - before)
        if a["kind"] == "silence":
            sc["silence"] -= 1
        sc["beats"] = (sc["beats"] + [{"at": self.clock, "novelty": nov, "engagement": eng}])[-WINDOW:]
        sc["threads"] = threads
        return self.advice(sid)

    def advice(self, sid: str) -> dict:
        sc = self.scenes[sid]
        recent = sc["beats"][-4:]
        novelty = sum(b["novelty"] for b in recent) // len(recent) if recent else 0
        falling = len(recent) >= 2 and recent[-1]["engagement"] < recent[0]["engagement"]
        dense = sum(1 for b in sc["beats"] if self.clock - b["at"] <= 10)
        if recent and sc["threads"] == 0 and novelty < 30 and falling:
            adv = "end"
        elif sc["last_delta"] >= 300_000 and sc["silence"] > 0:
            adv = "pause"
        elif sc["tension"] >= 800_000 or dense >= 6:
            adv = "slow"
        elif sc["tension"] < 200_000 and recent and novelty >= 50:
            adv = "escalate"
        else:
            adv = "steady"
        return {"advice": adv, "density": dense, "falling": falling, "novelty": novelty, "scene": sid,
                "silence": sc["silence"], "tension": sc["tension"], "threads_open": sc["threads"]}

    def op_pace(self, a: dict) -> dict:
        keys(a, ["scene"], [])
        sid = ident(a["scene"])
        need(sid in self.scenes)
        return self.advice(sid)

    def workflow(self, request: Any) -> dict:
        table = {"tick": self.op_tick, "style": self.op_style, "history": self.op_history, "pair": self.op_pair,
                 "joke": self.op_joke, "banter": self.op_banter, "scene": self.op_scene, "beat": self.op_beat,
                 "pace": self.op_pace}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Chemistry().workflow(request)
