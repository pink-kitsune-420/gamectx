"""Independent original reference for the unified turn frame and salience ranker (pass 3, stage 1).
Python standard library only; nothing here calls a model.

Three host faculties that put a character in the moment:

- trajectory sense (E4): `observe` keeps a bounded window of recent values per being and key
  (trust toward someone, mood, a task's outcome as +1/-1); `trend` reports the change over a
  window and labels it (`acute_rise` / `acute_drop` past a shock size, `settled_high` /
  `settled_low` when a strong value has not changed for a while, `rising` / `falling`, `steady`);
  `streak` counts consecutive successes or failures (`flow`, `frustration_spiral`); `time` turns
  a tick gap into a human anchor (`just_now` .. `long_ago`, or `due_in` / `overdue`);
- the turn frame (E1): `frame` takes the slices the host read from the other engines (body,
  motive, task, affect, listener model, off-screen, novelty, bystanders) and returns one bounded,
  validated struct with derived fields: urgent body items, task urgency, time anchors, trends
  for this being, and the untrusted bystanders in earshot;
- the salience ranker (E2): `rank` scores every candidate context item 0..1000 from urgency,
  listener relevance, utterance cues, task relevance, mood congruence and recency, then packs
  whole causal groups by salience per byte into a byte budget; mandatory groups are always kept.
"""
import pathlib
import sys
from typing import Any, Dict, List, Optional

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_bool, opt_uint, text, uint
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
M = 1_000_000
SPEC = [("beings", 1024, 1_000_000), ("keys", 16, 256), ("samples", 16, 256), ("items", 64, 4096)]
MAX_LIST = 16
WEIGHTS = [("urgency", 30), ("listener", 20), ("utterance", 20), ("task", 15), ("mood", 10), ("recency", 5)]
POLARITY = ["favor", "grievance", "neutral"]
INTERRUPT = ["any", "never", "urgent"]


def sint(v: Any, lo: int, hi: int) -> int:
    need(isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi)
    return v


def blist(d: dict, k: str, hi: int) -> list:
    v = d.get(k)
    v = [] if v is None else v
    need(isinstance(v, list) and len(v) <= hi)
    return v


def idset(d: dict, k: str, hi: int = MAX_LIST) -> List[str]:
    xs = [ident(x) for x in blist(d, k, hi)]
    need(len(set(xs)) == len(xs))
    return sorted(xs)


def opt_sint(d: dict, k: str, lo: int, hi: int, default: int) -> int:
    return default if d.get(k) is None else sint(d[k], lo, hi)


def opt_ident(d: dict, k: str) -> Optional[str]:
    return None if d.get(k) is None else ident(d[k])


def time_label(at: int, now: int, tph: int, due: bool) -> dict:
    """Human anchor for a tick gap; `tph` ticks make one hour."""
    if due:
        if now <= at:
            return {"hours": (at - now) // tph, "label": "due_in"}
        return {"hours": (now - at) // tph, "label": "overdue"}
    need(at <= now)
    ago = now - at
    minutes = ago * 60 // tph
    if ago == 0:
        label = "just_now"
    elif minutes < 15:
        label = "moments_ago"
    elif minutes < 8 * 60:
        label = "earlier_this_shift"
    elif minutes < 24 * 60:
        label = "earlier_today"
    elif minutes < 48 * 60:
        label = "yesterday"
    elif minutes < 14 * 24 * 60:
        label = "days_ago"
    else:
        label = "long_ago"
    return {"hours": ago // tph, "label": label}


class Frame:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.beings: Dict[str, dict] = {}  # being -> {"keys": {key: [(at, value)]}, "streak": int}
        self.clock = 0

    def being_room(self, b: str) -> None:
        need(b in self.beings or len(self.beings) < self.lim["beings"])

    def trend_of(self, b: str, k: str, window: int, shock: int, level: int, settle: int) -> dict:
        xs = self.beings[b]["keys"][k]
        at, value = xs[-1]
        # baseline: the newest sample at or before now - window, else the oldest one kept
        base = xs[0][1]
        if self.clock >= window:
            for t, v in xs:
                if t <= self.clock - window:
                    base = v
        delta = value - base
        changed = xs[0][0]  # when the value last changed (the first sample if it never did)
        for i in range(len(xs) - 1, 0, -1):
            if xs[i - 1][1] != xs[i][1]:
                changed = xs[i][0]
                break
        still = self.clock - changed
        if delta >= shock:
            label = "acute_rise"
        elif delta <= -shock:
            label = "acute_drop"
        elif value >= level and still >= settle:
            label = "settled_high"
        elif value <= -level and still >= settle:
            label = "settled_low"
        elif delta > 0:
            label = "rising"
        elif delta < 0:
            label = "falling"
        else:
            label = "steady"
        return {"being": b, "delta": delta, "key": k, "label": label, "samples": len(xs), "unchanged_for": still,
                "value": value}

    # ---------------------------------------------------------------- trajectory ops

    def op_tick(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        self.clock = now
        return {"now": now}

    def op_observe(self, a: dict) -> dict:
        keys(a, ["being", "key", "value"], [])
        b, k = ident(a["being"]), ident(a["key"])
        v = sint(a["value"], -M, M)
        self.being_room(b)
        x = self.beings.get(b)
        need(x is None or k in x["keys"] or len(x["keys"]) < self.lim["keys"])
        x = self.beings.setdefault(b, {"keys": {}, "streak": 0})
        xs = x["keys"].setdefault(k, [])
        xs.append((self.clock, v))
        del xs[:-self.lim["samples"]]
        return {"at": self.clock, "being": b, "key": k, "samples": len(xs), "value": v}

    def op_trend(self, a: dict) -> dict:
        keys(a, ["being", "key"], ["window", "shock", "level", "settle"])
        b, k = ident(a["being"]), ident(a["key"])
        window = opt_uint(a, "window", 0, MAX_AT, 3)
        shock = opt_uint(a, "shock", 1, 2 * M, 30)
        level = opt_uint(a, "level", 0, M, 40)
        settle = opt_uint(a, "settle", 0, MAX_AT, 500)
        need(b in self.beings and k in self.beings[b]["keys"])
        return self.trend_of(b, k, window, shock, level, settle)

    def op_outcome(self, a: dict) -> dict:
        keys(a, ["being", "ok"], [])
        b = ident(a["being"])
        need(isinstance(a["ok"], bool))
        self.being_room(b)
        x = self.beings.setdefault(b, {"keys": {}, "streak": 0})
        s = x["streak"]
        if a["ok"]:
            s = s + 1 if s > 0 else 1
        else:
            s = s - 1 if s < 0 else -1
        x["streak"] = max(-1000, min(1000, s))
        return self.streak_view(b)

    def streak_view(self, b: str) -> dict:
        s = self.beings[b]["streak"]
        label = "flow" if s >= 3 else "frustration_spiral" if s <= -3 else "mixed"
        return {"being": b, "label": label, "streak": s}

    def op_streak(self, a: dict) -> dict:
        keys(a, ["being"], [])
        b = ident(a["being"])
        need(b in self.beings)
        return self.streak_view(b)

    def op_time(self, a: dict) -> dict:
        keys(a, ["at"], ["now", "ticks_per_hour", "due"])
        at = uint(a["at"], 0, MAX_AT)
        now = opt_uint(a, "now", 0, MAX_AT, self.clock)
        tph = opt_uint(a, "ticks_per_hour", 1, 3600, 1)
        due = opt_bool(a, "due", False)
        return {"at": at, "now": now, **time_label(at, now, tph, due)}

    # ---------------------------------------------------------------- frame

    def op_frame(self, a: dict) -> dict:
        keys(a, ["being"], ["listener", "place", "ticks_per_hour", "bystanders", "body", "motive", "task", "affect",
                            "listener_model", "offscreen", "novelty"])
        b = ident(a["being"])
        listener = opt_ident(a, "listener")
        need(listener != b)
        place = opt_ident(a, "place")
        tph = opt_uint(a, "ticks_per_hour", 1, 3600, 1)
        now = self.clock
        out: Dict[str, Any] = {"being": b, "listener": listener, "now": now, "place": place}

        bys = []
        for x in blist(a, "bystanders", MAX_LIST):
            keys(x, ["being"], ["trusted", "earshot"])
            bys.append({"being": ident(x["being"]), "earshot": opt_bool(x, "earshot", True),
                        "trusted": opt_bool(x, "trusted", False)})
        need(len({x["being"] for x in bys}) == len(bys) and all(x["being"] not in (b, listener) for x in bys))
        bys.sort(key=lambda x: x["being"])
        out["bystanders"] = bys
        out["untrusted_in_earshot"] = [x["being"] for x in bys if x["earshot"] and not x["trusted"]]

        body = a.get("body")
        if body is None:
            out["body_urgent"] = None
        else:
            keys(body, [], ["fatigue", "pain", "needs", "conditions", "held", "posture"])
            fatigue = opt_uint(body, "fatigue", 0, 100, 0)
            pain = opt_uint(body, "pain", 0, 100, 0)
            urgent = []
            if fatigue >= 70:
                urgent.append({"item": "fatigue", "level": fatigue})
            if pain >= 50:
                urgent.append({"item": "pain", "level": pain})
            needs = blist(body, "needs", MAX_LIST)
            seen = set()
            for n in needs:
                keys(n, ["id", "level"], ["threshold"])
                i = ident(n["id"])
                need(i not in seen and i not in ("fatigue", "pain"))
                seen.add(i)
                lvl = uint(n["level"], 0, 100)
                if lvl >= opt_uint(n, "threshold", 0, 100, 70):
                    urgent.append({"item": i, "level": lvl})
            urgent.sort(key=lambda x: (-x["level"], x["item"]))
            out["body_urgent"] = {"conditions": idset(body, "conditions"), "fatigue": fatigue,
                                  "held": opt_ident(body, "held"), "pain": pain,
                                  "posture": opt_ident(body, "posture"), "urgent": urgent}

        mo = a.get("motive")
        if mo is None:
            out["active_motive"] = None
        else:
            keys(mo, ["id", "score"], ["reason", "runner_up", "runner_up_score"])
            score = uint(mo["score"], 0, 100)
            ru = opt_ident(mo, "runner_up")
            rus = None if mo.get("runner_up_score") is None else uint(mo["runner_up_score"], 0, score)
            need((ru is None) == (rus is None))
            out["active_motive"] = {"id": ident(mo["id"]), "margin": None if rus is None else score - rus,
                                    "overrides_talk": score >= 90,
                                    "reason": None if mo.get("reason") is None else text(mo["reason"], 80),
                                    "runner_up": ru, "score": score}

        tk = a.get("task")
        if tk is None:
            out["active_task"] = None
        else:
            keys(tk, ["id"], ["step", "progress", "interrupt", "deadline"])
            interrupt = "any" if tk.get("interrupt") is None else ident(tk["interrupt"])
            need(interrupt in INTERRUPT)
            deadline = None if tk.get("deadline") is None else uint(tk["deadline"], 0, MAX_AT)
            due = None if deadline is None else time_label(deadline, now, tph, True)
            pressing = due is not None and (due["label"] == "overdue" or due["hours"] <= 1)
            out["active_task"] = {"deadline": deadline, "due": due, "id": ident(tk["id"]), "interrupt": interrupt,
                                  "pressing": pressing, "progress": opt_uint(tk, "progress", 0, 100, 0),
                                  "step": opt_ident(tk, "step")}

        af = a.get("affect")
        if af is None:
            out["affect_snapshot"] = None
        else:
            keys(af, [], ["valence", "arousal", "label", "emotions"])
            ems = []
            for e in blist(af, "emotions", MAX_LIST):
                keys(e, ["emotion", "value"], ["toward"])
                ems.append({"emotion": ident(e["emotion"]), "toward": opt_ident(e, "toward"),
                            "value": uint(e["value"], 0, 100)})
            need(len({(e["emotion"], e["toward"]) for e in ems}) == len(ems))
            ems.sort(key=lambda e: (-e["value"], e["emotion"], e["toward"] or ""))
            out["affect_snapshot"] = {"arousal": opt_uint(af, "arousal", 0, 100, 0),
                                      "label": opt_ident(af, "label"), "top": ems[:3],
                                      "valence": opt_sint(af, "valence", -100, 100, 0)}

        lm = a.get("listener_model")
        if lm is None:
            out["listener_model"] = None
        else:
            need(listener is not None)
            keys(lm, [], ["stance", "trust", "knows", "wants", "feels", "lies", "overdue"])
            out["listener_model"] = {"feels": idset(lm, "feels"), "knows": idset(lm, "knows"),
                                     "lies": idset(lm, "lies"), "overdue": idset(lm, "overdue"),
                                     "stance": opt_ident(lm, "stance"),
                                     "trust": opt_sint(lm, "trust", -100, 100, 0), "wants": idset(lm, "wants")}

        off = a.get("offscreen")
        if off is None:
            out["recent_offscreen"] = None
        else:
            keys(off, ["summary", "at"], [])
            at = uint(off["at"], 0, now)
            out["recent_offscreen"] = {"at": at, "summary": text(off["summary"], 160),
                                       "when": time_label(at, now, tph, False)["label"]}

        nv = a.get("novelty")
        if nv is None:
            out["novelty_focus"] = None
        else:
            keys(nv, ["id", "score"], [])
            out["novelty_focus"] = {"id": ident(nv["id"]), "score": uint(nv["score"], 0, 100)}

        x = self.beings.get(b)
        out["trends"] = [] if x is None else [self.trend_of(b, k, 3, 30, 40, 500) for k in sorted(x["keys"])]
        out["streak"] = None if x is None else self.streak_view(b)
        return out

    # ---------------------------------------------------------------- rank

    def op_rank(self, a: dict) -> dict:
        keys(a, ["items", "budget"], ["listener", "earshot", "cues", "task_needs", "mood", "weights", "recent",
                                      "floor"])
        budget = uint(a["budget"], 0, 10_000_000)
        listener = opt_ident(a, "listener")
        earshot = set(idset(a, "earshot"))
        cues = a.get("cues")
        cues = {} if cues is None else cues
        keys(cues, [], ["entities", "tags", "places"])
        cue = {k: set(idset(cues, k)) for k in ("entities", "tags", "places")}
        task_needs = set(idset(a, "task_needs"))
        mood = a.get("mood")
        mood = {} if mood is None else mood
        keys(mood, [], ["valence", "resentment", "gratitude"])
        valence = opt_sint(mood, "valence", -100, 100, 0)
        resent = opt_uint(mood, "resentment", 0, 100, 0)
        grat = opt_uint(mood, "gratitude", 0, 100, 0)
        dark, warm = valence < -30 or resent >= 50, valence > 30 or grat >= 50
        w = a.get("weights")
        w = {} if w is None else w
        keys(w, [], [k for k, _ in WEIGHTS])
        weights = [(k, opt_uint(w, k, 0, 100, d)) for k, d in WEIGHTS]
        total_w = sum(v for _, v in weights)
        need(total_w > 0)
        recent = opt_uint(a, "recent", 0, 1000, 3)
        floor = opt_uint(a, "floor", 0, 1000, 1)

        rows = blist(a, "items", self.lim["items"])
        items = []
        for it in rows:
            keys(it, ["id", "bytes"], ["kind", "at", "entities", "tags", "places", "urgency", "mandatory", "group",
                                       "polarity"])
            i = ident(it["id"])
            nbytes = uint(it["bytes"], 1, 65_536)
            at = None if it.get("at") is None else uint(it["at"], 0, self.clock)
            ents, tags, places = (set(idset(it, k)) for k in ("entities", "tags", "places"))
            pol = "neutral" if it.get("polarity") is None else ident(it["polarity"])
            need(pol in POLARITY)
            u = opt_uint(it, "urgency", 0, 100, 0) * 10
            lis = 1000 if listener is not None and listener in ents else 600 if ents & earshot else 0
            hits = len(ents & cue["entities"]) + len(tags & cue["tags"]) + len(places & cue["places"])
            q = min(1000, 250 * hits)
            t = 1000 if i in task_needs or tags & task_needs else 0
            m = 1000 if (pol == "grievance" and dark) or (pol == "favor" and warm) else 0
            if at is None or self.clock - at > recent:
                r = 0
            else:
                r = (recent + 1 - (self.clock - at)) * 1000 // (recent + 1)
            parts = {"listener": lis, "mood": m, "recency": r, "task": t, "urgency": u, "utterance": q}
            score = sum(v * parts[k] for k, v in weights) // total_w
            items.append({"bytes": nbytes, "group": opt_ident(it, "group"), "id": i,
                          "kind": opt_ident(it, "kind"), "mandatory": opt_bool(it, "mandatory", False),
                          "parts": parts, "score": score})
        need(len({x["id"] for x in items}) == len(items))

        units: Dict[str, List[dict]] = {}
        for x in items:
            units.setdefault(x["group"] if x["group"] is not None else "\0" + x["id"], []).append(x)
        packs = []
        for members in units.values():
            packs.append({"bytes": sum(x["bytes"] for x in members), "first": min(x["id"] for x in members),
                          "mandatory": any(x["mandatory"] for x in members),
                          "members": members, "score": max(x["score"] for x in members)})
        used = 0
        kept, dropped = [], []
        for p in [p for p in packs if p["mandatory"]]:
            used += p["bytes"]
            kept += [(x, "mandatory") for x in p["members"]]
        rest = [p for p in packs if not p["mandatory"]]
        rest.sort(key=lambda p: (-(p["score"] * 1000 // p["bytes"]), -p["score"], p["first"]))
        for p in rest:
            if p["score"] < floor:
                dropped += [(x, "irrelevant") for x in p["members"]]
            elif used + p["bytes"] <= budget:
                used += p["bytes"]
                kept += [(x, "salience") for x in p["members"]]
            else:
                dropped += [(x, "budget") for x in p["members"]]

        def view(x: dict, why: str) -> dict:
            return {"bytes": x["bytes"], "group": x["group"], "id": x["id"], "kind": x["kind"], "parts": x["parts"],
                    "reason": why, "score": x["score"]}

        order = lambda xw: (-xw[0]["score"], xw[0]["id"])
        return {"budget": budget, "dropped": [view(x, y) for x, y in sorted(dropped, key=order)],
                "kept": [view(x, y) for x, y in sorted(kept, key=order)], "over_budget": used > budget, "used": used}

    def workflow(self, request: Any) -> dict:
        table = {"tick": self.op_tick, "observe": self.op_observe, "trend": self.op_trend,
                 "outcome": self.op_outcome, "streak": self.op_streak, "time": self.op_time,
                 "frame": self.op_frame, "rank": self.op_rank}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Frame().workflow(request)
