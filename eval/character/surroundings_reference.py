"""Independent original reference for environment cycles, place rules, aftermath traces and item
provenance (pass 3, stage 5, world schemas). Python standard library only; nothing here calls a
model.

Environment: `environment` declares a day of `day_length` ticks split into contiguous phases
(light `dark`/`dim`/`bright`, a temperature delta) and weather kinds (exterior light shift, noise,
travel cost, need rates, sought place tags); `weather` sets the current weather; `conditions`
reports the phase, light, temperature, weather effects and, for a place, its effective access,
crowding, earshot and whether secrets can be discussed there.

Places: `place` declares an owner, access (`public`, `customers_only`, `members_only`, `private`,
`restricted`, optionally different outside opening hours), privacy (`public`, `semi_private`,
`private`, `echoing`), a comfortable capacity, an exterior flag and named stations with a user
limit. `enter` admits or refuses a being by the access rule (a refusal is a trespass and changes
nothing), `leave` frees its stations, `reserve` and `release` share a station first come, first
served with a queue.

Traces: `trace_kind` declares what actions leave a trace, what clears it, whether it is seen at a
glance or only on inspection, how long it lasts and what fact it implies. `act` leaves and clears
traces at a place, `perceive` lists what an observer can see there (an inferred belief, never who
did it), and `tick` lets traces fade.

Items: `item` records condition, legal owner, possessor, maker, who holds it dear and how long it
keeps; `transfer` moves it by gift, sale, loan, return, taking, finding or dropping, keeping the
legal owner apart from whoever holds it; `damage` lowers its condition; `inspect_item` tells an
observer whether they are looking at their own stolen property and how much a loss would weigh.
"""
import pathlib
import sys
from typing import Any, Dict, List, Optional, Tuple

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_bool, opt_uint, text, uint
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
SPEC = [("places", 64, 100_000), ("occupants", 32, 10_000), ("trace_kinds", 64, 4096), ("traces", 256, 1_000_000),
        ("items", 256, 1_000_000)]
LIGHT = ["dark", "dim", "bright"]
ACCESS = ["public", "customers_only", "members_only", "private", "restricted"]
PRIVACY = ["public", "semi_private", "private", "echoing"]
EARSHOT = {"public": "bystanders", "semi_private": "nearby", "private": "none", "echoing": "everyone"}
HOW = ["gift", "sale", "loan", "return", "take", "find", "drop"]
HELD_AS = ["borrowed", "stolen", "found"]


def sint(v: Any, lo: int, hi: int) -> int:
    need(isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi)
    return v


def opt_ident(d: dict, k: str) -> Optional[str]:
    return None if d.get(k) is None else ident(d[k])


def one_of(d: dict, k: str, options: List[str], default: str) -> str:
    v = opt_ident(d, k)
    if v is None:
        return default
    need(v in options)
    return v


def id_list(v: Any, lo: int, hi: int) -> List[str]:
    v = [] if v is None else v
    need(isinstance(v, list) and lo <= len(v) <= hi)
    out = [ident(x) for x in v]
    need(len(set(out)) == len(out))
    return out


def label(cond: int) -> str:
    if cond == 0:
        return "broken"
    if cond < 30:
        return "damaged"
    if cond < 70:
        return "worn"
    if cond < 95:
        return "intact"
    return "masterwork"


class Surroundings:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.clock = 0
        self.env: Optional[dict] = None
        self.weather: Optional[str] = None
        self.places: Dict[str, dict] = {}
        self.kinds: Dict[str, dict] = {}
        self.traces: Dict[Tuple[str, str], int] = {}
        self.items: Dict[str, dict] = {}

    # ---------------------------------------------------------------- time and environment

    def op_tick(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        self.clock = now
        gone = sorted(k for k, at in self.traces.items() if now - at >= self.kinds[k[1]]["ttl"])
        for k in gone:
            del self.traces[k]
        return {"expired": [{"kind": k, "place": p} for p, k in gone], "now": now}

    def op_environment(self, a: dict) -> dict:
        keys(a, ["day_length", "phases"], ["weather"])
        day = uint(a["day_length"], 1, 1_000_000)
        rows = a["phases"]
        need(isinstance(rows, list) and 1 <= len(rows) <= 24)
        phases, edge = [], 0
        for x in rows:
            keys(x, ["id", "start", "end", "light"], ["temp_delta"])
            p = {"id": ident(x["id"]), "start": uint(x["start"], 0, day), "end": uint(x["end"], 1, day),
                 "light": x["light"], "temp_delta": 0 if x.get("temp_delta") is None else sint(x["temp_delta"], -100, 100)}
            need(isinstance(p["light"], str) and p["light"] in LIGHT)
            need(p["start"] == edge and p["end"] > p["start"] and all(q["id"] != p["id"] for q in phases))
            edge = p["end"]
            phases.append(p)
        need(edge == day)
        rows = a.get("weather")
        rows = [] if rows is None else rows
        need(isinstance(rows, list) and len(rows) <= 16)
        weather: Dict[str, dict] = {}
        for x in rows:
            keys(x, ["id"], ["light_delta", "noise", "travel_pct", "need_rates", "seek_tags"])
            w = ident(x["id"])
            need(w not in weather)
            rates = x.get("need_rates")
            rates = {} if rates is None else rates
            need(isinstance(rates, dict) and len(rates) <= 8)
            weather[w] = {
                "light_delta": 0 if x.get("light_delta") is None else sint(x["light_delta"], -2, 2),
                "noise": opt_uint(x, "noise", 0, 100, 0), "travel_pct": opt_uint(x, "travel_pct", 50, 500, 100),
                "need_rates": {ident(k): uint(v, 0, 100) for k, v in rates.items()},
                "seek_tags": id_list(x.get("seek_tags"), 0, 8)}
        self.env = {"day": day, "phases": phases, "weather": weather}
        if self.weather not in weather:
            self.weather = None
        return {"day_length": day, "phases": [p["id"] for p in phases], "weather": sorted(weather)}

    def op_weather(self, a: dict) -> dict:
        keys(a, [], ["kind"])
        need(self.env is not None)
        w = opt_ident(a, "kind")
        need(w is None or w in self.env["weather"])
        self.weather = w
        return {"weather": w}

    def day_tick(self) -> int:
        return self.clock % self.env["day"] if self.env is not None else self.clock

    def access_now(self, p: dict) -> str:
        h = p["hours"]
        if h is None:
            return p["access"]
        t = self.day_tick()
        inside = h["start"] <= t < h["end"] if h["start"] < h["end"] else (t >= h["start"] or t < h["end"])
        return p["access"] if inside else h["outside"]

    def place_view(self, pid: str) -> dict:
        p = self.places[pid]
        n = len(p["occupants"])
        priv = p["privacy"]
        return {"access": self.access_now(p), "crowded": p["capacity"] is not None and n > p["capacity"],
                "discuss_secrets": priv == "private" or (priv == "semi_private" and n <= 2),
                "earshot": EARSHOT[priv], "id": pid, "occupants": n}

    def op_conditions(self, a: dict) -> dict:
        keys(a, [], ["place"])
        need(self.env is not None)
        pid = opt_ident(a, "place")
        need(pid is None or pid in self.places)
        t = self.day_tick()
        ph = next(p for p in self.env["phases"] if p["start"] <= t < p["end"])
        exterior = pid is None or self.places[pid]["exterior"]
        w = self.env["weather"].get(self.weather) if self.weather is not None else None
        light = LIGHT.index(ph["light"])
        out = {"day_tick": t, "phase": ph["id"], "temp_delta": ph["temp_delta"], "weather": self.weather,
               "exterior": exterior, "place": None if pid is None else self.place_view(pid)}
        if w is not None and exterior:
            light = max(0, min(2, light + w["light_delta"]))
            out.update(noise=w["noise"], need_rates=dict(sorted(w["need_rates"].items())), seek_tags=w["seek_tags"])
        else:
            out.update(noise=0, need_rates={}, seek_tags=[])
        out["light"] = LIGHT[light]
        out["travel_pct"] = w["travel_pct"] if w is not None else 100
        return out

    # ---------------------------------------------------------------- places

    def op_place(self, a: dict) -> dict:
        keys(a, ["id"], ["owner", "access", "hours", "privacy", "capacity", "exterior", "stations"])
        pid = ident(a["id"])
        owner = opt_ident(a, "owner")
        access = one_of(a, "access", ACCESS, "public")
        hours = a.get("hours")
        if hours is not None:
            keys(hours, ["start", "end", "outside"], [])
            need(self.env is not None)
            hours = {"start": uint(hours["start"], 0, 1_000_000), "end": uint(hours["end"], 0, 1_000_000),
                     "outside": hours["outside"]}
            need(hours["start"] != hours["end"] and isinstance(hours["outside"], str) and hours["outside"] in ACCESS)
        privacy = one_of(a, "privacy", PRIVACY, "public")
        cap = None if a.get("capacity") is None else uint(a["capacity"], 1, 10_000)
        exterior = opt_bool(a, "exterior", False)
        rows = a.get("stations")
        rows = [] if rows is None else rows
        need(isinstance(rows, list) and len(rows) <= 16)
        stations: Dict[str, dict] = {}
        for x in rows:
            keys(x, ["id"], ["max_users"])
            s = ident(x["id"])
            need(s not in stations)
            stations[s] = {"max": opt_uint(x, "max_users", 1, 64, 1), "users": [], "queue": []}
        old = self.places.get(pid)
        need(old is None or not old["occupants"])
        need(old is not None or len(self.places) < self.lim["places"])
        self.places[pid] = {"owner": owner, "access": access, "hours": hours, "privacy": privacy, "capacity": cap,
                            "exterior": exterior, "stations": stations, "occupants": []}
        return {"access": access, "id": pid, "privacy": privacy, "stations": sorted(stations)}

    def op_enter(self, a: dict) -> dict:
        keys(a, ["place", "being"], ["customer", "member", "invited"])
        pid, b = ident(a["place"]), ident(a["being"])
        customer, member = opt_bool(a, "customer", False), opt_bool(a, "member", False)
        invited = opt_bool(a, "invited", False)
        need(pid in self.places)
        p = self.places[pid]
        access = self.access_now(p)
        own = p["owner"] == b
        allowed = {"public": True, "customers_only": customer or member or own, "members_only": member or own,
                   "private": own or invited, "restricted": own}[access]
        if allowed and b not in p["occupants"]:
            need(len(p["occupants"]) < self.lim["occupants"])
            p["occupants"].append(b)
        v = self.place_view(pid)
        return {"access": access, "allowed": allowed, "being": b, "crowded": v["crowded"], "occupants": v["occupants"],
                "place": pid, "trespass": not allowed}

    def free(self, s: dict, b: str) -> Optional[str]:
        if b in s["queue"]:
            s["queue"].remove(b)
            return None
        s["users"].remove(b)
        if s["queue"]:
            nxt = s["queue"].pop(0)
            s["users"].append(nxt)
            return nxt
        return None

    def op_leave(self, a: dict) -> dict:
        keys(a, ["place", "being"], [])
        pid, b = ident(a["place"]), ident(a["being"])
        need(pid in self.places and b in self.places[pid]["occupants"])
        p = self.places[pid]
        p["occupants"].remove(b)
        promoted = []
        for sid in sorted(p["stations"]):
            s = p["stations"][sid]
            if b in s["users"] or b in s["queue"]:
                nxt = self.free(s, b)
                if nxt is not None:
                    promoted.append({"being": nxt, "station": sid})
        return {"being": b, "occupants": len(p["occupants"]), "place": pid, "promoted": promoted}

    def op_reserve(self, a: dict) -> dict:
        keys(a, ["place", "station", "being"], [])
        pid, sid, b = ident(a["place"]), ident(a["station"]), ident(a["being"])
        need(pid in self.places and b in self.places[pid]["occupants"] and sid in self.places[pid]["stations"])
        s = self.places[pid]["stations"][sid]
        need(b not in s["users"] and b not in s["queue"])
        if len(s["users"]) < s["max"]:
            s["users"].append(b)
            return {"being": b, "granted": True, "queue_position": 0, "station": sid}
        s["queue"].append(b)
        return {"being": b, "granted": False, "queue_position": len(s["queue"]), "station": sid}

    def op_release(self, a: dict) -> dict:
        keys(a, ["place", "station", "being"], [])
        pid, sid, b = ident(a["place"]), ident(a["station"]), ident(a["being"])
        need(pid in self.places and sid in self.places[pid]["stations"])
        s = self.places[pid]["stations"][sid]
        need(b in s["users"] or b in s["queue"])
        return {"being": b, "promoted": self.free(s, b), "station": sid}

    # ---------------------------------------------------------------- traces

    def op_trace_kind(self, a: dict) -> dict:
        keys(a, ["id", "created_by", "visibility", "ttl", "implies"], ["cleared_by", "sight"])
        k = ident(a["id"])
        kind = {"created_by": id_list(a["created_by"], 1, 8), "cleared_by": id_list(a.get("cleared_by"), 0, 8),
                "visibility": a["visibility"], "ttl": uint(a["ttl"], 1, MAX_AT), "implies": ident(a["implies"]),
                "sight": None if a.get("sight") is None else text(a["sight"], 160)}
        need(isinstance(kind["visibility"], str) and kind["visibility"] in ("public", "inspect"))
        need(k in self.kinds or len(self.kinds) < self.lim["trace_kinds"])
        self.kinds[k] = kind
        return {"id": k}

    def op_act(self, a: dict) -> dict:
        keys(a, ["action", "place"], [])
        act, pid = ident(a["action"]), ident(a["place"])
        need(pid in self.places)
        cleared = [k for k in sorted(self.kinds) if act in self.kinds[k]["cleared_by"] and (pid, k) in self.traces]
        created = [k for k in sorted(self.kinds) if act in self.kinds[k]["created_by"]]
        after = len(self.traces) - len(cleared)
        growth = sum(1 for k in created if (pid, k) not in self.traces or k in cleared)
        need(growth == 0 or after + growth <= self.lim["traces"])
        for k in cleared:
            del self.traces[(pid, k)]
        for k in created:
            self.traces[(pid, k)] = self.clock
        return {"cleared": cleared, "created": created, "place": pid}

    def op_perceive(self, a: dict) -> dict:
        keys(a, ["place"], ["inspect"])
        pid = ident(a["place"])
        need(pid in self.places)
        look = opt_bool(a, "inspect", False)
        seen, hidden = [], 0
        for (p, k) in sorted(self.traces):
            age = self.clock - self.traces[(p, k)]
            kind = self.kinds[k]
            if p != pid or age >= kind["ttl"]:
                continue
            if kind["visibility"] == "public" or look:
                seen.append({"age": age, "belief": "inferred", "implies": kind["implies"], "kind": k,
                             "sight": kind["sight"]})
            else:
                hidden += 1
        return {"hidden": hidden, "place": pid, "traces": seen}

    # ---------------------------------------------------------------- items

    def item_view(self, iid: str) -> dict:
        it = self.items[iid]
        if it["owner"] is None:
            status = "unclaimed"
        elif it["possessor"] == it["owner"]:
            status = "owned"
        elif it["possessor"] is None:
            status = "lost"
        else:
            status = it["held_as"]
        return {"condition": it["condition"], "condition_label": label(it["condition"]), "id": iid,
                "maker": it["maker"], "owner": it["owner"], "possessor": it["possessor"],
                "sentimental_to": it["sentimental_to"], "spoiled": it["perishable"] is not None
                and self.clock - it["created"] >= it["perishable"], "status": status}

    def op_item(self, a: dict) -> dict:
        keys(a, ["id"], ["owner", "possessor", "held_as", "condition", "maker", "sentimental_to", "perishable"])
        iid = ident(a["id"])
        owner = opt_ident(a, "owner")
        poss = opt_ident(a, "possessor") if "possessor" in a else owner
        held = one_of(a, "held_as", HELD_AS, "borrowed")
        it = {"owner": owner, "possessor": poss, "held_as": held, "condition": opt_uint(a, "condition", 0, 100, 100),
              "maker": opt_ident(a, "maker"), "sentimental_to": sorted(id_list(a.get("sentimental_to"), 0, 8)),
              "perishable": None if a.get("perishable") is None else uint(a["perishable"], 1, MAX_AT),
              "created": self.clock}
        need(iid in self.items or len(self.items) < self.lim["items"])
        self.items[iid] = it
        return self.item_view(iid)

    def op_transfer(self, a: dict) -> dict:
        keys(a, ["item", "how"], ["to"])
        iid = ident(a["item"])
        need(iid in self.items)
        how = a["how"]
        need(isinstance(how, str) and how in HOW)
        to = opt_ident(a, "to")
        it = self.items[iid]
        own, poss = it["owner"], it["possessor"]
        if how == "drop":
            need(to is None and poss is not None)
            it["possessor"] = None
            return self.item_view(iid)
        need(to is not None and to != poss)
        if how in ("gift", "sale"):
            need(poss is not None)
            if own is None or poss == own:
                it["owner"] = to
            it["possessor"] = to
        elif how == "loan":
            need(own is not None and poss == own)
            it["possessor"], it["held_as"] = to, "borrowed"
        elif how == "return":
            need(own is not None and to == own)
            it["possessor"] = to
        elif how == "take":
            need(poss is not None)
            it["possessor"] = to
            if own is not None and to != own:
                it["held_as"] = "stolen"
        else:
            need(poss is None)
            it["possessor"] = to
            if own is not None and to != own:
                it["held_as"] = "found"
        return self.item_view(iid)

    def op_damage(self, a: dict) -> dict:
        keys(a, ["item", "amount"], [])
        iid = ident(a["item"])
        need(iid in self.items)
        amt = uint(a["amount"], 1, 100)
        it = self.items[iid]
        it["condition"] = max(0, it["condition"] - amt)
        return self.item_view(iid)

    def op_inspect_item(self, a: dict) -> dict:
        keys(a, ["item", "observer"], [])
        iid, obs = ident(a["item"]), ident(a["observer"])
        need(iid in self.items)
        v = self.item_view(iid)
        theft = v["owner"] == obs and v["possessor"] is not None and v["possessor"] != obs and v["status"] != "borrowed"
        return {"item": v, "loss_weight_pct": 300 if obs in v["sentimental_to"] else 100, "observer": obs,
                "theft_appraisal": theft}

    def workflow(self, request: Any) -> dict:
        table = {"tick": self.op_tick, "environment": self.op_environment, "weather": self.op_weather,
                 "conditions": self.op_conditions, "place": self.op_place, "enter": self.op_enter,
                 "leave": self.op_leave, "reserve": self.op_reserve, "release": self.op_release,
                 "trace_kind": self.op_trace_kind, "act": self.op_act, "perceive": self.op_perceive,
                 "item": self.op_item, "transfer": self.op_transfer, "damage": self.op_damage,
                 "inspect_item": self.op_inspect_item}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Surroundings().workflow(request)
