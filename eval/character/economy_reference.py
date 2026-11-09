"""Independent original reference for economy feedback (pass 2, C4).
Python standard library only; nothing here calls a model.

Goods have a base price, an elasticity, a floor and a ceiling for the price multiplier, and a
spoilage rate (all per-mille; a multiplier of 1000 is the base price, the same unit as community
market `price_mult`). Each market keeps stock per good. During a period beings `produce` (off-screen
work adds stock), `want` (needs register demand) and `buy` (stock goes down, wants are met).
`settle` closes the period for every market: with `supply = stock + sold` (what was on offer this
period), scarcity pressure `p = (demand - supply) * 1000 / max(1, demand + supply)` (truncated toward
zero, -1000..1000) gives a
target multiplier `clamp(1000 + p * elasticity / 1000, floor, ceiling)`; the multiplier moves
toward it by `rate` per-mille of the gap. A good at or above 1500 is `scarce`: beings whose wants
went unmet get an aspiration `need` proposal and its producers an `opportunity` proposal. At or
below 700 it is a `glut`. Then wants and production reset and stock spoils.
"""
import pathlib
import sys
from typing import Any, Dict, List

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_uint, uint
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
MAX_AMOUNT = 1_000_000
SPEC = [("goods", 64, 4096), ("markets", 16, 1024), ("traders", 256, 65536)]
SCARCE_AT, GLUT_AT = 1500, 700


def tdiv(a: int, b: int) -> int:
    """Integer division truncated toward zero (as in Rust)."""
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b > 0) else -q


class Economy:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.goods: Dict[str, dict] = {}
        self.markets: Dict[str, Dict[str, dict]] = {}
        self.clock = 0

    def entry(self, m: str, g: str, create: bool) -> dict:
        need(g in self.goods)
        if m not in self.markets:
            need(create and len(self.markets) < self.lim["markets"])
            self.markets[m] = {}
        return self.markets[m].setdefault(g, {"stock": 0, "mult": 1000, "demand": 0, "wants": {}, "made": {}, "sold": 0})

    def price(self, g: str, e: dict) -> int:
        return self.goods[g]["base"] * e["mult"] // 1000

    def view(self, m: str, g: str) -> dict:
        e = self.markets[m][g]
        return {"demand": e["demand"], "good": g, "market": m, "mult": e["mult"], "price": self.price(g, e),
                "sold": e["sold"], "stock": e["stock"]}

    def traders(self, e: dict, b: str) -> None:
        names = set(e["wants"]) | set(e["made"])
        need(b in names or len(names) < self.lim["traders"])

    # ---------------------------------------------------------------- ops

    def op_good(self, a: dict) -> dict:
        keys(a, ["id", "base"], ["elasticity", "floor", "ceiling", "rate", "spoil"])
        g = ident(a["id"])
        good = {"base": uint(a["base"], 1, 1_000_000_000), "elasticity": opt_uint(a, "elasticity", 0, 10_000, 1500),
                "floor": opt_uint(a, "floor", 1, 1000, 250), "ceiling": opt_uint(a, "ceiling", 1000, 100_000, 4000),
                "rate": opt_uint(a, "rate", 1, 1000, 500), "spoil": opt_uint(a, "spoil", 0, 1000, 0)}
        need(g in self.goods or len(self.goods) < self.lim["goods"])
        self.goods[g] = good
        return {"id": g, **good}

    def add_flow(self, a: dict, field: str, book: str) -> dict:
        keys(a, ["market", "good", "amount", "by"], [])
        m, g, b = ident(a["market"]), ident(a["good"]), ident(a["by"])
        amount = uint(a["amount"], 1, MAX_AMOUNT)
        need(g in self.goods and (m in self.markets or len(self.markets) < self.lim["markets"]))
        cur = self.markets.get(m, {}).get(g)
        if cur is not None:
            self.traders(cur, b)
            need(cur[field] + amount <= MAX_AT)
        e = self.entry(m, g, True)
        e[field] += amount
        e[book][b] = e[book].get(b, 0) + amount
        return self.view(m, g)

    def op_produce(self, a: dict) -> dict:
        return self.add_flow(a, "stock", "made")

    def op_want(self, a: dict) -> dict:
        return self.add_flow(a, "demand", "wants")

    def op_buy(self, a: dict) -> dict:
        keys(a, ["market", "good", "amount", "by"], [])
        m, g, b = ident(a["market"]), ident(a["good"]), ident(a["by"])
        amount = uint(a["amount"], 1, MAX_AMOUNT)
        need(m in self.markets and g in self.markets[m])
        e = self.markets[m][g]
        need(e["stock"] >= amount)
        if b in e["wants"]:
            met = min(amount, e["wants"][b])
            e["wants"][b] -= met
        cost = amount * self.price(g, e)
        e["stock"] -= amount
        e["sold"] += amount
        return {**self.view(m, g), "amount": amount, "by": b, "cost": cost}

    def op_settle(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        goods_out: List[dict] = []
        effects: List[dict] = []
        for m in sorted(self.markets):
            for g in sorted(self.markets[m]):
                e = self.markets[m][g]
                gd = self.goods[g]
                supply = e["stock"] + e["sold"]
                p = tdiv((e["demand"] - supply) * 1000, max(1, e["demand"] + supply))
                target = max(gd["floor"], min(gd["ceiling"], 1000 + tdiv(p * gd["elasticity"], 1000)))
                before = e["mult"]
                e["mult"] = before + tdiv((target - before) * gd["rate"], 1000)
                state = "scarce" if e["mult"] >= SCARCE_AT else "glut" if e["mult"] <= GLUT_AT else "steady"
                unmet = sorted((b, w) for b, w in e["wants"].items() if w > 0)
                if state == "scarce":
                    for b, w in unmet:
                        effects.append({"engine": "aspiration", "op": "experience",
                                        "args": {"being": b, "kind": "need", "subject": g, "weight": min(100, 10 + w)}})
                    for b in sorted(e["made"]):
                        effects.append({"engine": "aspiration", "op": "experience",
                                        "args": {"being": b, "kind": "opportunity", "subject": g,
                                                 "weight": min(100, e["mult"] // 50)}})
                goods_out.append({"demand": e["demand"], "from": before, "good": g, "market": m, "mult": e["mult"],
                                  "pressure": p, "price": self.price(g, e), "sold": e["sold"], "state": state,
                                  "stock": e["stock"], "target": target, "unmet": sum(w for _, w in unmet)})
                e["stock"] -= e["stock"] * gd["spoil"] // 1000
                e["demand"], e["sold"], e["wants"], e["made"] = 0, 0, {}, {}
        self.clock = now
        return {"effects": effects, "goods": goods_out, "now": now}

    def op_price(self, a: dict) -> dict:
        keys(a, ["market", "good"], [])
        m, g = ident(a["market"]), ident(a["good"])
        need(m in self.markets and g in self.markets[m])
        return self.view(m, g)

    def op_assess(self, a: dict) -> dict:
        keys(a, ["market"], [])
        m = ident(a["market"])
        need(m in self.markets)
        return {"goods": [self.view(m, g) for g in sorted(self.markets[m])], "market": m}

    def workflow(self, request: Any) -> dict:
        table = {"good": self.op_good, "produce": self.op_produce, "want": self.op_want, "buy": self.op_buy,
                 "settle": self.op_settle, "price": self.op_price, "assess": self.op_assess}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Economy().workflow(request)
