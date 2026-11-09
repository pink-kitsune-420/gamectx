"""Independent original reference for default packs (pass 2, D5).
Python standard library only; nothing here calls a model.

A pack is ready-made authoring data that turns into plain requests for the engines that
already exist, so a new being can start from a sensible default instead of a blank sheet:

- species (human, dog, horse, goat): body needs with thresholds, rates and a satisfying
  action, plus a lifespan in hour ticks;
- temperaments (bold, steady, cautious, timid, dreamy): a cognition temperament, a threat
  temperament and starting personality traits (signed millionths);
- roles (guard, baker, farmer, elder, healer): status, seats and timed duties;
- norm sets (village, guild, wanderer): personal norms an action can break.

`apply` never changes another engine: it returns the requests (`body`, `survival`,
`cognition`, `drift`, `roles`, `creed`) for the host to run, plus a hint for threat
responses. Authors `define` their own packs of the same shapes (bounded by the `custom`
limit; built-in ids cannot be replaced), and `list` / `show` read them back.
"""
import pathlib
import sys
from typing import Any, Dict, List

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_uint, text, uint
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
M = 1_000_000
SPEC = [("custom", 64, 1_000_000), ("roles", 2, 4), ("norms", 2, 4)]
KINDS = ["norms", "role", "species", "temperament"]
COGNITION = ["careful", "dreamy", "impulsive", "steady"]
SURVIVAL = ["bold", "cautious", "steady", "timid"]
MAX_NEEDS, MAX_TRAITS, MAX_DUTIES, MAX_TENETS = 8, 8, 8, 16
ACTIONS = {
    "activity", "add_member", "animal_train", "approach", "ask", "barter", "behave", "build", "calm", "can_carry",
    "check_relation", "cook", "craft", "create", "debt_query", "decline", "destroy", "dismantle", "enter", "feed",
    "fitness_train", "followup", "gesture", "gift", "harvest", "help", "lead", "load", "loan", "look", "move", "pet",
    "prop_interact", "remove_member", "repair", "repay", "ride", "service", "set_customs", "set_relation", "silence",
    "store", "take", "talk", "transfer", "travel", "travel_cost", "unload", "wait",
}


def sint(v: Any, lo: int, hi: int) -> int:
    need(isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi)
    return v


def blist(d: dict, k: str, lo: int, hi: int) -> list:
    v = d.get(k)
    v = [] if v is None else v
    need(isinstance(v, list) and lo <= len(v) <= hi)
    return v


def unique(rows: List[dict], k: str) -> None:
    need(len({r[k] for r in rows}) == len(rows))


def action(v: Any) -> str:
    a = ident(v)
    need(a in ACTIONS)
    return a


def read_species(p: Any) -> dict:
    keys(p, ["lifespan", "needs"], [])
    life = uint(p["lifespan"], 1, MAX_AT)
    needs = []
    for n in blist(p, "needs", 1, MAX_NEEDS):
        keys(n, ["id", "action"], ["label", "threshold", "rate_per_tick", "amount"])
        i = ident(n["id"])
        needs.append({"action": action(n["action"]), "amount": opt_uint(n, "amount", 0, 100, 20), "id": i,
                      "label": i if n.get("label") is None else text(n["label"], 40),
                      "rate_per_tick": opt_uint(n, "rate_per_tick", 0, 100, 1),
                      "threshold": opt_uint(n, "threshold", 0, 100, 70)})
    unique(needs, "id")
    return {"lifespan": life, "needs": needs}


def read_temperament(p: Any) -> dict:
    keys(p, ["traits"], ["cognition", "survival"])
    cog = "steady" if p.get("cognition") is None else ident(p["cognition"])
    sur = "steady" if p.get("survival") is None else ident(p["survival"])
    need(cog in COGNITION and sur in SURVIVAL)
    traits = []
    for t in blist(p, "traits", 0, MAX_TRAITS):
        keys(t, ["trait", "base"], [])
        traits.append({"base": sint(t["base"], -M, M), "trait": ident(t["trait"])})
    unique(traits, "trait")
    return {"cognition": cog, "survival": sur, "traits": traits}


def read_role(p: Any) -> dict:
    keys(p, ["status"], ["seats", "duties"])
    status = uint(p["status"], 0, 100)
    seats = None if p.get("seats") is None else uint(p["seats"], 1, 1_000_000)
    duties = []
    for d in blist(p, "duties", 0, MAX_DUTIES):
        keys(d, ["id", "action", "every"], ["at", "place", "priority"])
        duties.append({"action": action(d["action"]), "at": opt_uint(d, "at", 0, MAX_AT, 0),
                       "every": uint(d["every"], 1, MAX_AT), "id": ident(d["id"]),
                       "place": None if d.get("place") is None else ident(d["place"]),
                       "priority": opt_uint(d, "priority", 0, 100, 50)})
    unique(duties, "id")
    return {"duties": duties, "seats": seats, "status": status}


def read_norms(p: Any) -> dict:
    keys(p, ["tenets"], [])
    tenets = []
    for t in blist(p, "tenets", 1, MAX_TENETS):
        keys(t, ["id", "severity", "label", "action"], [])
        tenets.append({"action": ident(t["action"]), "id": ident(t["id"]), "label": text(t["label"], 80),
                       "severity": uint(t["severity"], 1, 100)})
    unique(tenets, "id")
    return {"tenets": tenets}


READ = {"norms": read_norms, "role": read_role, "species": read_species, "temperament": read_temperament}


def need_row(i: str, action_: str, threshold: int, rate: int, amount: int, label: str) -> dict:
    return {"id": i, "action": action_, "threshold": threshold, "rate_per_tick": rate, "amount": amount, "label": label}


def duty(i: str, action_: str, every: int, at: int, place: str, priority: int) -> dict:
    return {"id": i, "action": action_, "every": every, "at": at, "place": place, "priority": priority}


def traits(bold: int, trust: int, warmth: int) -> list:
    return [{"trait": "boldness", "base": bold}, {"trait": "trust", "base": trust}, {"trait": "warmth", "base": warmth}]


def tenet(i: str, action_: str, severity: int, label: str) -> dict:
    return {"id": i, "action": action_, "severity": severity, "label": label}


BUILTIN_RAW = {
    "species": {
        "human": {"lifespan": 613_200, "needs": [
            need_row("hunger", "cook", 70, 2, 30, "Hunger"), need_row("rest", "wait", 75, 1, 25, "Rest"),
            need_row("company", "talk", 60, 1, 20, "Company")]},
        "dog": {"lifespan": 105_120, "needs": [
            need_row("hunger", "feed", 65, 3, 35, "Hunger"), need_row("rest", "wait", 75, 2, 30, "Rest"),
            need_row("affection", "pet", 55, 2, 25, "Affection")]},
        "horse": {"lifespan": 219_000, "needs": [
            need_row("hunger", "feed", 70, 2, 30, "Hunger"), need_row("rest", "wait", 80, 1, 25, "Rest"),
            need_row("exercise", "ride", 60, 2, 25, "Exercise")]},
        "goat": {"lifespan": 131_400, "needs": [
            need_row("hunger", "feed", 60, 3, 30, "Hunger"), need_row("rest", "wait", 80, 1, 25, "Rest"),
            need_row("roam", "move", 55, 3, 20, "Roaming")]},
    },
    "temperament": {
        "bold": {"cognition": "impulsive", "survival": "bold", "traits": traits(600_000, 500_000, 400_000)},
        "steady": {"cognition": "steady", "survival": "steady", "traits": traits(0, 300_000, 300_000)},
        "cautious": {"cognition": "careful", "survival": "cautious", "traits": traits(-300_000, 0, 200_000)},
        "timid": {"cognition": "careful", "survival": "timid", "traits": traits(-600_000, -100_000, 300_000)},
        "dreamy": {"cognition": "dreamy", "survival": "steady", "traits": traits(-100_000, 400_000, 500_000)},
    },
    "role": {
        "guard": {"status": 40, "duties": [duty("patrol", "move", 6, 0, "gate", 70)]},
        "baker": {"status": 35, "duties": [duty("bake", "cook", 24, 5, "bakery", 60)]},
        "farmer": {"status": 30, "duties": [duty("tend", "harvest", 24, 6, "fields", 55)]},
        "elder": {"status": 70, "seats": 3, "duties": [duty("counsel", "talk", 24, 18, "hall", 40)]},
        "healer": {"status": 55, "duties": [duty("rounds", "help", 12, 8, "infirmary", 75)]},
    },
    "norms": {
        "village": {"tenets": [tenet("village_no_theft", "take", 60, "Do not take what is not yours"),
                               tenet("village_no_wrecking", "destroy", 50, "Do not wreck a neighbour's work")]},
        "guild": {"tenets": [tenet("guild_answer_calls", "decline", 45, "Do not refuse a guild-mate who asks for help"),
                             tenet("guild_no_undoing", "dismantle", 40, "Do not take apart guild work without leave")]},
        "wanderer": {"tenets": [tenet("wanderer_no_roots", "build", 20, "Do not put down roots"),
                                tenet("wanderer_travel_light", "store", 15, "Carry only what you can use")]},
    },
}
BUILTIN = {k: {i: READ[k](p) for i, p in v.items()} for k, v in BUILTIN_RAW.items()}


def req(kind: str, ops: List[dict]) -> dict:
    return {"args": {"ops": ops}, "kind": kind}


def op(name: str, args: dict) -> dict:
    return {"args": args, "op": name}


class Packs:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.custom: Dict[str, Dict[str, dict]] = {k: {} for k in KINDS}
        self.count = 0

    def kind(self, a: dict) -> str:
        k = ident(a["kind"])
        need(k in KINDS)
        return k

    def find(self, k: str, i: str) -> dict:
        p = BUILTIN[k].get(i) or self.custom[k].get(i)
        need(p is not None)
        return p

    # ---------------------------------------------------------------- ops

    def op_define(self, a: dict) -> dict:
        keys(a, ["kind", "id", "pack"], [])
        k, i = self.kind(a), ident(a["id"])
        p = READ[k](a["pack"])
        need(i not in BUILTIN[k])
        need(i in self.custom[k] or self.count < self.lim["custom"])
        if i not in self.custom[k]:
            self.count += 1
        self.custom[k][i] = p
        return {"id": i, "kind": k, "pack": p}

    def op_list(self, a: dict) -> dict:
        keys(a, ["kind"], [])
        k = self.kind(a)
        return {"builtin": sorted(BUILTIN[k]), "custom": sorted(self.custom[k]), "kind": k}

    def op_show(self, a: dict) -> dict:
        keys(a, ["kind", "id"], [])
        k, i = self.kind(a), ident(a["id"])
        return {"builtin": i in BUILTIN[k], "id": i, "kind": k, "pack": self.find(k, i)}

    def op_apply(self, a: dict) -> dict:
        keys(a, ["being"], ["species", "temperament", "roles", "norms"])
        b = ident(a["being"])
        sp = None if a.get("species") is None else self.find("species", ident(a["species"]))
        tp = None if a.get("temperament") is None else self.find("temperament", ident(a["temperament"]))
        role_ids = [ident(r) for r in blist(a, "roles", 0, self.lim["roles"])]
        norm_ids = [ident(n) for n in blist(a, "norms", 0, self.lim["norms"])]
        need(len(set(role_ids)) == len(role_ids) and len(set(norm_ids)) == len(norm_ids))
        roles = [(r, self.find("role", r)) for r in role_ids]
        tenets = [t for n in norm_ids for t in self.find("norms", n)["tenets"]]
        need(len(tenets) <= MAX_TENETS)
        unique(tenets, "id")
        need(sp is not None or tp is not None or roles or tenets)
        out = []
        if sp is not None:
            needs = [{"id": n["id"], "label": n["label"], "rate_per_tick": n["rate_per_tick"],
                      "satisfiers": [{"action": n["action"], "amount": n["amount"]}], "threshold": n["threshold"]}
                     for n in sp["needs"]]
            out.append(req("body", [op("init", {"being": b, "needs": needs, "species": a["species"]})]))
            out.append(req("survival", [op("born", {"being": b, "lifespan": sp["lifespan"]})]))
        if tp is not None:
            out.append(req("cognition", [op("profile", {"being": b, "temperament": tp["cognition"]})]))
            if tp["traits"]:
                out.append(req("drift", [op("trait", {"base": t["base"], "being": b, "trait": t["trait"]})
                                         for t in tp["traits"]]))
        if roles:
            ops = []
            for r, p in roles:
                args = {"duties": [{k: v for k, v in d.items() if v is not None} for d in p["duties"]], "id": r,
                        "status": p["status"]}
                if p["seats"] is not None:
                    args["seats"] = p["seats"]
                ops.append(op("role", args))
            ops += [op("assign", {"being": b, "role": r}) for r, _ in roles]
            out.append(req("roles", ops))
        if tenets:
            out.append(req("creed", [op("set_norms", {"being": b, "norms": tenets})]))
        return {"being": b, "hints": {"lifespan": None if sp is None else sp["lifespan"],
                                      "survival_temperament": None if tp is None else tp["survival"]},
                "requests": out}

    def workflow(self, request: Any) -> dict:
        table = {"define": self.op_define, "list": self.op_list, "show": self.op_show, "apply": self.op_apply}

        def f(op_: str, args: Any) -> dict:
            need(op_ in table and isinstance(args, dict))
            return table[op_](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Packs().workflow(request)
