"""Independent original reference for defense mechanisms and strain (pass 4, stage 3). Python
standard library only; nothing here calls a model.

A being with a defense profile has a maturity (0-100), a tolerance (0-100), protected secrets, a
preferred style (mechanisms tried first), a strain threshold (millionths), a crisis outcome and a
crisis cooldown. The host reports a threat to the self (`threat`: a self-value of competence,
goodness or belonging lowered by `drop` points, or a protected secret exposed); a threat larger
than the tolerance, or any exposed protected secret, is ego-threatening. The being then picks one
mechanism by a fixed first-match rule: its effective maturity (maturity minus 60 points at full
strain) gives a band (high >= 70, mid >= 40, low below); its style entries in that band are tried
first, then the band's defaults; a mechanism that needs an input (projection a `blame`,
displacement a `safer_target`, rationalization an `excuse`, sublimation a `project`) is skipped
without it. The mechanism keeps part of the threat as strain, tags the distortion it makes, and
names the speech or behaviour effect. Immature distortions (denial, projection, displacement) are
cheap now and costly later: a later contradiction of the same fact adds half again as much strain,
and strain decays at half speed while one is held. Strain at or over the threshold breaks the
being once (its crisis outcome) and drops to half the threshold; within the cooldown no second
crisis happens. A fact distorted three or more times is a defense loop.

Strain decays lazily by the clock: 4,000 millionths per tick, 2,000 while an immature distortion
is held. Rest, support and coping relieve strain; `accept` drops a distortion.
"""
import pathlib
import sys
from typing import Any, Dict, List, Optional

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_uint, text, uint
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
FULL = 1_000_000
SPEC = [("beings", 1024, 1_000_000), ("distortions", 16, 256)]
VALUES = ["competence", "goodness", "belonging", "secret"]
CRISES = ["confession", "flight", "collapse", "outburst"]
LOW = ["denial", "projection", "displacement"]
MID = ["rationalization", "intellectualization", "withdrawal"]
HIGH = ["humour", "sublimation"]
MECHANISMS = LOW + MID + HIGH
KEEP = {"denial": 30, "projection": 40, "displacement": 40, "rationalization": 60, "withdrawal": 70,
        "intellectualization": 60, "humour": 40, "sublimation": 50}
EFFECT = {"denial": "refuses_topic", "projection": "accuses", "displacement": "snaps",
          "rationalization": "explains_too_much", "withdrawal": "goes_quiet", "intellectualization": "talks_procedure",
          "humour": "jokes", "sublimation": "works_harder"}
TAG = {"denial": "denied", "projection": "projected", "displacement": "displaced", "rationalization": "self_serving"}
NEEDS = {"projection": "blame", "displacement": "safer_target", "rationalization": "excuse", "sublimation": "project"}
RELIEF = {"rest": 2_000, "support": 4_000, "coping": 3_000}
LOOP = 3


def band(eff: int) -> str:
    return "high" if eff >= 70 else "mid" if eff >= 40 else "low"


def in_band(m: str, b: str) -> bool:
    return m in {"low": LOW, "mid": MID, "high": HIGH}[b]


class Defenses:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.beings: Dict[str, dict] = {}
        self.clock = 0

    def get(self, b: str) -> dict:
        need(b in self.beings)
        return self.beings[b]

    def strain_now(self, st: dict) -> int:
        immature = any(d["mechanism"] in LOW for d in st["distortions"].values())
        rate = 2_000 if immature else 4_000
        return max(0, st["strain"] - rate * (self.clock - st["at"]))

    def effective(self, st: dict, strain: int) -> int:
        return max(0, st["maturity"] - strain * 60 // FULL)

    def crisis(self, st: dict, strain: int) -> (int, Optional[dict]):
        """Strain after a possible break, and the crisis if one happened."""
        if strain < st["threshold"]:
            return strain, None
        if st["crisis_at"] is not None and self.clock < st["crisis_at"] + st["cooldown"]:
            return strain, None
        return st["threshold"] // 2, {"at": self.clock, "outcome": st["crisis"]}

    def op_tick(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        self.clock = now
        return {"now": now}

    def op_profile(self, a: dict) -> dict:
        keys(a, ["being", "maturity", "tolerance", "crisis"], ["threshold", "cooldown", "style", "protects"])
        b = ident(a["being"])
        maturity = uint(a["maturity"], 0, 100)
        tolerance = uint(a["tolerance"], 0, 100)
        need(isinstance(a["crisis"], str) and a["crisis"] in CRISES)
        threshold = opt_uint(a, "threshold", 1, FULL, 800_000)
        cooldown = opt_uint(a, "cooldown", 1, MAX_AT, 72)
        style = [] if a.get("style") is None else a["style"]
        need(isinstance(style, list) and len(style) <= len(MECHANISMS))
        style = [ident(x) for x in style]
        need(len(set(style)) == len(style) and all(x in MECHANISMS for x in style))
        protects = [] if a.get("protects") is None else a["protects"]
        need(isinstance(protects, list) and len(protects) <= 16)
        protects = [ident(x) for x in protects]
        need(len(set(protects)) == len(protects))
        old = self.beings.get(b)
        need(old is not None or len(self.beings) < self.lim["beings"])
        if old is None:
            st = {"strain": 0, "at": self.clock, "distortions": {}, "crisis_at": None}
        else:
            st = old
            st["strain"] = self.strain_now(old)
            st["at"] = self.clock
        st.update({"maturity": maturity, "tolerance": tolerance, "crisis": a["crisis"], "threshold": threshold,
                   "cooldown": cooldown, "style": style, "protects": sorted(protects)})
        self.beings[b] = st
        return self.view(b, st)

    def view(self, b: str, st: dict) -> dict:
        strain = self.strain_now(st)
        eff = self.effective(st, strain)
        ds = [{"fact": f, "hits": d["hits"], "mechanism": d["mechanism"]} for f, d in sorted(st["distortions"].items())]
        return {"band": band(eff), "being": b, "crisis_at": st["crisis_at"], "crisis_outcome": st["crisis"],
                "distortions": ds, "effective_maturity": eff,
                "loops": [d["fact"] for d in ds if d["hits"] >= LOOP], "maturity": st["maturity"],
                "strain": strain, "threshold": st["threshold"]}

    def op_status(self, a: dict) -> dict:
        keys(a, ["being"], [])
        b = ident(a["being"])
        return self.view(b, self.get(b))

    def op_threat(self, a: dict) -> dict:
        keys(a, ["being", "value", "drop", "fact"], ["blame", "safer_target", "excuse", "project"])
        b = ident(a["being"])
        need(isinstance(a["value"], str) and a["value"] in VALUES)
        value = a["value"]
        drop = uint(a["drop"], 0, 100)
        fact = ident(a["fact"])
        given = {}
        for k in ("blame", "safer_target", "project"):
            if a.get(k) is not None:
                given[k] = ident(a[k])
                need(given[k] != b)
        if a.get("excuse") is not None:
            given["excuse"] = text(a["excuse"], 120)
        st = self.get(b)
        strain = self.strain_now(st)
        threatened = fact in st["protects"] if value == "secret" else drop > st["tolerance"]
        eff = self.effective(st, strain)
        out = {"band": band(eff), "being": b, "crisis": None, "distortion": None, "effect": None,
               "effective_maturity": eff, "explain": None, "fact": fact, "hits": 0, "kept": 0, "loop": False,
               "mechanism": None, "strain": strain, "threatened": threatened, "value": value}
        if not threatened:
            return out
        bd = band(eff)
        order = [m for m in st["style"] if in_band(m, bd)] + {"low": LOW, "mid": MID, "high": HIGH}[bd]
        mech = next(m for m in order if m not in NEEDS or NEEDS[m] in given)
        records = mech in TAG
        ds = st["distortions"]
        need(not records or fact in ds or len(ds) < self.lim["distortions"])
        kept = drop * 10_000 * KEEP[mech] // 100
        strain = min(FULL, strain + kept)
        hits = 0
        dist = None
        if records:
            hits = ds[fact]["hits"] + 1 if fact in ds else 1
            dist = {"fact": fact, "mechanism": mech, "tag": TAG[mech]}
            if mech == "denial":
                dist["confidence"] = 30
            elif mech == "projection":
                dist["blame"] = given["blame"]
            elif mech == "displacement":
                dist["target"] = given["safer_target"]
            else:
                dist["excuse"] = given["excuse"]
            ds[fact] = {"mechanism": mech, "hits": hits}
        strain, crisis = self.crisis(st, strain)
        if crisis is not None:
            st["crisis_at"] = self.clock
        st["strain"], st["at"] = strain, self.clock
        verb = {"denial": "denied it", "projection": f"blamed {given.get('blame')}",
                "displacement": f"snapped at {given.get('safer_target')}", "rationalization": "made an excuse",
                "withdrawal": "withdrew", "intellectualization": "talked procedure", "humour": "joked",
                "sublimation": f"threw it into {given.get('project')}"}[mech]
        out.update({"crisis": crisis, "distortion": dist, "effect": EFFECT[mech],
                    "explain": f"{b} {verb} because {fact} threatened {value}", "hits": hits, "kept": kept,
                    "loop": hits >= LOOP, "mechanism": mech, "strain": strain})
        if mech == "sublimation":
            out["project"] = given["project"]
            out["progress"] = drop
        return out

    def op_contradict(self, a: dict) -> dict:
        keys(a, ["being", "fact", "drop"], [])
        b = ident(a["being"])
        fact = ident(a["fact"])
        drop = uint(a["drop"], 0, 100)
        st = self.get(b)
        strain = self.strain_now(st)
        d = st["distortions"].get(fact)
        if d is None or d["mechanism"] not in LOW:
            return {"being": b, "crisis": None, "distorted": False, "fact": fact, "hits": 0, "kept": 0,
                    "loop": False, "strain": strain}
        kept = drop * 15_000
        strain = min(FULL, strain + kept)
        d["hits"] += 1
        strain, crisis = self.crisis(st, strain)
        if crisis is not None:
            st["crisis_at"] = self.clock
        st["strain"], st["at"] = strain, self.clock
        return {"being": b, "crisis": crisis, "distorted": True, "fact": fact, "hits": d["hits"], "kept": kept,
                "loop": d["hits"] >= LOOP, "strain": strain}

    def op_relieve(self, a: dict) -> dict:
        keys(a, ["being", "kind", "amount"], [])
        b = ident(a["being"])
        need(isinstance(a["kind"], str) and a["kind"] in RELIEF)
        amount = uint(a["amount"], 0, 100)
        st = self.get(b)
        before = self.strain_now(st)
        strain = max(0, before - amount * RELIEF[a["kind"]])
        st["strain"], st["at"] = strain, self.clock
        return {"being": b, "kind": a["kind"], "relieved": before - strain, "strain": strain}

    def op_accept(self, a: dict) -> dict:
        keys(a, ["being", "fact"], [])
        b = ident(a["being"])
        fact = ident(a["fact"])
        st = self.get(b)
        need(fact in st["distortions"])
        st["strain"], st["at"] = self.strain_now(st), self.clock
        d = st["distortions"].pop(fact)
        return {"being": b, "fact": fact, "mechanism": d["mechanism"], "strain": st["strain"]}

    def workflow(self, request: Any) -> dict:
        table = {"tick": self.op_tick, "profile": self.op_profile, "status": self.op_status,
                 "threat": self.op_threat, "contradict": self.op_contradict, "relieve": self.op_relieve,
                 "accept": self.op_accept}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Defenses().workflow(request)
