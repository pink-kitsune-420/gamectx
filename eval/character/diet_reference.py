"""Pure-Python reference for diets and foods (pass 2, stage 3, local kind `diet`).

Written independently of src/character/diet.rs; diet_cases.py checks the two agree on
conformance cases and randomized workflows.
"""
import json
from typing import Any, Dict, List, Optional

from beliefs_speech_reference import Bad, blist, ident, keys, need, opt_bool, opt_uint, run_ops, text, uint

FOOD_TAGS = ["alcohol", "dairy", "egg", "fish", "fruit", "grain", "herb", "meat", "mineral", "nut", "sweet",
             "vegetable", "water"]
DIETS = ["herbivore", "carnivore", "omnivore", "photosynthetic", "scavenger", "mineral"]
MAX_FOODS = 256
MAX_BEINGS = 64
MAX_RECENT = 16
WINDOW = 8
MAX_SHELF = 1_000_000
MAX_AT = 1_000_000_000_000
SICK_AT = 30

VILLAGE = [
    {"id": "rye_bread", "label": "rye bread", "tags": ["grain"], "hunger": 35, "joy": 2, "spoils_after": 96,
     "cooked_bonus": 0},
    {"id": "oat_porridge", "label": "oat porridge", "tags": ["grain", "dairy"], "hunger": 30, "thirst": 5,
     "spoils_after": 24},
    {"id": "orchard_apple", "label": "orchard apple", "tags": ["fruit"], "hunger": 12, "thirst": 8, "joy": 3,
     "spoils_after": 240},
    {"id": "hard_cheese", "label": "hard cheese", "tags": ["dairy"], "hunger": 25, "joy": 4, "spoils_after": 480},
    {"id": "river_trout", "label": "river trout", "tags": ["fish"], "hunger": 30, "spoils_after": 24,
     "preserved_spoils_after": 720, "raw_risk": 35, "cooked_bonus": 20},
    {"id": "hearty_stew", "label": "hearty stew", "tags": ["meat", "vegetable"], "hunger": 50, "thirst": 10,
     "joy": 6, "spoils_after": 36, "raw_risk": 40, "cooked_bonus": 10},
    {"id": "bramble_berries", "label": "bramble berries", "tags": ["fruit"], "hunger": 8, "thirst": 4, "joy": 4,
     "spoils_after": 48},
    {"id": "honey_cake", "label": "honey cake", "tags": ["grain", "sweet", "egg"], "hunger": 20, "joy": 10,
     "spoils_after": 72},
    {"id": "barley_ale", "label": "barley ale", "tags": ["alcohol", "grain"], "hunger": 5, "thirst": 30, "joy": 8},
    {"id": "spring_water", "label": "spring water", "tags": ["water"], "hunger": 0, "thirst": 40},
    {"id": "roasted_nuts", "label": "roasted nuts", "tags": ["nut"], "hunger": 15, "joy": 3, "spoils_after": 720},
    {"id": "salt_lick", "label": "salt lick", "tags": ["mineral"], "hunger": 10, "thirst": 0, "joy": -2},
]


def read_food(v: Any) -> Dict[str, Any]:
    keys(v, ["id", "tags", "hunger"],
         ["label", "thirst", "joy", "spoils_after", "preserved_spoils_after", "raw_risk", "cooked_bonus"])
    fid = ident(v["id"])
    label = fid if v.get("label") is None else text(v["label"], 48)
    tags = blist(v["tags"], 8) if v["tags"] is not None else None
    need(tags is not None and len(tags) > 0)
    seen: List[str] = []
    for t in tags:
        t = ident(t)
        need(t in FOOD_TAGS and t not in seen)
        seen.append(t)
    joy = v.get("joy")
    if joy is None:
        joy = 0
    need(isinstance(joy, int) and not isinstance(joy, bool) and -20 <= joy <= 20)
    spoils = opt_uint(v, "spoils_after", 0, MAX_SHELF, 0)
    return {
        "cooked_bonus": opt_uint(v, "cooked_bonus", 0, 100, 0),
        "hunger": uint(v["hunger"], 0, 100),
        "id": fid,
        "joy": joy,
        "label": label,
        "preserved_spoils_after": opt_uint(v, "preserved_spoils_after", 0, MAX_SHELF, min(spoils * 4, MAX_SHELF)),
        "raw_risk": opt_uint(v, "raw_risk", 0, 100, 0),
        "spoils_after": spoils,
        "tags": sorted(seen),
        "thirst": opt_uint(v, "thirst", 0, 100, 0),
    }


def tag_set(v: Any, n: int) -> List[str]:
    out = set()
    for t in blist(v, n):
        t = ident(t)
        need(t in FOOD_TAGS)
        out.add(t)
    return sorted(out)


def read_serving(v: Any, extra: List[str]) -> Dict[str, Any]:
    keys(v, ["food"], ["cooked", "preserved", "made_at", "at"] + extra)
    at = opt_uint(v, "at", 0, MAX_AT, 0)
    made = opt_uint(v, "made_at", 0, MAX_AT, at)
    need(made <= at)
    return {"food": ident(v["food"]), "cooked": opt_bool(v, "cooked", False),
            "preserved": opt_bool(v, "preserved", False), "made_at": made, "at": at}


def new_profile() -> Dict[str, Any]:
    return {"diet": "omnivore", "forbids": [], "allergies": [], "likes": [], "dislikes": [], "recent": [], "meals": 0}


class DietRef:
    def __init__(self) -> None:
        self.foods: Dict[str, Dict[str, Any]] = {}
        self.beings: Dict[str, Dict[str, Any]] = {}

    def run(self, request: Any) -> Dict[str, Any]:
        def f(op, a):
            if op == "register_food":
                return self.register(a)
            if op == "set_diet":
                return self.set_diet(a)
            if op == "evaluate":
                being = ident(a.get("being")) if isinstance(a, dict) and "being" in a else ident(None)
                s = read_serving(a, ["being"])
                return self.view(being, self.evaluate(being, s))
            if op == "choose":
                return self.choose(a)
            if op == "eat":
                return self.eat(a)
            if op == "assess":
                return self.assess(a)
            raise Bad(f"unknown diet op {op}")
        return run_ops(request, f)

    def register(self, a: Any) -> Dict[str, Any]:
        keys(a, [], ["foods", "pack"])
        fresh = []
        if a.get("pack") is not None:
            need(text(a["pack"], 16) == "village")
            fresh += [read_food(x) for x in json.loads(json.dumps(VILLAGE))]
        given = blist(a.get("foods"), 64)
        need(len(given) > 0 or len(fresh) > 0)
        fresh += [read_food(x) for x in given]
        ids = {f["id"] for f in fresh}
        need(len(self.foods) + len([i for i in ids if i not in self.foods]) <= MAX_FOODS)
        for fd in fresh:
            self.foods[fd["id"]] = fd
        return {"registered": [fd["id"] for fd in fresh], "total": len(self.foods)}

    def set_diet(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being"], ["diet", "forbids", "allergies", "likes", "dislikes"])
        being = ident(a["being"])
        diet = "omnivore" if a.get("diet") is None else text(a["diet"], 16)
        need(diet in DIETS)
        allergies = sorted({ident(x) for x in blist(a.get("allergies"), 8)})
        likes = tag_set(a.get("likes"), len(FOOD_TAGS))
        dislikes = tag_set(a.get("dislikes"), len(FOOD_TAGS))
        need(not set(likes) & set(dislikes))
        forbids = tag_set(a.get("forbids"), len(FOOD_TAGS))
        need(being in self.beings or len(self.beings) < MAX_BEINGS)
        p = self.beings.get(being, new_profile())
        p.update({"diet": diet, "forbids": forbids, "allergies": allergies, "likes": likes, "dislikes": dislikes})
        self.beings[being] = p
        return self.assess({"being": being})

    def evaluate(self, being: str, s: Dict[str, Any]) -> Dict[str, Any]:
        need(s["food"] in self.foods)
        f = self.foods[s["food"]]
        p = self.beings.get(being, new_profile())
        shelf = f["preserved_spoils_after"] if s["preserved"] else f["spoils_after"]
        age = s["at"] - s["made_at"]
        if shelf == 0 or age * 2 < shelf:
            fresh = "fresh"
        elif age < shelf:
            fresh = "stale"
        else:
            fresh = "spoiled"
        tags = set(f["tags"])
        diet = p["diet"]
        if diet == "herbivore":
            species_ok = not tags & {"meat", "fish", "mineral"}
        elif diet == "carnivore":
            species_ok = bool(tags & {"meat", "fish"})
        elif diet == "photosynthetic":
            species_ok = False
        elif diet == "mineral":
            species_ok = "mineral" in tags
        else:
            species_ok = "mineral" not in tags
        allergic = f["id"] in p["allergies"] or bool(tags & set(p["allergies"]))
        raw_unsafe = not s["cooked"] and f["raw_risk"] > 0
        refusals = []
        if not species_ok:
            refusals.append("species_diet")
        if tags & set(p["forbids"]):
            refusals.append("forbidden")
        if allergic:
            refusals.append("allergy")
        if fresh == "spoiled":
            refusals.append("spoiled")
        if raw_unsafe:
            refusals.append("unsafe_raw")
        hunger = f["hunger"]
        if s["cooked"]:
            hunger = min(100, hunger + hunger * f["cooked_bonus"] // 100)
        thirst = f["thirst"]
        if fresh == "stale":
            hunger, thirst = hunger * 3 // 4, thirst * 3 // 4
        elif fresh == "spoiled":
            hunger, thirst = hunger // 2, thirst // 2
        sickness = max([0] + ([40] if allergic else []) + ([30] if fresh == "spoiled" else [])
                       + ([f["raw_risk"]] if raw_unsafe else []))
        repeats = sum(1 for food, _ in p["recent"][-WINDOW:] if food == f["id"])
        joy = (f["joy"] + 5 * len(tags & set(p["likes"])) - 5 * len(tags & set(p["dislikes"]))
               - min(15, 3 * repeats) - {"stale": 3, "spoiled": 10}.get(fresh, 0))
        joy = max(-50, min(50, joy))
        return {"food": f["id"], "freshness": fresh, "refusals": refusals, "hunger": hunger, "thirst": thirst,
                "joy": joy, "sickness": sickness, "repeats": repeats}

    @staticmethod
    def view(being: str, e: Dict[str, Any]) -> Dict[str, Any]:
        return {"being": being, "edible": not e["refusals"], "food": e["food"], "freshness": e["freshness"],
                "hunger": e["hunger"], "joy": e["joy"], "refusals": e["refusals"], "repeats": e["repeats"],
                "sickness": e["sickness"], "thirst": e["thirst"]}

    def choose(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being", "options"], ["hunger_level"])
        being = ident(a["being"])
        level = opt_uint(a, "hunger_level", 0, 100, 50)
        opts = a["options"]
        need(isinstance(opts, list) and 0 < len(opts) <= 16)
        ranked, refused = [], []
        for i, o in enumerate(opts):
            e = self.evaluate(being, read_serving(o, []))
            if e["refusals"]:
                refused.append({"food": e["food"], "index": i, "refusals": e["refusals"]})
            else:
                ranked.append((e["hunger"] * level // 100 + 2 * e["joy"], i, e))
        ranked.sort(key=lambda r: (-r[0], r[2]["food"], r[1]))
        return {
            "being": being,
            "best": ranked[0][1] if ranked else None,
            "ranked": [{"food": e["food"], "freshness": e["freshness"], "hunger": e["hunger"], "index": i,
                        "joy": e["joy"], "score": sc} for sc, i, e in ranked],
            "refused": refused,
        }

    def eat(self, a: Any) -> Dict[str, Any]:
        need(isinstance(a, dict) and "being" in a)
        being = ident(a["being"])
        s = read_serving(a, ["being", "force"])
        force = opt_bool(a, "force", False)
        e = self.evaluate(being, s)
        eaten = not e["refusals"] or force
        effects = []
        if eaten:
            need(being in self.beings or len(self.beings) < MAX_BEINGS)
            p = self.beings.setdefault(being, new_profile())
            if len(p["recent"]) >= MAX_RECENT:
                p["recent"].pop(0)
            p["recent"].append((e["food"], s["at"]))
            p["meals"] += 1
            for need_id, amount in (("hunger", e["hunger"]), ("thirst", e["thirst"])):
                if amount > 0:
                    effects.append({"args": {"amount": amount, "being": being, "need": need_id},
                                    "engine": "body", "op": "satisfy"})
            if e["sickness"] >= SICK_AT:
                effects.append({"args": {"being": being, "condition": "sick"}, "engine": "body",
                                "op": "set_condition"})
            if e["joy"] != 0:
                effects.append({"args": {"being": being, "reason": f"ate {self.foods[e['food']]['label']}",
                                         "valence_delta": e["joy"]}, "engine": "affect", "op": "set_mood"})
        out = self.view(being, e)
        out.update({"eaten": eaten, "effects": effects, "forced": eaten and bool(e["refusals"])})
        return out

    def assess(self, a: Any) -> Dict[str, Any]:
        keys(a, ["being"], [])
        being = ident(a["being"])
        need(being in self.beings)
        p = self.beings[being]
        return {"allergies": p["allergies"], "being": being, "diet": p["diet"], "dislikes": p["dislikes"],
                "forbids": p["forbids"], "likes": p["likes"], "meals": p["meals"],
                "recent": [{"at": at, "food": food} for food, at in p["recent"]]}


def process_local(kind: str, args: Any) -> Dict[str, Any]:
    if kind == "diet":
        return DietRef().run(args)
    raise Bad(f"unknown local kind {kind}")
