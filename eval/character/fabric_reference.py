"""Independent original reference for graded norms with sanction ladders, social contracts and
favor balance, dyad memory, and attention and pragmatic profiles (pass 3, stage 5, social
schemas). Python standard library only; nothing here calls a model.

Norms: `norm` declares a severity (`etiquette` 10, `custom` 30, `contract` 60, `taboo` 80, `crime`
100), the situations it applies in, a guilt cost and the role a severe breach is reported to.
`breach` climbs a sanction ladder per norm, offender and witness: a minor first breach earns a
verbal warning, a repeat or a contract breach refused service and a demand for apology or fine, a
taboo or crime (or a third repeat) flight or confrontation and a report. Etiquette never goes past
the second step. An offender with a conscience of 50 or more feels the guilt cost and wants to make
amends. `apologize` steps the ladder back once.

Contracts: `contract` records a promise, loan, trade order, kept secret, alibi pact, employment,
owed apology or favor between a debtor and a creditor, with witnesses, a due time, terms and a
weight; `tick` marks overdue ones; `settle` closes one as fulfilled (trust up a quarter of the
weight), released, or broken (trust down by the weight; the witnesses learn of it); `ledger` sums
what two beings owe each other and how much more readily each would help the other.

Dyads: `meet` records how two beings first met; `landmark` keeps up to four defining moments (the
same kind replaces, the oldest drops); `address` sets how one addresses the other; `thread` and
`resolve` keep their unfinished business; `dyad` shows it all (strangers by default).

Profiles: `profile` sets a being's attention (bandwidth, vigilance, per-category salience biases)
and pragmatic style (conflict style, baseline cooperativeness, interrupt tolerance, small-talk
patience, secret keeping, lying tells, coping style). `notice` ranks stimuli through the attention
profile; `tells` says which lying tells an observer of given insight perceives; `style` turns the
pragmatic profile into reactions to interruption and small talk.
"""
import pathlib
import sys
from typing import Any, Dict, List, Optional, Tuple

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_bool, opt_uint, text, uint
from growth_reference import run_limited
from limits_reference import Limits

MAX_AT = 1_000_000_000_000
SPEC = [("beings", 1024, 1_000_000), ("norms", 64, 4096), ("breaches", 1024, 1_000_000),
        ("contracts", 256, 1_000_000), ("dyads", 1024, 1_000_000), ("threads", 4, 32)]
SEVERITY = {"etiquette": 10, "custom": 30, "contract": 60, "taboo": 80, "crime": 100}
KINDS = ["alibi_pact", "apology_owed", "employment", "favor", "keep_secret", "loan", "promise", "trade_order"]
OUTCOMES = ["fulfilled", "released", "broken"]
STYLES = ["direct", "avoidant", "diplomatic", "passive_aggressive"]
COOP = ["eager", "cooperative", "guarded", "reluctant", "deflecting", "hostile_refusal"]
TELL_KINDS = ["gaze", "speech", "posture", "hands"]
LANDMARKS = 4
STIMULI = 64


def opt_ident(d: dict, k: str) -> Optional[str]:
    return None if d.get(k) is None else ident(d[k])


def one_of(d: dict, k: str, options: List[str], default: str) -> str:
    v = opt_ident(d, k)
    if v is None:
        return default
    need(v in options)
    return v


def id_list(v: Any, hi: int) -> List[str]:
    v = [] if v is None else v
    need(isinstance(v, list) and len(v) <= hi)
    out = [ident(x) for x in v]
    need(len(set(out)) == len(out))
    return out


def pair(a: str, b: str) -> Tuple[str, str]:
    need(a != b)
    return (a, b) if a < b else (b, a)


ATTENTION = {"bandwidth": 4, "vigilance": 50, "biases": {}}
PRAGMATIC = {"conflict_style": "direct", "cooperativeness": "cooperative", "interrupt_tolerance": 50,
             "small_talk_patience": 50, "secret_keeping": 50, "tells": [], "coping": None}


class Fabric:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.clock = 0
        self.norms: Dict[str, dict] = {}
        self.counts: Dict[Tuple[str, str, str], int] = {}
        self.contracts: Dict[str, dict] = {}
        self.dyads: Dict[Tuple[str, str], dict] = {}
        self.profiles: Dict[str, dict] = {}

    def op_tick(self, a: dict) -> dict:
        keys(a, ["now"], [])
        now = uint(a["now"], 0, MAX_AT)
        need(now >= self.clock)
        self.clock = now
        late = sorted(i for i, c in self.contracts.items()
                      if c["status"] == "open" and c["due_at"] is not None and c["due_at"] < now)
        for i in late:
            self.contracts[i]["status"] = "overdue"
        return {"now": now, "overdue": late}

    # ---------------------------------------------------------------- norms

    def op_norm(self, a: dict) -> dict:
        keys(a, ["id", "severity"], ["applies_when", "guilt_cost", "report_to"])
        nid = ident(a["id"])
        sev = a["severity"]
        need(isinstance(sev, str) and sev in SEVERITY)
        n = {"severity": SEVERITY[sev], "applies_when": sorted(id_list(a.get("applies_when"), 8)),
             "guilt_cost": opt_uint(a, "guilt_cost", 0, 100, SEVERITY[sev] // 2), "report_to": opt_ident(a, "report_to")}
        need(nid in self.norms or len(self.norms) < self.lim["norms"])
        self.norms[nid] = n
        return {"id": nid, "severity": n["severity"]}

    def op_breach(self, a: dict) -> dict:
        keys(a, ["norm", "offender", "witness"], ["context", "conscience"])
        nid, off, wit = ident(a["norm"]), ident(a["offender"]), ident(a["witness"])
        ctx = id_list(a.get("context"), 8)
        conscience = opt_uint(a, "conscience", 0, 100, 0)
        need(nid in self.norms and off != wit)
        n = self.norms[nid]
        key = (nid, off, wit)
        prior = self.counts.get(key, 0)
        out = {"amends": False, "applies": False, "deltas": {"disagreement": 0, "resentment": 0, "trust": 0},
               "guilt": 0, "norm": nid, "offender": off, "prior": prior, "report_to": None, "sanctions": [], "step": 0,
               "witness": wit}
        if n["applies_when"] and not set(ctx) & set(n["applies_when"]):
            return out
        need(key in self.counts or len(self.counts) < self.lim["breaches"])
        sev = n["severity"]
        step = min(3, (3 if sev >= 80 else 2 if sev >= 60 else 1) + prior)
        if sev <= 10:
            step = min(step, 2)
        if step == 1:
            sanctions, deltas = ["verbal_warning"], {"disagreement": 10, "resentment": 0, "trust": 0}
        elif step == 2:
            sanctions, deltas = ["refuse_service", "demand_apology_or_fine"], {"disagreement": 0, "resentment": 0,
                                                                               "trust": -20}
        else:
            sanctions = ["flee_or_confront"] + (["report_to_role"] if n["report_to"] is not None else [])
            deltas = {"disagreement": 0, "resentment": 50, "trust": 0}
        guilt = n["guilt_cost"] if conscience >= 50 else 0
        self.counts[key] = prior + 1
        out.update(amends=guilt > 0, applies=True, deltas=deltas, guilt=guilt,
                   report_to=n["report_to"] if step == 3 else None, sanctions=sanctions, step=step)
        return out

    def op_apologize(self, a: dict) -> dict:
        keys(a, ["norm", "offender", "witness"], [])
        key = (ident(a["norm"]), ident(a["offender"]), ident(a["witness"]))
        need(self.counts.get(key, 0) > 0)
        self.counts[key] -= 1
        if self.counts[key] == 0:
            del self.counts[key]
        return {"norm": key[0], "offender": key[1], "prior": self.counts.get(key, 0), "witness": key[2]}

    # ---------------------------------------------------------------- contracts

    def op_contract(self, a: dict) -> dict:
        keys(a, ["id", "kind", "debtor", "creditor", "terms"], ["witnesses", "due_at", "weight"])
        cid = ident(a["id"])
        kind = a["kind"]
        need(isinstance(kind, str) and kind in KINDS)
        deb, cred = ident(a["debtor"]), ident(a["creditor"])
        wits = sorted(id_list(a.get("witnesses"), 8))
        need(deb != cred and deb not in wits and cred not in wits)
        due = None if a.get("due_at") is None else uint(a["due_at"], self.clock, MAX_AT)
        c = {"kind": kind, "debtor": deb, "creditor": cred, "witnesses": wits, "due_at": due,
             "terms": text(a["terms"], 160), "weight": opt_uint(a, "weight", 1, 100, 20), "status": "open"}
        need(cid not in self.contracts and len(self.contracts) < self.lim["contracts"])
        self.contracts[cid] = c
        return {"id": cid, "status": "open"}

    def op_settle(self, a: dict) -> dict:
        keys(a, ["id", "outcome"], [])
        cid = ident(a["id"])
        res = a["outcome"]
        need(cid in self.contracts and isinstance(res, str) and res in OUTCOMES)
        c = self.contracts.pop(cid)
        delta = c["weight"] // 4 if res == "fulfilled" else -c["weight"] if res == "broken" else 0
        return {"creditor": c["creditor"], "debtor": c["debtor"], "id": cid, "late": c["status"] == "overdue",
                "status": res, "told": c["witnesses"] if res == "broken" else [], "trust_delta": delta}

    def op_ledger(self, a: dict) -> dict:
        keys(a, ["a", "b"], [])
        x, y = ident(a["a"]), ident(a["b"])
        need(x != y)
        xy = yx = 0
        open_, late = [], []
        for cid in sorted(self.contracts):
            c = self.contracts[cid]
            if {c["debtor"], c["creditor"]} != {x, y}:
                continue
            if c["debtor"] == x:
                xy += c["weight"]
            else:
                yx += c["weight"]
            (late if c["status"] == "overdue" else open_).append(cid)
        return {"a": x, "a_owes_b": xy, "b": y, "b_owes_a": yx,
                "help_bias": {"a_asks_b": min(50, yx), "b_asks_a": min(50, xy)}, "open": open_, "overdue": late}

    # ---------------------------------------------------------------- dyads

    def op_meet(self, a: dict) -> dict:
        keys(a, ["a", "b", "episode", "summary"], [])
        k = pair(ident(a["a"]), ident(a["b"]))
        first = {"at": self.clock, "episode": ident(a["episode"]), "summary": text(a["summary"], 160)}
        if k in self.dyads:
            return {"first_encounter": self.dyads[k]["first"], "new": False}
        need(len(self.dyads) < self.lim["dyads"])
        self.dyads[k] = {"first": first, "landmarks": [], "address": {}, "threads": []}
        return {"first_encounter": first, "new": True}

    def dyad_of(self, a: dict) -> dict:
        k = pair(ident(a["a"]), ident(a["b"]))
        need(k in self.dyads)
        return self.dyads[k]

    def op_landmark(self, a: dict) -> dict:
        keys(a, ["a", "b", "kind", "episode"], ["summary"])
        d = self.dyad_of(a)
        kind, ep = ident(a["kind"]), ident(a["episode"])
        summ = None if a.get("summary") is None else text(a["summary"], 160)
        marks = [m for m in d["landmarks"] if m["kind"] != kind]
        evicted = None
        if len(marks) == len(d["landmarks"]) and len(marks) >= LANDMARKS:
            evicted = marks.pop(0)["kind"]
        marks.append({"at": self.clock, "episode": ep, "kind": kind, "summary": summ})
        d["landmarks"] = marks
        return {"evicted": evicted, "landmarks": [m["kind"] for m in marks]}

    def op_address(self, a: dict) -> dict:
        keys(a, ["from", "to", "term"], [])
        f, t = ident(a["from"]), ident(a["to"])
        d = self.dyad_of({"a": f, "b": t})
        term = text(a["term"], 40)
        d["address"][f] = term
        return {"from": f, "term": term, "to": t}

    def op_thread(self, a: dict) -> dict:
        keys(a, ["a", "b", "id", "text", "opened_by"], [])
        d = self.dyad_of(a)
        tid, body, by = ident(a["id"]), text(a["text"], 160), ident(a["opened_by"])
        need(by in (a["a"], a["b"]) and all(t["id"] != tid for t in d["threads"]))
        need(len(d["threads"]) < self.lim["threads"])
        d["threads"].append({"id": tid, "opened_by": by, "text": body})
        return {"id": tid, "open": len(d["threads"])}

    def op_resolve(self, a: dict) -> dict:
        keys(a, ["a", "b", "id"], [])
        d = self.dyad_of(a)
        tid = ident(a["id"])
        need(any(t["id"] == tid for t in d["threads"]))
        d["threads"] = [t for t in d["threads"] if t["id"] != tid]
        return {"id": tid, "open": len(d["threads"])}

    def op_dyad(self, a: dict) -> dict:
        keys(a, ["a", "b"], [])
        x, y = ident(a["a"]), ident(a["b"])
        d = self.dyads.get(pair(x, y))
        if d is None:
            return {"a": x, "address_a_to_b": "stranger", "address_b_to_a": "stranger", "b": y,
                    "first_encounter": None, "known_for": 0, "landmarks": [], "met": False, "threads": []}
        return {"a": x, "address_a_to_b": d["address"].get(x, "acquaintance"),
                "address_b_to_a": d["address"].get(y, "acquaintance"), "b": y, "first_encounter": d["first"],
                "known_for": self.clock - d["first"]["at"], "landmarks": list(d["landmarks"]), "met": True,
                "threads": list(d["threads"])}

    # ---------------------------------------------------------------- profiles

    def op_profile(self, a: dict) -> dict:
        keys(a, ["being"], ["attention", "pragmatic"])
        b = ident(a["being"])
        att = dict(ATTENTION)
        if a.get("attention") is not None:
            x = a["attention"]
            keys(x, [], ["bandwidth", "vigilance", "biases"])
            bias = x.get("biases")
            bias = {} if bias is None else bias
            need(isinstance(bias, dict) and len(bias) <= 16)
            att = {"bandwidth": opt_uint(x, "bandwidth", 1, 16, 4), "vigilance": opt_uint(x, "vigilance", 0, 100, 50),
                   "biases": {ident(k): uint(v, 0, 300) for k, v in bias.items()}}
        prag = dict(PRAGMATIC)
        if a.get("pragmatic") is not None:
            x = a["pragmatic"]
            keys(x, [], ["conflict_style", "cooperativeness", "interrupt_tolerance", "small_talk_patience",
                         "secret_keeping", "tells", "coping"])
            rows = x.get("tells")
            rows = [] if rows is None else rows
            need(isinstance(rows, list) and len(rows) <= 8)
            tells = []
            for t in rows:
                keys(t, ["kind", "cue"], ["min_insight"])
                k = t["kind"]
                need(isinstance(k, str) and k in TELL_KINDS)
                tells.append({"cue": ident(t["cue"]), "kind": k, "min_insight": opt_uint(t, "min_insight", 0, 100, 50)})
            prag = {"conflict_style": one_of(x, "conflict_style", STYLES, "direct"),
                    "cooperativeness": one_of(x, "cooperativeness", COOP, "cooperative"),
                    "interrupt_tolerance": opt_uint(x, "interrupt_tolerance", 0, 100, 50),
                    "small_talk_patience": opt_uint(x, "small_talk_patience", 0, 100, 50),
                    "secret_keeping": opt_uint(x, "secret_keeping", 0, 100, 50), "tells": tells,
                    "coping": opt_ident(x, "coping")}
        need(b in self.profiles or len(self.profiles) < self.lim["beings"])
        self.profiles[b] = {"attention": att, "pragmatic": prag}
        return {"being": b, "tells": len(prag["tells"])}

    def op_notice(self, a: dict) -> dict:
        keys(a, ["being", "stimuli"], [])
        b = ident(a["being"])
        att = self.profiles[b]["attention"] if b in self.profiles else ATTENTION
        rows = a["stimuli"]
        need(isinstance(rows, list) and len(rows) <= STIMULI)
        seen, scored = set(), []
        for x in rows:
            keys(x, ["id", "category", "intensity"], [])
            sid, cat = ident(x["id"]), ident(x["category"])
            need(sid not in seen)
            seen.add(sid)
            scored.append((uint(x["intensity"], 0, 100) * att["biases"].get(cat, 100) // 100, sid, cat))
        floor = 100 - att["vigilance"]
        ok = sorted((s for s in scored if s[0] >= floor), key=lambda s: (-s[0], s[1]))
        noticed = ok[:att["bandwidth"]]
        picked = {s[1] for s in noticed}
        return {"being": b, "missed": sorted(s[1] for s in scored if s[1] not in picked),
                "noticed": [{"category": c, "id": i, "score": s} for s, i, c in noticed]}

    def op_tells(self, a: dict) -> dict:
        keys(a, ["being", "observer_insight"], ["lying"])
        b = ident(a["being"])
        insight = uint(a["observer_insight"], 0, 100)
        lying = opt_bool(a, "lying", True)
        p = self.profiles[b]["pragmatic"] if b in self.profiles else PRAGMATIC
        bar = p["secret_keeping"] // 5
        seen = [{"cue": t["cue"], "kind": t["kind"]} for t in p["tells"] if lying and insight >= t["min_insight"] + bar]
        return {"being": b, "hidden": len(p["tells"]) - len(seen) if lying else 0, "perceived": seen,
                "suspicion": min(100, 15 * len(seen))}

    def op_style(self, a: dict) -> dict:
        keys(a, ["being"], [])
        b = ident(a["being"])
        p = self.profiles[b]["pragmatic"] if b in self.profiles else PRAGMATIC
        tol, pat = p["interrupt_tolerance"], p["small_talk_patience"]
        return {"being": b, "conflict_style": p["conflict_style"], "cooperativeness": p["cooperativeness"],
                "coping": p["coping"],
                "on_interrupt": "snaps" if tol < 30 else "notes_it" if tol < 70 else "lets_it_go",
                "on_small_talk": "cuts_to_point" if pat < 30 else "humors_briefly" if pat < 70 else "enjoys",
                "secret_keeping": p["secret_keeping"]}

    def workflow(self, request: Any) -> dict:
        table = {"tick": self.op_tick, "norm": self.op_norm, "breach": self.op_breach,
                 "apologize": self.op_apologize, "contract": self.op_contract, "settle": self.op_settle,
                 "ledger": self.op_ledger, "meet": self.op_meet, "landmark": self.op_landmark,
                 "address": self.op_address, "thread": self.op_thread, "resolve": self.op_resolve,
                 "dyad": self.op_dyad, "profile": self.op_profile, "notice": self.op_notice,
                 "tells": self.op_tells, "style": self.op_style}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def workflow(request: Any) -> dict:
    return Fabric().workflow(request)
