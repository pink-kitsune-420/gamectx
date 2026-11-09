"""Independent original reference for ambivalence, embodied micro-reactions and voice
modulation (pass 3, stage 4). Python standard library only; nothing here calls a model.

Tension (G1): `tension` reads three sources of inner conflict: a motive near-tie (the top two
motives within 12 points and from different domains), ambivalence toward someone (affection and
resentment both at least 40, or respect and fear both at least 50), and a norm the chosen action
breaks (under duress or not), plus whether the being is deceiving. It returns one tension mode
(`none`, `reluctant_compliance`, `torn_hesitation`, `bittersweet_warmth`, `resentful_obligation`,
`guilty_concealment`) with its effects: extra task failure risk (per mille), hesitation ticks and
a speech tone.

Micro-reactions (G2): `react` picks a posture, a gaze and a hand activity from fear, anger,
shock, guilt, lying, cooperativeness, task state and bystanders, by fixed first-match rules, and
whether the turn should be silent (hostile refusal or extreme shock). It remembers each being's
last reaction and reports the channels repeated within a window so the host can vary wording.

Voice (G3): `voice` compresses replies to one terse sentence under fatigue, pain, arousal or task
urgency, picks a pacing (breathless, trembling, hesitant, clipped, warm and relaxed, or steady) and
gives prosody hints (speed and volume per mille, pitch shift in tenths of a semitone, pause in ms)
for line selection or speech synthesis; a whisper lowers the volume.
"""
import pathlib
import sys
from typing import Any, Dict, List, Optional

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_bool, opt_uint, uint
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
SPEC = [("beings", 1024, 1_000_000), ("attitudes", 8, 64)]
MODES = ["none", "reluctant_compliance", "torn_hesitation", "bittersweet_warmth", "resentful_obligation",
         "guilty_concealment"]
EFFECTS = {"none": (0, 0, "plain"), "reluctant_compliance": (50, 1, "grudging"), "torn_hesitation": (80, 2, "hesitant"),
           "bittersweet_warmth": (0, 0, "tender"), "resentful_obligation": (30, 0, "curt"),
           "guilty_concealment": (60, 1, "evasive")}
COOP = ["eager", "cooperative", "guarded", "reluctant", "deflecting", "hostile_refusal"]
PACING = {"steady": (1000, 0, 250, 1000), "clipped": (1150, 0, 120, 1050), "breathless": (1200, 20, 80, 900),
          "hesitant": (850, -10, 600, 850), "trembling": (1050, 30, 350, 750), "warm_relaxed": (920, -10, 400, 950)}
VERBOSITY = ["terse", "normal", "full"]
PRIVACY = ["deflect_due_to_bystanders", "open", "whispered"]
WHISPER_VOLUME = 400


def opt_ident(d: dict, k: str) -> Optional[str]:
    return None if d.get(k) is None else ident(d[k])


def one_of(d: dict, k: str, options: List[str], default: Optional[str]) -> Optional[str]:
    v = opt_ident(d, k)
    if v is None:
        return default
    need(v in options)
    return v


class Embodiment:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.last: Dict[str, dict] = {}
        self.clock = 0

    def op_tick(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        self.clock = now
        return {"now": now}

    # ---------------------------------------------------------------- tension

    def op_tension(self, a: dict) -> dict:
        keys(a, ["being"], ["motives", "attitudes", "norm_breach", "deceiving"])
        b = ident(a["being"])
        near = None
        mo = a.get("motives")
        if mo is not None:
            keys(mo, ["top", "runner_up"], [])
            rows = []
            for k in ("top", "runner_up"):
                m = mo[k]
                keys(m, ["id", "domain", "score"], [])
                rows.append((ident(m["id"]), ident(m["domain"]), uint(m["score"], 0, 100)))
            (ti, td, ts), (ri, rd, rs) = rows
            need(ti != ri and rs <= ts)
            if ts - rs <= 12 and td != rd:
                near = {"gap": ts - rs, "runner_up": ri, "top": ti}
        rows = a.get("attitudes")
        rows = [] if rows is None else rows
        need(isinstance(rows, list) and len(rows) <= self.lim["attitudes"])
        warm, cold, awe, seen = [], [], [], set()
        for x in rows:
            keys(x, ["toward"], ["affection", "resentment", "respect", "fear"])
            w = ident(x["toward"])
            need(w not in seen)
            seen.add(w)
            aff, res = opt_uint(x, "affection", 0, 100, 0), opt_uint(x, "resentment", 0, 100, 0)
            rsp, fear = opt_uint(x, "respect", 0, 100, 0), opt_uint(x, "fear", 0, 100, 0)
            if aff >= 40 and res >= 40:
                (warm if aff >= res else cold).append(w)
            elif rsp >= 50 and fear >= 50:
                awe.append(w)
        nb = a.get("norm_breach")
        breach = None
        if nb is not None:
            keys(nb, ["norm", "severity"], ["duress"])
            breach = {"duress": opt_bool(nb, "duress", False), "norm": ident(nb["norm"]),
                      "severity": uint(nb["severity"], 1, 100)}
        deceiving = opt_bool(a, "deceiving", False)
        if deceiving or (breach is not None and not breach["duress"]):
            mode = "guilty_concealment"
        elif breach is not None:
            mode = "reluctant_compliance"
        elif warm:
            mode = "bittersweet_warmth"
        elif cold:
            mode = "resentful_obligation"
        elif awe or near is not None:
            mode = "torn_hesitation"
        else:
            mode = "none"
        risk, wait, tone = EFFECTS[mode]
        return {"ambivalent": sorted(warm + cold + awe), "being": b, "conscience": breach, "deceiving": deceiving,
                "effects": {"failure_risk": risk, "hesitation_ticks": wait, "tone": tone}, "mode": mode,
                "near_tie": near}

    # ---------------------------------------------------------------- micro-reactions

    def op_react(self, a: dict) -> dict:
        keys(a, ["being"], ["fear", "anger", "shock", "guilt", "lying", "cooperativeness", "tension", "task",
                            "holding", "offering", "threat_present", "bystander_untrusted", "window"])
        b = ident(a["being"])
        fear, anger = opt_uint(a, "fear", 0, 100, 0), opt_uint(a, "anger", 0, 100, 0)
        shock, guilt = opt_uint(a, "shock", 0, 100, 0), opt_uint(a, "guilt", 0, 100, 0)
        lying = opt_bool(a, "lying", False)
        coop = one_of(a, "cooperativeness", COOP, "cooperative")
        tension = one_of(a, "tension", MODES, "none")
        task = one_of(a, "task", ["none", "active", "urgent"], "none")
        holding, offering = opt_ident(a, "holding"), opt_ident(a, "offering")
        threat = opt_bool(a, "threat_present", False)
        watcher = opt_bool(a, "bystander_untrusted", False)
        window = opt_uint(a, "window", 0, MAX_AT, 3)
        need(b in self.last or len(self.last) < self.lim["beings"])

        if fear >= 70:
            posture = "steps_back"
        elif anger >= 70:
            posture = "squares_shoulders"
        elif shock >= 80 or coop == "hostile_refusal":
            posture = "turns_away"
        elif guilt >= 60 or tension == "torn_hesitation":
            posture = "slumps"
        elif coop == "eager":
            posture = "leans_in"
        else:
            posture = None

        if lying or guilt >= 50 or tension == "guilty_concealment":
            gaze = "avoids_eye_contact"
        elif fear >= 70:
            gaze = "glances_at_exit"
        elif watcher:
            gaze = "watches_bystander"
        elif shock >= 80 and holding is not None:
            gaze = "stares_at_held_item"
        elif coop in ("eager", "cooperative"):
            gaze = "meets_eyes"
        else:
            gaze = None

        if threat and anger >= 70:
            hand = "reaches_for_weapon"
        elif anger >= 50:
            hand = "clenches_fist"
        elif offering is not None:
            hand = "offers_item"
        elif task == "urgent":
            hand = "continues_working"
        elif task == "active" and shock >= 50:
            hand = "sets_down_tool"
        elif task == "active":
            hand = "pauses_work"
        else:
            hand = None

        silent = coop == "hostile_refusal" or shock >= 90
        now = {"gaze": gaze, "hand": hand, "posture": posture}
        prev = self.last.get(b)
        repeated = []
        if prev is not None and self.clock - prev["at"] <= window:
            repeated = [k for k in ("gaze", "hand", "posture") if now[k] is not None and now[k] == prev[k]]
        self.last[b] = {**now, "at": self.clock}
        return {"being": b, "gaze": gaze, "hand": hand, "item": offering if hand == "offers_item" else
                holding if gaze == "stares_at_held_item" else None, "posture": posture, "repeated": repeated,
                "silent": silent}

    # ---------------------------------------------------------------- voice

    def op_voice(self, a: dict) -> dict:
        keys(a, [], ["fatigue", "pain", "arousal", "task_urgency", "fear", "affection", "exertion", "safe_place",
                     "deceiving", "tension", "base_sentences", "base_verbosity", "privacy"])
        fatigue, pain = opt_uint(a, "fatigue", 0, 100, 0), opt_uint(a, "pain", 0, 100, 0)
        arousal, urg = opt_uint(a, "arousal", 0, 100, 0), opt_uint(a, "task_urgency", 0, 100, 0)
        fear, aff = opt_uint(a, "fear", 0, 100, 0), opt_uint(a, "affection", 0, 100, 0)
        exertion = opt_uint(a, "exertion", 0, 100, 0)
        safe = opt_bool(a, "safe_place", False)
        deceiving = opt_bool(a, "deceiving", False)
        tension = one_of(a, "tension", MODES, "none")
        base_n = opt_uint(a, "base_sentences", 1, 8, 4)
        base_v = one_of(a, "base_verbosity", VERBOSITY, "normal")
        privacy = one_of(a, "privacy", PRIVACY, "open")
        terse = fatigue >= 80 or pain >= 50 or arousal >= 75 or urg >= 80
        if exertion >= 70:
            pacing = "breathless"
        elif fear >= 70:
            pacing = "trembling"
        elif deceiving or tension in ("torn_hesitation", "guilty_concealment"):
            pacing = "hesitant"
        elif terse:
            pacing = "clipped"
        elif aff >= 60 and safe:
            pacing = "warm_relaxed"
        else:
            pacing = "steady"
        speed, pitch, pause, volume = PACING[pacing]
        if privacy == "whispered":
            volume = WHISPER_VOLUME
        return {"max_sentences": 1 if terse else base_n, "pacing": pacing, "pause_ms": pause, "pitch_shift": pitch,
                "speed": speed, "terse": terse, "verbosity": "terse" if terse else base_v, "volume": volume}

    def workflow(self, request: Any) -> dict:
        table = {"tick": self.op_tick, "tension": self.op_tension, "react": self.op_react, "voice": self.op_voice}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Embodiment().workflow(request)
