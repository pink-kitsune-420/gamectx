"""Independent original reference for cooperation and conflict between beings (pass 2, B2/B3).
Python standard library only; nothing here calls a model.

B2 cooperation: joint tasks with roles, delegation by the organizer, asking for help (accepted
from trust, favors owed, busyness, effort and grudges), hand-offs, completion and leaving, and
promises between characters that are kept, released or broken by the clock.

B3 conflict: claims on a resource (granted when free, otherwise queued by urgency and status),
release to the next in line, contests where the holder yields, stands or offers turns, and
grudges that follow and fade.

All scores are signed fixed-point millionths (integers). Effects are proposals for other
engines in their argument shapes (relationship `appraise_action`, expectation `expect`, affect
`appraise`); the host decides whether to apply them.
"""
import pathlib
import sys
from typing import Any, Dict, List, Optional, Tuple

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import Bad, ident, keys, need, opt_uint, run_ops, text, uint
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
M = 1_000_000
SPEC = [("tasks", 64, 1_000_000), ("roles", 8, 64), ("promises", 256, 1_000_000), ("resources", 256, 1_000_000),
        ("queue", 16, 1024), ("beings", 1024, 1_000_000), ("grudges", 32, 4096), ("debts", 4096, 1_000_000)]

# Help: accept when the score reaches HELP_ACCEPT.
HELP_BASE, HELP_TRUST, HELP_FAVOR, HELP_FAVOR_CAP, HELP_BUSY, HELP_EFFORT, HELP_ACCEPT = (
    300_000, 4_000, 100_000, 5, 300_000, 2_000, 500_000)
# Contest: the holder yields at or above YIELD_AT, stands at or below STAND_AT, offers turns between.
CONTEST_BASE, CONTEST_STATUS, CONTEST_URGENCY, CONTEST_TRUST, YIELD_AT, STAND_AT = (
    500_000, 5_000, 4_000, 3_000, 650_000, 350_000)
QUEUE_URGENCY, QUEUE_STATUS = 6_000, 4_000
BOND_TRUST = 20  # a holder yielding with less trust than this resents it
GRUDGE_LEAVE, GRUDGE_BROKEN, GRUDGE_STAND, GRUDGE_STAND_URGENCY, GRUDGE_YIELD, GRUDGE_YIELD_URGENCY = (
    200_000, 300_000, 100_000, 3_000, 100_000, 2_000)
FADE_PER_TICK = 1_000


def sint(v: Any, lo: int, hi: int) -> int:
    need(isinstance(v, int) and not isinstance(v, bool) and lo <= v <= hi)
    return v


def opt_sint(d: dict, k: str, lo: int, hi: int, default: int) -> int:
    return default if d.get(k) is None else sint(d[k], lo, hi)


def opt_flag(d: dict, k: str) -> bool:
    v = d.get(k)
    if v is None:
        return False
    need(isinstance(v, bool))
    return v


def at_of(a: dict) -> int:
    return uint(a.get("at"), 0, MAX_AT) if a.get("at") is not None else 0


def rel(actor: str, target: str, action: str) -> dict:
    return {"engine": "relationship", "op": "appraise_action",
            "args": {"actor": actor, "target": target, "action": action, "success": True}}


def resent(being: str, subject: str, weight: int, reason: str) -> dict:
    return {"engine": "affect", "op": "appraise",
            "args": {"being": being, "subject": subject, "kind": "resentment", "delta": max(1, weight // 10_000),
                     "reason": reason}}


class Cooperation:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.tasks: Dict[str, dict] = {}
        self.promises: Dict[str, dict] = {}
        self.resources: Dict[str, dict] = {}
        self.grudges: Dict[str, Dict[str, list]] = {}  # holder -> against -> [weight, last, reason]
        self.debts: Dict[Tuple[str, str], int] = {}  # (debtor, creditor) -> favors owed
        self.clock = 0
        self.seq = 0

    # ---------------------------------------------------------------- grudges

    def grudge_room(self, holders: List[str]) -> None:
        new = sorted({h for h in holders if h not in self.grudges})
        need(len(self.grudges) + len(new) <= self.lim["beings"])

    def add_grudge(self, holder: str, against: str, weight: int, at: int, reason: str) -> dict:
        book = self.grudges.setdefault(holder, {})
        if against not in book and len(book) >= self.lim["grudges"]:
            weakest = min(book, key=lambda k: (book[k][0], k))
            del book[weakest]
        cur = book.get(against, [0, at, reason])
        cur[0] = min(M, cur[0] + weight)
        cur[1] = max(cur[1], at)
        cur[2] = reason
        book[against] = cur
        return resent(holder, against, weight, reason)

    def grudge(self, holder: str, against: str) -> int:
        return self.grudges.get(holder, {}).get(against, [0])[0]

    def grudge_rows(self, being: str) -> list:
        book = self.grudges.get(being, {})
        return [{"against": k, "weight": v[0], "reason": v[2]}
                for k, v in sorted(book.items(), key=lambda kv: (-kv[1][0], kv[0]))]

    # ---------------------------------------------------------------- B2 joint tasks

    def task(self, a: dict) -> dict:
        t = self.tasks.get(ident(a.get("task")))
        need(t is not None)
        return t

    def role(self, t: dict, a: dict) -> dict:
        r = t["roles"].get(ident(a.get("role")))
        need(r is not None)
        return r

    @staticmethod
    def status(t: dict) -> str:
        rs = t["roles"].values()
        if all(r["done"] for r in rs):
            return "done"
        return "open" if any(r["being"] is None for r in rs) else "ready"

    @staticmethod
    def holds(t: dict, being: str) -> bool:
        return any(r["being"] == being for r in t["roles"].values())

    def view(self, tid: str) -> dict:
        t = self.tasks[tid]
        return {"id": tid, "organizer": t["organizer"], "goal": t["goal"], "status": self.status(t),
                "roles": [{"role": k, "being": r["being"], "done": r["done"], "via": r["via"]}
                          for k, r in sorted(t["roles"].items())]}

    def op_form(self, a: dict) -> dict:
        keys(a, ["id", "organizer", "goal", "roles"], ["at"])
        tid, org, goal = ident(a["id"]), ident(a["organizer"]), text(a["goal"], 64)
        at_of(a)
        need(tid not in self.tasks and len(self.tasks) < self.lim["tasks"])
        rows = a["roles"]
        need(isinstance(rows, list) and 0 < len(rows) <= self.lim["roles"])
        roles: Dict[str, dict] = {}
        for r in rows:
            keys(r, ["role"], ["being"])
            rid = ident(r["role"])
            b = None if r.get("being") is None else ident(r["being"])
            need(rid not in roles and (b is None or all(x["being"] != b for x in roles.values())))
            roles[rid] = {"being": b, "done": False, "via": None}
        self.tasks[tid] = {"organizer": org, "goal": goal, "roles": roles}
        return self.view(tid)

    def op_assign(self, a: dict) -> dict:
        keys(a, ["task", "role", "being", "by"], ["at"])
        t, r = self.task(a), None
        r = self.role(t, a)
        b, by = ident(a["being"]), ident(a["by"])
        at_of(a)
        need(by == t["organizer"] and r["being"] is None and not self.holds(t, b))
        r["being"] = b
        return self.view(a["task"])

    def op_ask_help(self, a: dict) -> dict:
        keys(a, ["task", "role", "asker", "helper", "trust"], ["busy", "effort", "at"])
        t = self.task(a)
        r = self.role(t, a)
        asker, helper = ident(a["asker"]), ident(a["helper"])
        trust = sint(a["trust"], -100, 100)
        busy = opt_flag(a, "busy")
        effort = opt_uint(a, "effort", 0, 100, 50)
        at_of(a)
        need(r["being"] is None and asker != helper and (asker == t["organizer"] or self.holds(t, asker)))
        need(not self.holds(t, helper))
        owed = self.debts.get((helper, asker), 0)
        held = self.grudge(helper, asker)
        score = (HELP_BASE + trust * HELP_TRUST + min(owed, HELP_FAVOR_CAP) * HELP_FAVOR
                 - (HELP_BUSY if busy else 0) - effort * HELP_EFFORT - held // 2)
        reasons = []
        if trust:
            reasons.append("trust")
        if owed:
            reasons.append("owes_favor")
        if busy:
            reasons.append("busy")
        if held:
            reasons.append("grudge")
        accepted = score >= HELP_ACCEPT
        if accepted:
            r["being"], r["via"] = helper, asker
        return {"accepted": accepted, "helper": helper, "reasons": reasons, "role": a["role"], "score": score,
                "task": a["task"]}

    def op_hand_off(self, a: dict) -> dict:
        keys(a, ["task", "role", "from", "to"], ["at"])
        t = self.task(a)
        r = self.role(t, a)
        frm, to = ident(a["from"]), ident(a["to"])
        at_of(a)
        need(r["being"] == frm and not r["done"] and not self.holds(t, to))
        r["being"], r["via"] = to, None
        return self.view(a["task"])

    def op_complete(self, a: dict) -> dict:
        keys(a, ["task", "role", "being"], ["at"])
        t = self.task(a)
        r = self.role(t, a)
        b = ident(a["being"])
        at_of(a)
        need(r["being"] == b and not r["done"])
        effects = []
        via = r["via"]
        if via is not None:
            if self.debts.get((b, via), 0) == 0:
                need((via, b) in self.debts or len(self.debts) < self.lim["debts"])
        r["done"] = True
        if via is not None:
            if self.debts.get((b, via), 0) > 0:
                self.debts[(b, via)] -= 1
                if self.debts[(b, via)] == 0:
                    del self.debts[(b, via)]
            else:
                self.debts[(via, b)] = self.debts.get((via, b), 0) + 1
            effects.append(rel(b, via, "help"))
        return {"effects": effects, "task": self.view(a["task"])}

    def op_leave(self, a: dict) -> dict:
        keys(a, ["task", "role", "being"], ["at"])
        t = self.task(a)
        r = self.role(t, a)
        b = ident(a["being"])
        at = at_of(a)
        need(r["being"] == b and not r["done"])
        via = r["via"]
        effects = []
        if via is not None:
            self.grudge_room([via])
            effects.append(rel(b, via, "broken_promise"))
            effects.append(self.add_grudge(via, b, GRUDGE_LEAVE, at, "left_task"))
        r["being"], r["via"] = None, None
        return {"effects": effects, "task": self.view(a["task"])}

    # ---------------------------------------------------------------- B2 promises

    def op_promise(self, a: dict) -> dict:
        keys(a, ["id", "from", "to", "what", "due"], ["at"])
        pid, frm, to, what = ident(a["id"]), ident(a["from"]), ident(a["to"]), ident(a["what"])
        due = uint(a["due"], 0, MAX_AT)
        at_of(a)
        need(frm != to and pid not in self.promises and len(self.promises) < self.lim["promises"])
        self.promises[pid] = {"from": frm, "to": to, "what": what, "due": due, "status": "pending"}
        effect = {"engine": "expectation", "op": "expect",
                  "args": {"being": to, "id": pid, "about": frm, "event": what, "due": due, "grace": 0,
                           "importance": 50, "kind": "hope"}}
        return {"effects": [effect], "promise": self.promise_view(pid)}

    def promise_view(self, pid: str) -> dict:
        p = self.promises[pid]
        return {"id": pid, "from": p["from"], "to": p["to"], "what": p["what"], "due": p["due"], "status": p["status"]}

    def op_keep(self, a: dict) -> dict:
        keys(a, ["id"], ["at"])
        p = self.promises.get(ident(a["id"]))
        at = at_of(a)
        need(p is not None and p["status"] == "pending")
        p["status"] = "kept" if at <= p["due"] else "kept_late"
        return {"effects": [rel(p["from"], p["to"], "fulfilled_promise")], "promise": self.promise_view(a["id"])}

    def op_release(self, a: dict) -> dict:
        keys(a, ["id", "by"], ["at"])
        p = self.promises.get(ident(a["id"]))
        by = ident(a["by"])
        at_of(a)
        need(p is not None and p["status"] == "pending" and by == p["to"])
        p["status"] = "released"
        return {"effects": [], "promise": self.promise_view(a["id"])}

    def op_tick(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        broken = sorted(pid for pid, p in self.promises.items() if p["status"] == "pending" and p["due"] < now)
        self.grudge_room([self.promises[pid]["to"] for pid in broken])
        effects = []
        faded = []
        for holder in sorted(self.grudges):
            book = self.grudges[holder]
            for against in sorted(book):
                g = book[against]
                g[0] -= max(0, now - g[1]) * FADE_PER_TICK
                g[1] = max(g[1], now)
                if g[0] <= 0:
                    del book[against]
                    faded.append({"holder": holder, "against": against})
            if not book:
                del self.grudges[holder]
        for pid in broken:
            p = self.promises[pid]
            p["status"] = "broken"
            effects.append(rel(p["from"], p["to"], "broken_promise"))
            effects.append(self.add_grudge(p["to"], p["from"], GRUDGE_BROKEN, now, "broken_promise"))
        self.clock = now
        return {"broken": broken, "effects": effects, "faded": faded, "now": now}

    # ---------------------------------------------------------------- B3 resources

    @staticmethod
    def priority(q: dict) -> Tuple[int, int, int]:
        # Turns offered in a contest come first, then urgency and status, then arrival.
        return (0 if q["turn"] else 1, -(q["urgency"] * QUEUE_URGENCY + q["status"] * QUEUE_STATUS), q["seq"])

    def ordered(self, res: dict) -> list:
        return sorted(res["queue"], key=self.priority)

    def position(self, res: dict, being: str) -> int:
        return [q["being"] for q in self.ordered(res)].index(being) + 1

    def enqueue(self, res: dict, being: str, urgency: int, status: int, at: int, turn: bool) -> None:
        self.seq += 1
        res["queue"].append({"being": being, "urgency": urgency, "status": status, "at": at, "seq": self.seq,
                             "turn": turn})

    def res_view(self, rid: str) -> dict:
        res = self.resources[rid]
        return {"resource": rid, "holder": res["holder"], "since": res["since"],
                "queue": [q["being"] for q in self.ordered(res)]}

    def op_claim(self, a: dict) -> dict:
        keys(a, ["resource", "being"], ["urgency", "status", "at"])
        rid, b = ident(a["resource"]), ident(a["being"])
        urgency, status = opt_uint(a, "urgency", 0, 100, 50), opt_uint(a, "status", 0, 100, 50)
        at = at_of(a)
        res = self.resources.get(rid)
        if res is None or res["holder"] is None:
            need(res is not None or len(self.resources) < self.lim["resources"])
            if res is None:
                res = self.resources[rid] = {"holder": None, "since": 0, "queue": []}
            res["holder"], res["since"] = b, at
            return {"granted": True, "position": 0, "resource": self.res_view(rid)}
        need(res["holder"] != b and all(q["being"] != b for q in res["queue"]))
        need(len(res["queue"]) < self.lim["queue"])
        self.enqueue(res, b, urgency, status, at, False)
        return {"granted": False, "position": self.position(res, b), "resource": self.res_view(rid)}

    def op_free(self, a: dict) -> dict:
        keys(a, ["resource", "being"], ["at"])
        rid, b = ident(a["resource"]), ident(a["being"])
        at = at_of(a)
        res = self.resources.get(rid)
        need(res is not None)
        if res["holder"] == b:
            order = self.ordered(res)
            if order:
                nxt = order[0]
                res["queue"] = [q for q in res["queue"] if q is not nxt]
                res["holder"], res["since"] = nxt["being"], at
            else:
                res["holder"], res["since"] = None, at
        else:
            need(any(q["being"] == b for q in res["queue"]))
            res["queue"] = [q for q in res["queue"] if q["being"] != b]
        return {"resource": self.res_view(rid)}

    def op_contest(self, a: dict) -> dict:
        keys(a, ["resource", "challenger", "trust"], ["urgency", "status", "holder_urgency", "holder_status", "at"])
        rid, ch = ident(a["resource"]), ident(a["challenger"])
        trust = sint(a["trust"], -100, 100)
        urg, st = opt_uint(a, "urgency", 0, 100, 50), opt_uint(a, "status", 0, 100, 50)
        h_urg, h_st = opt_uint(a, "holder_urgency", 0, 100, 50), opt_uint(a, "holder_status", 0, 100, 50)
        at = at_of(a)
        res = self.resources.get(rid)
        need(res is not None and res["holder"] is not None and res["holder"] != ch)
        holder = res["holder"]
        held = self.grudge(holder, ch)
        score = (CONTEST_BASE + (st - h_st) * CONTEST_STATUS + (urg - h_urg) * CONTEST_URGENCY
                 + trust * CONTEST_TRUST - held // 2)
        queued = any(q["being"] == ch for q in res["queue"])
        rest = [q for q in res["queue"] if q["being"] != ch]
        effects = []
        if score >= YIELD_AT:
            outcome = "yield"
            need(len(rest) < self.lim["queue"])
            resented = trust < BOND_TRUST
            if resented:
                self.grudge_room([holder])
            res["queue"] = rest
            res["holder"], res["since"] = ch, at
            self.enqueue(res, holder, h_urg, h_st, at, False)
            if resented:
                effects.append(self.add_grudge(holder, ch, GRUDGE_YIELD + h_urg * GRUDGE_YIELD_URGENCY, at,
                                               "gave_way"))
        elif score <= STAND_AT:
            outcome = "stand"
            need(queued or len(rest) < self.lim["queue"])
            self.grudge_room([ch])
            res["queue"] = rest
            self.enqueue(res, ch, urg, st, at, False)
            effects.append(self.add_grudge(ch, holder, GRUDGE_STAND + urg * GRUDGE_STAND_URGENCY, at, "refused"))
        else:
            outcome = "turns"
            need(queued or len(rest) < self.lim["queue"])
            res["queue"] = rest
            self.enqueue(res, ch, urg, st, at, True)
        return {"effects": effects, "holder": holder, "outcome": outcome, "resource": self.res_view(rid),
                "score": score}

    # ---------------------------------------------------------------- views

    def op_grudges(self, a: dict) -> dict:
        keys(a, ["being"], [])
        b = ident(a["being"])
        return {"being": b, "grudges": self.grudge_rows(b)}

    def op_assess(self, a: dict) -> dict:
        keys(a, ["being"], [])
        b = ident(a["being"])
        roles = [{"task": tid, "role": rid, "done": r["done"]}
                 for tid, t in sorted(self.tasks.items()) for rid, r in sorted(t["roles"].items()) if r["being"] == b]
        made = sorted(pid for pid, p in self.promises.items() if p["from"] == b and p["status"] == "pending")
        received = sorted(pid for pid, p in self.promises.items() if p["to"] == b and p["status"] == "pending")
        holds = sorted(rid for rid, r in self.resources.items() if r["holder"] == b)
        queued = [{"resource": rid, "position": self.position(r, b)}
                  for rid, r in sorted(self.resources.items()) if any(q["being"] == b for q in r["queue"])]
        owes = [{"to": c, "favors": n} for (d, c), n in sorted(self.debts.items()) if d == b]
        owed = [{"by": d, "favors": n} for (d, c), n in sorted(self.debts.items()) if c == b]
        return {"being": b, "grudges": self.grudge_rows(b), "holds": holds, "owed": owed, "owes": owes,
                "promises_made": made, "promises_received": received, "queued": queued, "roles": roles}

    def workflow(self, request: Any) -> dict:
        table = {"form": self.op_form, "assign": self.op_assign, "ask_help": self.op_ask_help,
                 "hand_off": self.op_hand_off, "complete": self.op_complete, "leave": self.op_leave,
                 "promise": self.op_promise, "keep": self.op_keep, "release": self.op_release, "tick": self.op_tick,
                 "claim": self.op_claim, "free": self.op_free, "contest": self.op_contest,
                 "grudges": self.op_grudges, "assess": self.op_assess}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Cooperation().workflow(request)
