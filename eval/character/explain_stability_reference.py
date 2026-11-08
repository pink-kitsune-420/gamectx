"""Pure-Python reference for the behavior explanation trace (pass 2, D4) and the
stability monitors (pass 2, D1).

Written independently of src/character/explain.rs and src/character/stability.rs; the
differential runner in explain_stability_cases.py checks the two agree byte for byte
(as parsed JSON) on conformance cases and randomized workflows.
"""
from typing import Any, Dict, List, Optional
from limits_reference import Limits, limited

MAX_OPS = 64
MAX_TRACES = 128
MAX_BEINGS = 64
MAX_AT = 1_000_000_000_000
MOTIVE_KINDS = ["emergency", "need", "promise", "routine", "task", "goal", "reaction"]
BELIEF_SOURCES = ["seen", "told", "inferred", "authored"]
RECORD_STATUSES = ["pending", "success", "failure", "interrupted"]
CONFIRM_STATUSES = ["success", "failure", "interrupted"]

MAX_AUDIT = 64
MAX_TRACKED = 256
MAX_WARNINGS = 256
MAX_TURN = 1_000_000_000_000
MAX_TOTAL = 1_000_000_000
KINDS = ["deadlock", "dithering", "grudge_runaway", "herding", "rumor_runaway", "starvation_spiral"]
IDENT_CHARS = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_:-")


class Bad(ValueError):
    pass


def need(cond: bool) -> None:
    if not cond:
        raise Bad("invalid or out-of-bounds runtime value")


def check_keys(v: Any, required: List[str], optional: List[str]) -> Dict[str, Any]:
    need(isinstance(v, dict))
    for k in required:
        need(k in v)
    for k in v:
        need(k in required or k in optional)
    return v


def ident(v: Any) -> str:
    need(isinstance(v, str) and 0 < len(v.encode()) <= 64 and all(ch in IDENT_CHARS for ch in v))
    return v


def text(v: Any, max_len: int) -> str:
    need(isinstance(v, str) and 0 < len(v) <= max_len)
    need(not any(ord(ch) < 32 or 127 <= ord(ch) <= 159 for ch in v))
    return v


def uint(v: Any, lo: int, hi: int) -> int:
    need(isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi)
    return v


def get_opt(d: Dict[str, Any], k: str) -> Any:
    return d.get(k)


def opt_uint(d: Dict[str, Any], k: str, lo: int, hi: int, default: int) -> int:
    v = d.get(k)
    return default if v is None else uint(v, lo, hi)


def one_of(v: Any, allowed: List[str]) -> str:
    s = text(v, 32)
    need(s in allowed)
    return s


def bounded_list(v: Any, max_len: int) -> list:
    if v is None:
        return []
    need(isinstance(v, list) and len(v) <= max_len)
    return v


def run_ops(request: Any, dispatch) -> Dict[str, Any]:
    check_keys(request, ["ops"], [])
    ops = request["ops"]
    need(isinstance(ops, list) and 0 < len(ops) <= MAX_OPS)
    results = []
    for item in ops:
        check_keys(item, ["op"], ["args"])
        op = text(item["op"], 32)
        args = item.get("args")
        if args is None:
            args = {}
        results.append(dispatch(op, args))
    return {"version": 1, "results": results}


# ---------------------------------------------------------------- explain (D4)

def render_trace(t: Dict[str, Any]) -> Dict[str, Any]:
    m = t["motive"]
    chain = []
    for p in t["perceptions"]:
        chain.append({"detail": f"{p['source']}: {p['text']}", "stage": "perception"})
    for b in t["beliefs"]:
        chain.append({"detail": f"{b['about']}: {b['fact']} ({b['source']}, {b['confidence']}%)", "stage": "belief"})
    md = f"{m['id']} [{m['kind']}] score {m['score']} rank {m['rank']}: {m['reason']}"
    if m["runner_up"] is not None:
        md += f" (over {m['runner_up']['id']} at {m['runner_up']['score']})"
    chain.append({"detail": md, "stage": "motive"})
    task = t["task"]
    if task is not None:
        chain.append({"detail": f"{task['id']} step {task['index']}: {task['step']}", "stage": "task"})
    ad = t["action"] + (f" -> {t['target']}" if t["target"] is not None else "")
    chain.append({"detail": ad, "stage": "action"})
    out = t["outcome"]
    od = out["status"] + (f": {out['summary']}" if out["summary"] is not None else "")
    chain.append({"detail": od, "stage": "outcome"})

    why = f"{t['being']} did {t['action']}"
    if t["target"] is not None:
        why += f" to {t['target']}"
    why += f" because {m['reason']} ({m['kind']} motive {m['id']}, score {m['score']}"
    if m["runner_up"] is not None:
        why += f", beating {m['runner_up']['id']} at {m['runner_up']['score']}"
    why += ")."
    if t["perceptions"]:
        why += " Triggered by: " + "; ".join(p["text"] for p in t["perceptions"]) + "."
    if t["beliefs"]:
        why += " Believed: " + "; ".join(b["fact"] for b in t["beliefs"]) + "."
    if task is not None:
        why += f" Step {task['index']} of task {task['id']}."
    why += f" Outcome: {out['status']}"
    if out["summary"] is not None:
        why += f" ({out['summary']})"
    why += "."

    view = {k: v for k, v in t.items()}
    view["motive"] = dict(m)
    view["outcome"] = dict(out)
    view["chain"] = chain
    view["why"] = why
    return view


class ExplainReference:
    def __init__(self) -> None:
        self.limits = Limits([("beings", 64, 1_000_000), ("traces", 128, 4096)])
        self.next_seq = 1
        self.beings: Dict[str, List[Dict[str, Any]]] = {}

    def run(self, request: Any) -> Dict[str, Any]:
        return limited(request, self.limits, self.run_inner)

    def run_inner(self, request: Any) -> Dict[str, Any]:
        return run_ops(request, self.dispatch)

    def dispatch(self, op: str, args: Any) -> Dict[str, Any]:
        if op == "init":
            return self.init(args)
        if op == "record_trace":
            return self.record(args)
        if op == "confirm_outcome":
            return self.confirm(args)
        if op == "query_trace":
            return self.query(args)
        raise Bad(f"unknown explain op {op}")

    def init(self, args: Any) -> Dict[str, Any]:
        check_keys(args, [], ["being"])
        b = args.get("being")
        if b is None:
            cleared = sum(len(h) for h in self.beings.values())
            self.beings = {}
        else:
            ident(b)
            cleared = len(self.beings.pop(b, []))
        return {"cleared": cleared, "ok": True}

    def record(self, args: Any) -> Dict[str, Any]:
        check_keys(args, ["being", "action", "motive"], ["target", "at", "perceptions", "beliefs", "task", "outcome"])
        being = ident(args["being"])
        action = ident(args["action"])
        target = None if args.get("target") is None else ident(args["target"])
        at = opt_uint(args, "at", 0, MAX_AT, 0)

        perceptions = []
        for p in bounded_list(args.get("perceptions"), 8):
            check_keys(p, ["source", "text"], [])
            perceptions.append({"source": ident(p["source"]), "text": text(p["text"], 160)})

        beliefs = []
        for b in bounded_list(args.get("beliefs"), 8):
            check_keys(b, ["about", "fact", "source"], ["confidence"])
            beliefs.append({
                "about": ident(b["about"]),
                "confidence": opt_uint(b, "confidence", 0, 100, 100),
                "fact": text(b["fact"], 160),
                "source": one_of(b["source"], BELIEF_SOURCES),
            })

        mv = check_keys(args["motive"], ["id", "kind", "score", "reason"], ["rank", "runner_up"])
        runner_up = None
        if mv.get("runner_up") is not None:
            r = check_keys(mv["runner_up"], ["id", "score"], [])
            runner_up = {"id": ident(r["id"]), "score": uint(r["score"], 0, 100)}
        motive = {
            "id": ident(mv["id"]),
            "kind": one_of(mv["kind"], MOTIVE_KINDS),
            "rank": opt_uint(mv, "rank", 1, 64, 1),
            "reason": text(mv["reason"], 200),
            "runner_up": runner_up,
            "score": uint(mv["score"], 0, 100),
        }

        task = None
        if args.get("task") is not None:
            tv = check_keys(args["task"], ["id", "step", "index"], [])
            task = {"id": ident(tv["id"]), "index": uint(tv["index"], 0, 63), "step": ident(tv["step"])}

        outcome = {"status": "pending", "summary": None}
        if args.get("outcome") is not None:
            ov = check_keys(args["outcome"], ["status"], ["summary"])
            outcome = {
                "status": one_of(ov["status"], RECORD_STATUSES),
                "summary": None if ov.get("summary") is None else text(ov["summary"], 200),
            }

        need(being in self.beings or len(self.beings) < self.limits["beings"])
        seq = self.next_seq
        self.next_seq += 1
        tid = f"tr_{seq}"
        hist = self.beings.setdefault(being, [])
        evicted = 0
        if len(hist) >= self.limits["traces"]:
            hist.pop(0)
            evicted = 1
        hist.append({
            "action": action,
            "at": at,
            "being": being,
            "beliefs": beliefs,
            "id": tid,
            "motive": motive,
            "outcome": outcome,
            "perceptions": perceptions,
            "seq": seq,
            "target": target,
            "task": task,
        })
        return {"being": being, "evicted": evicted, "id": tid, "ok": True, "stored": len(hist)}

    def confirm(self, args: Any) -> Dict[str, Any]:
        check_keys(args, ["being", "id", "status"], ["summary"])
        being = ident(args["being"])
        tid = ident(args["id"])
        status = one_of(args["status"], CONFIRM_STATUSES)
        summary = None if args.get("summary") is None else text(args["summary"], 200)
        need(being in self.beings)
        found = [t for t in self.beings[being] if t["id"] == tid]
        need(len(found) == 1)
        t = found[0]
        need(t["outcome"]["status"] == "pending")
        t["outcome"] = {"status": status, "summary": summary}
        return {"being": being, "id": tid, "ok": True, "status": status}

    def query(self, args: Any) -> Dict[str, Any]:
        check_keys(args, ["being"], ["id", "action", "limit"])
        being = ident(args["being"])
        tid = None if args.get("id") is None else ident(args["id"])
        action = None if args.get("action") is None else ident(args["action"])
        limit = opt_uint(args, "limit", 1, self.limits["traces"], 8)
        need(being in self.beings)
        hist = self.beings[being]
        picked = []
        for t in reversed(hist):
            if tid is not None and t["id"] != tid:
                continue
            if action is not None and t["action"] != action:
                continue
            if len(picked) >= limit:
                break
            picked.append(render_trace(t))
        return {"being": being, "count": len(picked), "total": len(hist), "traces": picked}


# ---------------------------------------------------------------- stability (D1)

DEFAULT_CONFIG = {
    "critical": 80,
    "dither_margin": 10,
    "dither_switches": 3,
    "herd_min": 3,
    "herd_pct": 80,
    "runaway_floor": 100,
    "runaway_turns": 3,
    "spiral_turns": 3,
    "window": 8,
}
CONFIG_BOUNDS = {
    "window": (2, 32),
    "dither_switches": (1, 31),
    "dither_margin": (0, 100),
    "critical": (1, 100),
    "spiral_turns": (1, 64),
    "herd_min": (2, 64),
    "herd_pct": (51, 100),
    "runaway_turns": (1, 64),
    "runaway_floor": (0, MAX_TOTAL),
}


class StabilityReference:
    def __init__(self, config: Optional[Dict[str, int]] = None) -> None:
        self.limits = Limits([("tracked_beings", 256, 1_000_000), ("audit_beings", 64, 4096), ("warnings", 256, 65_536)])
        self.config = dict(DEFAULT_CONFIG if config is None else config)
        self.watch: Dict[str, Dict[str, Any]] = {}
        self.totals_watch = {"grudge": {"last": None, "streak": 0}, "rumor": {"last": None, "streak": 0}}
        self.last_turn: Optional[int] = None
        self.turns_audited = 0
        self.last_kinds: List[str] = []
        self.totals: Dict[str, int] = {}
        self.log: List[Dict[str, Any]] = []
        self.next_warning = 1

    def run(self, request: Any) -> Dict[str, Any]:
        return limited(request, self.limits, self.run_inner)

    def run_inner(self, request: Any) -> Dict[str, Any]:
        return run_ops(request, self.dispatch)

    def dispatch(self, op: str, args: Any) -> Dict[str, Any]:
        if op == "init":
            return self.init(args)
        if op == "audit_turn":
            return self.audit(args)
        if op == "assess_health":
            return self.health(args)
        if op == "get_warnings":
            return self.get_warnings(args)
        raise Bad(f"unknown stability op {op}")

    def init(self, args: Any) -> Dict[str, Any]:
        check_keys(args, [], list(CONFIG_BOUNDS))
        cfg = {}
        for k, (lo, hi) in CONFIG_BOUNDS.items():
            cfg[k] = opt_uint(args, k, lo, hi, DEFAULT_CONFIG[k])
        need(cfg["dither_switches"] < cfg["window"])
        limits = self.limits
        self.__init__(cfg)
        self.limits = limits
        return {"config": dict(cfg), "ok": True}

    def parse_report(self, b: Any) -> Dict[str, Any]:
        check_keys(b, ["being", "motive", "activity"],
                   ["score", "runner_up", "needs", "fatigue", "recovering", "holds", "wants"])
        rep = {
            "being": ident(b["being"]),
            "motive": ident(b["motive"]),
            "activity": ident(b["activity"]),
        }
        score = opt_uint(b, "score", 0, 100, 0)
        if b.get("runner_up") is None:
            rep["margin"] = 100
        else:
            rep["margin"] = max(0, score - uint(b["runner_up"], 0, 100))
        severity = opt_uint(b, "fatigue", 0, 100, 0)
        worst = "fatigue"
        seen = set()
        for n in bounded_list(b.get("needs"), 16):
            check_keys(n, ["id", "level"], [])
            nid = ident(n["id"])
            need(nid != "fatigue" and nid not in seen)
            seen.add(nid)
            level = uint(n["level"], 0, 100)
            if level > severity or (level == severity and nid < worst):
                severity, worst = level, nid
        rep["severity"] = severity
        rep["worst"] = worst
        rec = b.get("recovering")
        need(rec is None or isinstance(rec, bool))
        rep["recovering"] = bool(rec)
        rep["holds"] = sorted({ident(x) for x in bounded_list(b.get("holds"), 8)})
        rep["wants"] = sorted({ident(x) for x in bounded_list(b.get("wants"), 8)})
        return rep

    def audit(self, args: Any) -> Dict[str, Any]:
        check_keys(args, ["turn", "beings"], ["world"])
        turn = uint(args["turn"], 0, MAX_TURN)
        need(self.last_turn is None or turn > self.last_turn)
        raw = args["beings"]
        need(isinstance(raw, list) and len(raw) <= self.limits["audit_beings"])
        reports = [self.parse_report(b) for b in raw]
        names = [r["being"] for r in reports]
        need(len(set(names)) == len(names))
        grudge = rumor = None
        world = args.get("world")
        if world is not None:
            check_keys(world, [], ["grudge_total", "rumor_total"])
            if world.get("grudge_total") is not None:
                grudge = uint(world["grudge_total"], 0, MAX_TOTAL)
            if world.get("rumor_total") is not None:
                rumor = uint(world["rumor_total"], 0, MAX_TOTAL)
        fresh = [n for n in names if n not in self.watch]
        need(len(self.watch) + len(fresh) <= self.limits["tracked_beings"])

        cfg = self.config
        self.last_turn = turn
        self.turns_audited += 1
        reports.sort(key=lambda r: r["being"])
        found: List[Dict[str, Any]] = []

        for r in reports:
            w = self.watch.setdefault(r["being"], {"history": [], "streak": 0, "last": None})
            hist = w["history"]
            hist.append((r["motive"], r["margin"]))
            del hist[:max(0, len(hist) - cfg["window"])]
            switches = 0
            for i in range(1, len(hist)):
                if hist[i][0] != hist[i - 1][0] and hist[i][1] <= cfg["dither_margin"]:
                    switches += 1
            motives = sorted({h[0] for h in hist})
            if switches >= cfg["dither_switches"] and len(motives) == 2:
                found.append({
                    "beings": [r["being"]],
                    "detail": (f"{r['being']} switched {switches} times between {motives[0]} and {motives[1]}"
                               f" in {len(hist)} turns; hold {r['motive']} for commitment inertia"),
                    "kind": "dithering",
                    "motives": motives,
                    "switches": switches,
                })

            worsening = w["last"] is None or r["severity"] >= w["last"]
            if r["severity"] >= cfg["critical"] and not r["recovering"] and worsening:
                w["streak"] += 1
            else:
                w["streak"] = 0
            w["last"] = r["severity"]
            if w["streak"] >= cfg["spiral_turns"]:
                found.append({
                    "beings": [r["being"]],
                    "detail": (f"{r['being']} stayed critical on {r['worst']} at {r['severity']}"
                               f" for {w['streak']} turns without recovering"),
                    "kind": "starvation_spiral",
                    "need": r["worst"],
                    "severity": r["severity"],
                    "streak": w["streak"],
                })

        found.extend(find_deadlocks(reports))

        total = len(reports)
        if total >= cfg["herd_min"]:
            counts: Dict[str, int] = {}
            for r in reports:
                counts[r["activity"]] = counts.get(r["activity"], 0) + 1
            top = min(counts, key=lambda a: (-counts[a], a))
            best = counts[top]
            if best * 100 >= cfg["herd_pct"] * total:
                found.append({
                    "activity": top,
                    "beings": [r["being"] for r in reports if r["activity"] == top],
                    "count": best,
                    "detail": f"{best} of {total} beings chose {top} in the same turn",
                    "kind": "herding",
                    "total": total,
                })

        for label, value in (("grudge", grudge), ("rumor", rumor)):
            if value is None:
                continue
            tw = self.totals_watch[label]
            if tw["last"] is not None and value > tw["last"]:
                tw["streak"] += 1
            else:
                tw["streak"] = 0
            tw["last"] = value
            if tw["streak"] >= cfg["runaway_turns"] and value >= cfg["runaway_floor"]:
                found.append({
                    "beings": [],
                    "detail": f"{label} total rose {tw['streak']} turns in a row to {value}",
                    "kind": f"{label}_runaway",
                    "streak": tw["streak"],
                    "total": value,
                })

        kinds = set()
        out = []
        for f in found:
            f["id"] = f"w_{self.next_warning}"
            self.next_warning += 1
            f["turn"] = turn
            self.totals[f["kind"]] = self.totals.get(f["kind"], 0) + 1
            kinds.add(f["kind"])
            if len(self.log) >= self.limits["warnings"]:
                self.log.pop(0)
            self.log.append(f)
            out.append(f)
        self.last_kinds = sorted(kinds)
        return {"count": len(out), "status": "warn" if out else "ok", "turn": turn, "warnings": out}

    def health(self, args: Any) -> Dict[str, Any]:
        check_keys(args, [], [])
        totals = {k: self.totals.get(k, 0) for k in KINDS}
        if self.last_kinds:
            status = "unstable"
        elif sum(totals.values()) > 0:
            status = "recovered"
        else:
            status = "healthy"
        return {
            "active": list(self.last_kinds),
            "beings_tracked": len(self.watch),
            "last_turn": self.last_turn,
            "status": status,
            "totals": totals,
            "turns_audited": self.turns_audited,
            "warnings_logged": len(self.log),
        }

    def get_warnings(self, args: Any) -> Dict[str, Any]:
        check_keys(args, [], ["kind", "being", "since_turn", "limit"])
        kind = None if args.get("kind") is None else text(args["kind"], 32)
        need(kind is None or kind in KINDS)
        being = None if args.get("being") is None else ident(args["being"])
        since = opt_uint(args, "since_turn", 0, MAX_TURN, 0)
        limit = opt_uint(args, "limit", 1, self.limits["warnings"], 32)
        matched = [
            w for w in self.log
            if (kind is None or w["kind"] == kind)
            and (being is None or being in w["beings"])
            and w["turn"] >= since
        ]
        picked = matched[-limit:]
        return {"count": len(picked), "warnings": picked}


def find_deadlocks(reports: List[Dict[str, Any]]) -> List[Dict[str, Any]]:
    """Strongly connected groups (size >= 2) of the wait-for graph, via Tarjan-free
    pairwise reachability so the result is independent of traversal order."""
    n = len(reports)
    succ = [[j for j in range(n) if j != i and set(reports[i]["wants"]) & set(reports[j]["holds"])] for i in range(n)]

    def reachable(s: int) -> set:
        seen: set = set()
        frontier = list(succ[s])
        while frontier:
            x = frontier.pop()
            if x in seen:
                continue
            seen.add(x)
            frontier.extend(succ[x])
        return seen

    reach = [reachable(i) for i in range(n)]
    taken: set = set()
    out = []
    for i in range(n):
        if i in taken or i not in reach[i]:
            continue
        group = [j for j in range(n) if j in reach[i] and i in reach[j]]
        taken.update(group)
        members = sorted(reports[j]["being"] for j in group)
        resources = sorted({
            r for a in group for b in group if a != b
            for r in set(reports[a]["wants"]) & set(reports[b]["holds"])
        })
        out.append({
            "beings": members,
            "detail": f"{', '.join(members)} wait on each other for {', '.join(resources)}",
            "kind": "deadlock",
            "resources": resources,
        })
    return out


def process_local(kind: str, args: Any) -> Dict[str, Any]:
    if kind == "explain":
        return ExplainReference().run(args)
    if kind == "stability":
        return StabilityReference().run(args)
    raise Bad(f"unknown local kind {kind}")
