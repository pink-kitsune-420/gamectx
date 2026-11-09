"""Independent original reference for save growth: retention and migration (pass 2, D3).
Python standard library only; nothing here calls a model.

Retention: the host registers what its saves keep (memories, lessons, expectations, ledgers) as
entries in named collections, each `{id, at, used, importance 0..100, bytes, pinned}`, and gives
each collection a rule: `max_entries`, `max_bytes`, `max_age` (ticks since last use),
`min_importance` and a `grace` period that protects recently used entries. `add` and `touch` may advance the clock
with `now`; `prune` advances it unless it is a dry run. `prune` removes
entries in a fixed order (stale, then unimportant, then the least important and least recently
used until the collection fits) and reports why each one went. Pinned and in-grace entries are
never removed; a collection that still does not fit is reported as `over`.

Migration: the host declares numbered migrations (2, 3, ...), each a list of changes to section
data (`rename`, `add`, `remove`, `clamp` at a dotted path where `*` means every element), and
`migrate` upgrades a document from any older version in order. Work is bounded by counting the
nodes each path reaches.
"""
import copy
import pathlib
import sys
from typing import Any, Dict, List, Optional

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_bool, opt_uint, uint
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
MAX_BYTES = 9_007_199_254_740_991
MAX_WORK = 1_000_000
MAX_SEGMENTS = 8
SPEC = [("collections", 64, 4096), ("entries", 4096, 1_000_000), ("migrations", 16, 256), ("changes", 32, 256)]
CHANGE_OPS = ["add", "clamp", "remove", "rename"]


def parse_path(v: Any) -> List[str]:
    need(isinstance(v, str))
    segs = v.split(".")
    need(0 < len(segs) <= MAX_SEGMENTS)
    for s in segs[:-1]:
        if s != "*":
            ident(s)
    ident(segs[-1])
    return segs


def is_int(v: Any) -> bool:
    return isinstance(v, int) and not isinstance(v, bool)


class Retention:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.cols: Dict[str, dict] = {}
        self.clock = 0
        self.migrations: List[List[dict]] = []  # index i holds migration version i + 2

    # ---------------------------------------------------------------- retention

    def rule_view(self, c: str) -> dict:
        r = self.cols[c]["rule"]
        return {"collection": c, "grace": r["grace"], "max_age": r["max_age"], "max_bytes": r["max_bytes"],
                "max_entries": r["max_entries"], "min_importance": r["min_importance"]}

    def op_rule(self, a: dict) -> dict:
        keys(a, ["collection"], ["max_entries", "max_bytes", "max_age", "min_importance", "grace"])
        c = ident(a["collection"])
        rule = {
            "max_entries": None if a.get("max_entries") is None else uint(a["max_entries"], 1, 1_000_000),
            "max_bytes": None if a.get("max_bytes") is None else uint(a["max_bytes"], 1, MAX_BYTES),
            "max_age": None if a.get("max_age") is None else uint(a["max_age"], 0, MAX_AT),
            "min_importance": opt_uint(a, "min_importance", 0, 100, 0),
            "grace": opt_uint(a, "grace", 0, MAX_AT, 0),
        }
        need(c in self.cols or len(self.cols) < self.lim["collections"])
        self.cols.setdefault(c, {"entries": {}, "bytes": 0})["rule"] = rule
        return self.rule_view(c)

    def entry_view(self, c: str, i: str) -> dict:
        e = self.cols[c]["entries"][i]
        return {"at": e["at"], "bytes": e["bytes"], "collection": c, "id": i, "importance": e["importance"],
                "pinned": e["pinned"], "used": e["used"]}

    def op_add(self, a: dict) -> dict:
        keys(a, ["collection", "id", "bytes"], ["importance", "pinned", "now"])
        c, i = ident(a["collection"]), ident(a["id"])
        size = uint(a["bytes"], 1, 16_777_216)
        imp = opt_uint(a, "importance", 0, 100, 50)
        pinned = opt_bool(a, "pinned", False)
        now = opt_uint(a, "now", 0, MAX_AT, self.clock)
        need(c in self.cols and now >= self.clock)
        col = self.cols[c]
        need(i not in col["entries"] and len(col["entries"]) < self.lim["entries"])
        self.clock = now
        col["entries"][i] = {"at": self.clock, "used": self.clock, "importance": imp, "bytes": size, "pinned": pinned}
        col["bytes"] += size
        return self.entry_view(c, i)

    def op_touch(self, a: dict) -> dict:
        keys(a, ["collection", "id"], ["importance", "pinned", "now"])
        c, i = ident(a["collection"]), ident(a["id"])
        imp = None if a.get("importance") is None else uint(a["importance"], 0, 100)
        pinned = None if a.get("pinned") is None else opt_bool(a, "pinned", False)
        now = opt_uint(a, "now", 0, MAX_AT, self.clock)
        need(c in self.cols and i in self.cols[c]["entries"] and now >= self.clock)
        self.clock = now
        e = self.cols[c]["entries"][i]
        e["used"] = self.clock
        if imp is not None:
            e["importance"] = imp
        if pinned is not None:
            e["pinned"] = pinned
        return self.entry_view(c, i)

    def plan(self, c: str, now: int) -> dict:
        col = self.cols[c]
        r = col["rule"]
        removed: List[dict] = []
        gone = set()
        cands = sorted((i for i, e in col["entries"].items()
                        if not e["pinned"] and now - e["used"] >= r["grace"]),
                       key=lambda i: (col["entries"][i]["importance"], col["entries"][i]["used"],
                                      col["entries"][i]["at"], i))
        for i in cands:
            if r["max_age"] is not None and now - col["entries"][i]["used"] > r["max_age"]:
                removed.append({"id": i, "reason": "stale"})
                gone.add(i)
        for i in cands:
            if i not in gone and col["entries"][i]["importance"] < r["min_importance"]:
                removed.append({"id": i, "reason": "unimportant"})
                gone.add(i)
        count = len(col["entries"]) - len(gone)
        size = col["bytes"] - sum(col["entries"][i]["bytes"] for i in gone)
        for i in cands:
            if i in gone:
                continue
            if r["max_entries"] is not None and count > r["max_entries"]:
                reason = "over_entries"
            elif r["max_bytes"] is not None and size > r["max_bytes"]:
                reason = "over_bytes"
            else:
                break
            removed.append({"id": i, "reason": reason})
            gone.add(i)
            count -= 1
            size -= col["entries"][i]["bytes"]
        over = (r["max_entries"] is not None and count > r["max_entries"]) or \
               (r["max_bytes"] is not None and size > r["max_bytes"])
        return {"bytes_after": size, "bytes_before": col["bytes"], "collection": c, "kept": count, "over": over,
                "removed": removed, "gone": gone}

    def op_prune(self, a: dict) -> dict:
        keys(a, ["now"], ["dry_run", "collection"])
        now = uint(a["now"], 0, MAX_AT)
        dry = opt_bool(a, "dry_run", False)
        only = None if a.get("collection") is None else ident(a["collection"])
        need(now >= self.clock and (only is None or only in self.cols))
        reports = []
        for c in sorted(self.cols) if only is None else [only]:
            p = self.plan(c, now)
            gone = p.pop("gone")
            if not dry:
                col = self.cols[c]
                for i in gone:
                    del col["entries"][i]
                col["bytes"] = p["bytes_after"]
            reports.append(p)
        if not dry:
            self.clock = now
        return {"collections": reports, "dry_run": dry, "freed": sum(p["bytes_before"] - p["bytes_after"] for p in reports),
                "now": now, "removed": sum(len(p["removed"]) for p in reports)}

    def op_assess(self, a: dict) -> dict:
        keys(a, ["collection"], [])
        c = ident(a["collection"])
        need(c in self.cols)
        col = self.cols[c]
        es = col["entries"].values()
        return {"bytes": col["bytes"], "entries": len(col["entries"]),
                "oldest_used": min((e["used"] for e in es), default=None),
                "pinned": sum(1 for e in es if e["pinned"]), "rule": self.rule_view(c)}

    # ---------------------------------------------------------------- migration

    def op_migration(self, a: dict) -> dict:
        keys(a, ["version", "changes"], [])
        v = uint(a["version"], 2, 1_000_000)
        rows = a["changes"]
        need(isinstance(rows, list) and 0 < len(rows) <= self.lim["changes"])
        changes = []
        for r in rows:
            need(isinstance(r, dict) and r.get("op") in CHANGE_OPS)
            kind = r["op"]
            if kind == "rename":
                keys(r, ["op", "path", "to"], [])
                changes.append({"op": kind, "path": parse_path(r["path"]), "to": ident(r["to"])})
            elif kind == "add":
                keys(r, ["op", "path", "value"], [])
                changes.append({"op": kind, "path": parse_path(r["path"]), "value": r["value"]})
            elif kind == "remove":
                keys(r, ["op", "path"], [])
                changes.append({"op": kind, "path": parse_path(r["path"])})
            else:
                keys(r, ["op", "path", "min", "max"], [])
                lo, hi = r["min"], r["max"]
                need(is_int(lo) and is_int(hi) and -MAX_BYTES <= lo <= hi <= MAX_BYTES)
                changes.append({"op": kind, "path": parse_path(r["path"]), "min": lo, "max": hi})
        need(v == len(self.migrations) + 2 and len(self.migrations) < self.lim["migrations"])
        self.migrations.append(changes)
        return {"changes": len(changes), "latest": v}

    def op_migrate(self, a: dict) -> dict:
        keys(a, ["document", "from"], [])
        doc = a["document"]
        need(isinstance(doc, dict))
        latest = len(self.migrations) + 1
        frm = uint(a["from"], 1, latest)
        doc = copy.deepcopy(doc)
        work = [0]
        edits = 0
        for changes in self.migrations[frm - 1:]:
            for ch in changes:
                edits += self.apply(doc, ch, work)
        return {"document": doc, "edits": edits, "from": frm, "to": latest, "work": work[0]}

    @staticmethod
    def parents(doc: dict, segs: List[str], work: List[int]) -> List[Any]:
        nodes = [doc]
        for s in segs:
            nxt = []
            for n in nodes:
                if s == "*":
                    kids = n if isinstance(n, list) else list(n.values()) if isinstance(n, dict) else []
                    nxt.extend(kids)
                elif isinstance(n, dict) and s in n:
                    nxt.append(n[s])
            work[0] += len(nxt)
            need(work[0] <= MAX_WORK)
            nodes = nxt
        return [n for n in nodes if isinstance(n, dict)]

    def apply(self, doc: dict, ch: dict, work: List[int]) -> int:
        *head, last = ch["path"]
        edits = 0
        for obj in self.parents(doc, head, work):
            if ch["op"] == "rename":
                if last in obj and ch["to"] != last:
                    need(ch["to"] not in obj)
                    items = [(ch["to"] if k == last else k, v) for k, v in obj.items()]
                    obj.clear()
                    obj.update(items)
                    edits += 1
            elif ch["op"] == "add":
                if last not in obj:
                    obj[last] = copy.deepcopy(ch["value"])
                    edits += 1
            elif ch["op"] == "remove":
                if last in obj:
                    del obj[last]
                    edits += 1
            else:
                if last in obj:
                    v = obj[last]
                    need(is_int(v) and -MAX_BYTES <= v <= MAX_BYTES)
                    c = max(ch["min"], min(ch["max"], v))
                    if c != v:
                        obj[last] = c
                        edits += 1
        return edits

    def workflow(self, request: Any) -> dict:
        table = {"rule": self.op_rule, "add": self.op_add, "touch": self.op_touch, "prune": self.op_prune,
                 "assess": self.op_assess, "migration": self.op_migration, "migrate": self.op_migrate}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Retention().workflow(request)
