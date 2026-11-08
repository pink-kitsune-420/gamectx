"""Pure-Python reference implementation of Theory of Mind, Deception Ledger, and Relationship Reconciliation.

Produces byte-identical JSON matching src/character/mind.rs, src/character/deception.rs,
and src/character/relationship.rs.
"""
from typing import Any, Dict, List, Optional, Tuple

from limits_reference import Limits, limited


def room(d: Dict[str, Any], key: str, mx: int) -> None:
    """Refuse a new key once a table holds `mx` entries."""
    if len(d) >= mx and key not in d:
        raise ValueError("over host limit")

ATTITUDE_KINDS = [
    "preference",
    "opinion",
    "familiarity",
    "trust",
    "affection",
    "resentment",
    "obligation",
    "disagreement",
    "belief",
    "report",
    "hypothesis",
    "misconception",
    "unknown",
    "goal",
    "commitment",
    "unfinished",
    "priority",
    "physical",
    "emotion",
    "fear",
    "respect",
    "gratitude",
    "envy",
    "admiration",
]

STANCES = ["allied", "friendly", "neutral", "wary", "hostile"]


def stance_from_trust(trust: int) -> str:
    if trust >= 50:
        return "allied"
    elif trust >= 20:
        return "friendly"
    elif trust >= -15:
        return "neutral"
    elif trust >= -50:
        return "wary"
    else:
        return "hostile"


def sheet_kind_baselines(kind: str) -> Tuple[int, int]:
    if kind == "friend":
        return (40, 40)
    elif kind == "mentor":
        return (50, 30)
    elif kind == "apprentice":
        return (30, 20)
    elif kind == "ally":
        return (35, 10)
    elif kind == "rival":
        return (-20, -10)
    elif kind == "enemy":
        return (-50, -50)
    else:
        return (0, 0)


def can_perceive(being: Dict[str, Any], entry: Dict[str, Any]) -> Tuple[bool, str]:
    being_id = being.get("id", "")
    being_place = being.get("place")
    senses = being.get("senses", {})
    conditions = senses.get("conditions", []) if isinstance(senses, dict) else []

    if "dead" in conditions:
        return (False, "dead")
    if "unconscious" in conditions:
        return (False, "unconscious")

    noise = entry.get("noise", 10)
    if "asleep" in conditions and noise < 80:
        return (False, "asleep_noise_too_low")

    vis = entry.get("visibility", "public")
    actors = entry.get("actors", [])
    witnesses = entry.get("witnesses", [])
    event_place = entry.get("place")

    if being_id in actors:
        return (True, "actor")

    if vis == "private":
        return (False, "private_not_actor")

    is_named_witness = being_id in witnesses
    in_same_place = being_place is not None and event_place is not None and being_place == event_place
    occluded = senses.get("occluded", False) if isinstance(senses, dict) else False
    sight = senses.get("sight", True) if isinstance(senses, dict) else True
    hearing = senses.get("hearing", True) if isinstance(senses, dict) else True

    if in_same_place:
        if not occluded and sight:
            return (True, "seen_in_place")
        if hearing and noise >= 15:
            return (True, "heard_in_place")
        if is_named_witness:
            return (True, "witness_in_place")
        return (False, "occluded_cannot_sense")

    if is_named_witness and not occluded and sight:
        return (True, "witness_line_of_sight")

    if vis == "public" and hearing and noise >= 60:
        return (True, "heard_at_distance")

    return (False, "different_place")


# --- Theory of Mind Reference ---

class MindReference:
    def __init__(self):
        # subject -> other -> {"knows": {}, "wants": {}, "feels": {}}
        self.beings: Dict[str, Dict[str, Dict[str, Any]]] = {}
        self.limits = Limits([("beings", 64, 1_000_000), ("others", 64, 4096), ("beliefs", 64, 4096), ("witnesses", 64, 4096)])

    def get_other(self, subject: str, other: str) -> Dict[str, Any]:
        room(self.beings, subject, self.limits["beings"])
        if subject not in self.beings:
            self.beings[subject] = {}
        room(self.beings[subject], other, self.limits["others"])
        if other not in self.beings[subject]:
            self.beings[subject][other] = {"knows": {}, "wants": {}, "feels": {}}
        return self.beings[subject][other]

    def run(self, request: Dict[str, Any]) -> Dict[str, Any]:
        return limited(request, self.limits, self.run_inner)

    def run_inner(self, request: Dict[str, Any]) -> Dict[str, Any]:
        ops = request.get("ops", [])
        if not isinstance(ops, list) or not ops or len(ops) > 64:
            raise ValueError("bad ops")

        if "beings" in request and isinstance(request["beings"], list):
            for b in request["beings"]:
                subj = b["subject"]
                for other_item in b.get("others", []):
                    o_id = other_item["other"]
                    model = self.get_other(subj, o_id)
                    for k in other_item.get("knows", []):
                        fact = k["fact"]
                        room(model["knows"], fact, self.limits["beliefs"])
                        model["knows"][fact] = {
                            "fact": fact,
                            "status": bool(k["status"]),
                            "source": k["source"],
                            "at": int(k.get("at", 0)),
                            "confidence": int(k.get("confidence", 100)),
                        }

        results = []
        for op_val in ops:
            op = op_val.get("op")
            args = op_val.get("args", {})
            if op == "init":
                subj = args["subject"]
                room(self.beings, subj, self.limits["beings"])
                self.beings[subj] = {}
                for other_item in args.get("others", []):
                    o_id = other_item["other"]
                    model = self.get_other(subj, o_id)
                    for k in other_item.get("knows", []):
                        fact = k["fact"]
                        room(model["knows"], fact, self.limits["beliefs"])
                        model["knows"][fact] = {
                            "fact": fact,
                            "status": bool(k["status"]),
                            "source": k["source"],
                            "at": int(k.get("at", 0)),
                            "confidence": int(k.get("confidence", 100)),
                        }
                results.append({"ok": True, "subject": subj})
            elif op == "assess":
                subj = args["subject"]
                if subj not in self.beings:
                    raise ValueError(f"unknown subject {subj}")
                target_other = args.get("other")
                category = args.get("category", "all")
                others_arr = []
                for o_id in sorted(self.beings[subj].keys()):
                    if target_other and o_id != target_other:
                        continue
                    model = self.beings[subj][o_id]
                    row: Dict[str, Any] = {"other": o_id}
                    if category in ("all", "feels"):
                        f_list = [model["feels"][k] for k in sorted(model["feels"].keys())]
                        row["feels"] = f_list
                    if category in ("all", "knows"):
                        k_list = [model["knows"][k] for k in sorted(model["knows"].keys())]
                        row["knows"] = k_list
                    if category in ("all", "wants"):
                        w_list = [model["wants"][k] for k in sorted(model["wants"].keys())]
                        row["wants"] = w_list
                    # Sort row keys
                    sorted_row = {k: row[k] for k in sorted(row.keys())}
                    others_arr.append(sorted_row)
                results.append({"others": others_arr, "subject": subj})
            elif op == "record":
                subj = args["subject"]
                other = args["other"]
                cat = args["category"]
                src = args.get("source", "inferred")
                at = int(args.get("at", 0))
                model = self.get_other(subj, other)
                if cat == "knows":
                    fact = args["fact"]
                    status = bool(args["status"])
                    conf = int(args.get("confidence", 100))
                    room(model["knows"], fact, self.limits["beliefs"])
                    model["knows"][fact] = {
                        "at": at,
                        "confidence": conf,
                        "fact": fact,
                        "source": src,
                        "status": status,
                    }
                    results.append({
                        "category": "knows",
                        "fact": fact,
                        "ok": True,
                        "other": other,
                        "source": src,
                        "status": status,
                        "subject": subj,
                    })
                elif cat == "wants":
                    goal = args["goal"]
                    prio = int(args["priority"])
                    room(model["wants"], goal, self.limits["beliefs"])
                    model["wants"][goal] = {
                        "at": at,
                        "goal": goal,
                        "priority": prio,
                        "source": src,
                    }
                    results.append({
                        "category": "wants",
                        "goal": goal,
                        "ok": True,
                        "other": other,
                        "priority": prio,
                        "source": src,
                        "subject": subj,
                    })
                elif cat == "feels":
                    emotion = args["emotion"]
                    if emotion not in ATTITUDE_KINDS:
                        raise ValueError(f"unknown emotion {emotion}")
                    lvl = int(args["level"])
                    tgt = args.get("target")
                    key = f"{emotion}:{tgt}" if tgt else emotion
                    feel_val: Dict[str, Any] = {
                        "at": at,
                        "emotion": emotion,
                        "level": lvl,
                        "source": src,
                    }
                    if tgt:
                        feel_val["target"] = tgt
                    # Sort feel_val keys
                    feel_val = {k: feel_val[k] for k in sorted(feel_val.keys())}
                    room(model["feels"], key, self.limits["beliefs"])
                    model["feels"][key] = feel_val
                    res_val: Dict[str, Any] = {
                        "category": "feels",
                        "emotion": emotion,
                        "level": lvl,
                        "ok": True,
                        "other": other,
                        "source": src,
                        "subject": subj,
                    }
                    if tgt:
                        res_val["target"] = tgt
                    res_val = {k: res_val[k] for k in sorted(res_val.keys())}
                    results.append(res_val)
                else:
                    raise ValueError(f"unknown category {cat}")
            elif op == "infer_from_witness":
                subj = args["subject"]
                entry = args["entry"]
                beings = args["beings"]
                default_fact = entry.get("kind") or entry.get("id") or "event"
                fact = args.get("fact", default_fact)
                at = int(entry.get("at", 0))
                if not isinstance(beings, list) or not beings or len(beings) > self.limits["witnesses"]:
                    raise ValueError("bad witnesses")
                room(self.beings, subj, self.limits["beings"])
                self.beings.setdefault(subj, {})

                inferences = []
                for b in beings:
                    b_id = b["id"]
                    if b_id == subj:
                        continue
                    perceived, reason = can_perceive(b, entry)
                    src = "seen" if perceived else "inferred"
                    conf = 100 if perceived else 90
                    model = self.get_other(subj, b_id)
                    room(model["knows"], fact, self.limits["beliefs"])
                    model["knows"][fact] = {
                        "at": at,
                        "confidence": conf,
                        "fact": fact,
                        "source": src,
                        "status": perceived,
                    }
                    inferences.append({
                        "confidence": conf,
                        "fact": fact,
                        "knows": perceived,
                        "other": b_id,
                        "reason": reason,
                        "source": src,
                    })
                inferences.sort(key=lambda x: x["other"])
                results.append({
                    "fact": fact,
                    "inferences": inferences,
                    "ok": True,
                    "subject": subj,
                })
            elif op == "query":
                subj = args["subject"]
                other = args["other"]
                cat = args["category"]
                key = args.get("key")
                if subj not in self.beings or other not in self.beings[subj]:
                    results.append({"found": False, "other": other, "subject": subj})
                    continue
                model = self.beings[subj][other]
                if cat == "knows":
                    if key and key in model["knows"]:
                        results.append({
                            "belief": model["knows"][key],
                            "found": True,
                            "other": other,
                            "subject": subj,
                        })
                    else:
                        results.append({"found": False, "other": other, "subject": subj})
                elif cat == "wants":
                    if key and key in model["wants"]:
                        results.append({
                            "belief": model["wants"][key],
                            "found": True,
                            "other": other,
                            "subject": subj,
                        })
                    else:
                        results.append({"found": False, "other": other, "subject": subj})
                elif cat == "feels":
                    if key and key in model["feels"]:
                        results.append({
                            "belief": model["feels"][key],
                            "found": True,
                            "other": other,
                            "subject": subj,
                        })
                    else:
                        results.append({"found": False, "other": other, "subject": subj})
                else:
                    raise ValueError(f"bad query category {cat}")
            else:
                raise ValueError(f"unknown mind op {op}")

        return {"results": results, "version": 1}


# --- Deception Ledger Reference ---

class DeceptionReference:
    def __init__(self):
        self.next_id = 1
        self.records: Dict[str, Dict[str, Any]] = {}
        self.limits = Limits([("records", 128, 1_000_000)])

    def run(self, request: Dict[str, Any]) -> Dict[str, Any]:
        return limited(request, self.limits, self.run_inner)

    def run_inner(self, request: Dict[str, Any]) -> Dict[str, Any]:
        ops = request.get("ops", [])
        if not isinstance(ops, list) or not ops or len(ops) > 64:
            raise ValueError("bad ops")

        if "records" in request and isinstance(request["records"], list):
            for r in request["records"]:
                r_id = r["id"]
                claim = r["claim"]
                true_val = r["true_value"]
                is_lie = r.get("is_lie", claim != true_val)
                room(self.records, r_id, self.limits["records"])
                self.records[r_id] = {
                    "at": int(r.get("at", 0)),
                    "claim": claim,
                    "id": r_id,
                    "is_lie": bool(is_lie),
                    "listener": r["listener"],
                    "speaker": r["speaker"],
                    "status": r.get("status", "active"),
                    "topic": r["topic"],
                    "true_value": true_val,
                }

        results = []
        for op_val in ops:
            op = op_val.get("op")
            args = op_val.get("args", {})
            if op == "init":
                self.records.clear()
                self.next_id = 1
                for r in args.get("records", []):
                    r_id = r.get("id") or f"dec_{self.next_id}"
                    if not r.get("id"):
                        self.next_id += 1
                    claim = r["claim"]
                    true_val = r.get("true_value", claim)
                    is_lie = r.get("is_lie", claim != true_val)
                    room(self.records, r_id, self.limits["records"])
                    self.records[r_id] = {
                        "at": int(r.get("at", 0)),
                        "claim": claim,
                        "id": r_id,
                        "is_lie": bool(is_lie),
                        "listener": r["listener"],
                        "speaker": r["speaker"],
                        "status": r.get("status", "active"),
                        "topic": r["topic"],
                        "true_value": true_val,
                    }
                results.append({"count": len(self.records), "ok": True})
            elif op == "assess":
                filter_speaker = args.get("speaker")
                filter_listener = args.get("listener")
                filter_topic = args.get("topic")
                filter_status = args.get("status")
                matched = []
                for r_id in sorted(self.records.keys()):
                    r = self.records[r_id]
                    if filter_speaker and r["speaker"] != filter_speaker:
                        continue
                    if filter_listener and r["listener"] != filter_listener:
                        continue
                    if filter_topic and r["topic"] != filter_topic:
                        continue
                    if filter_status and r["status"] != filter_status:
                        continue
                    # Ensure sorted keys
                    matched.append({k: r[k] for k in sorted(r.keys())})
                results.append({"records": matched})
            elif op == "record":
                speaker = args["speaker"]
                listener = args["listener"]
                if speaker == listener:
                    raise ValueError("speaker cannot be listener")
                topic = args["topic"]
                claim = args["claim"]
                true_val = args.get("true_value", claim)
                is_lie = args.get("is_lie", claim != true_val)
                at = int(args.get("at", 0))
                status = args.get("status", "active")
                if len(self.records) >= self.limits["records"]:
                    raise ValueError("too many records")
                r_id = args.get("id") or f"dec_{self.next_id}"
                if not args.get("id"):
                    self.next_id += 1
                rec = {
                    "at": at,
                    "claim": claim,
                    "id": r_id,
                    "is_lie": bool(is_lie),
                    "listener": listener,
                    "speaker": speaker,
                    "status": status,
                    "topic": topic,
                    "true_value": true_val,
                }
                self.records[r_id] = rec
                results.append({
                    "id": r_id,
                    "is_lie": bool(is_lie),
                    "listener": listener,
                    "ok": True,
                    "speaker": speaker,
                    "topic": topic,
                })
            elif op == "check_consistency":
                speaker = args["speaker"]
                listener = args["listener"]
                topic = args["topic"]
                proposed = args["proposed_claim"]
                priors = [
                    r for r in self.records.values()
                    if r["speaker"] == speaker and r["listener"] == listener and r["topic"] == topic and r["status"] == "active"
                ]
                if priors:
                    priors.sort(key=lambda x: x["at"])
                    prior = priors[-1]
                    consistent = prior["claim"] == proposed
                    out: Dict[str, Any] = {
                        "consistent": consistent,
                        "is_prior_lie": prior["is_lie"],
                        "listener": listener,
                        "ok": True,
                        "prior_claim": prior["claim"],
                        "speaker": speaker,
                        "topic": topic,
                    }
                    if not consistent:
                        out["warning"] = "contradicts_prior_claim_to_this_listener"
                    out = {k: out[k] for k in sorted(out.keys())}
                    results.append(out)
                else:
                    results.append({
                        "consistent": True,
                        "is_prior_lie": False,
                        "listener": listener,
                        "ok": True,
                        "prior_claim": None,
                        "speaker": speaker,
                        "topic": topic,
                    })
            elif op == "appraise_contradiction":
                listener = args["listener"]
                topic = args["topic"]
                observed = args["observed_truth"]
                contradictions = []
                effects = []
                for r_id in sorted(self.records.keys()):
                    r = self.records[r_id]
                    if r["listener"] == listener and r["topic"] == topic and r["status"] == "active" and r["claim"] != observed:
                        r["status"] = "contradicted"
                        contradictions.append({
                            "claim": r["claim"],
                            "id": r["id"],
                            "speaker": r["speaker"],
                            "topic": r["topic"],
                        })
                        effects.append({
                            "attitude": "trust",
                            "delta": -25,
                            "reason": f"contradicted_claim:{r['topic']}",
                            "subject": listener,
                            "target": r["speaker"],
                        })
                        effects.append({
                            "attitude": "resentment",
                            "delta": 15,
                            "reason": f"deception_revealed:{r['topic']}",
                            "subject": listener,
                            "target": r["speaker"],
                        })
                contradictions.sort(key=lambda x: x["id"])
                effects.sort(key=lambda x: (x["target"], x["attitude"]))
                found = len(contradictions) > 0
                results.append({
                    "contradiction_found": found,
                    "contradictions": contradictions,
                    "listener": listener,
                    "observed_truth": observed,
                    "relationship_effects": effects,
                    "topic": topic,
                })
            else:
                raise ValueError(f"unknown deception op {op}")

        return {"results": results, "version": 1}


# --- Relationship Reconciliation Reference ---

class RelationshipReference:
    def __init__(self):
        # subject -> target -> kind
        self.sheet_relationships: Dict[str, Dict[str, str]] = {}
        # (subject, target, kind) -> level
        self.personal_attitudes: Dict[Tuple[str, str, str], int] = {}
        # (comm1, comm2) -> trust
        self.community_trust: Dict[Tuple[str, str], int] = {}
        self.limits = Limits([("subjects", 4096, 1_000_000), ("relations", 64, 4096), ("attitudes", 4096, 1_000_000), ("community_pairs", 4096, 1_000_000)])

    def put_sheet(self, s: str, w: str, k: str) -> None:
        room(self.sheet_relationships, s, self.limits["subjects"])
        sheet = self.sheet_relationships.setdefault(s, {})
        room(sheet, w, self.limits["relations"])
        sheet[w] = k

    def put_attitude(self, key: Tuple[str, str, str], lvl: int) -> None:
        room(self.personal_attitudes, key, self.limits["attitudes"])
        self.personal_attitudes[key] = lvl

    def put_community(self, key: Tuple[str, str], tr: int) -> None:
        room(self.community_trust, key, self.limits["community_pairs"])
        self.community_trust[key] = tr

    def set_attitude(self, subject: str, target: str, kind: str, delta: int) -> int:
        key = (subject, target, kind)
        cur = self.personal_attitudes.get(key, 0)
        nxt = max(-100, min(100, cur + delta))
        self.personal_attitudes[key] = nxt
        return nxt

    def run(self, request: Dict[str, Any]) -> Dict[str, Any]:
        return limited(request, self.limits, self.run_inner)

    def run_inner(self, request: Dict[str, Any]) -> Dict[str, Any]:
        ops = request.get("ops", [])
        if not isinstance(ops, list) or not ops or len(ops) > 64:
            raise ValueError("bad ops")

        if "sheet_relationships" in request and isinstance(request["sheet_relationships"], list):
            for item in request["sheet_relationships"]:
                s = item["subject"]
                w = item["with"]
                k = item["kind"]
                self.put_sheet(s, w, k)

        if "attitudes" in request and isinstance(request["attitudes"], list):
            for item in request["attitudes"]:
                s = item["subject"]
                t = item["target"]
                k = item["kind"]
                lvl = int(item["level"])
                self.put_attitude((s, t, k), max(-100, min(100, lvl)))

        if "communities" in request and isinstance(request["communities"], list):
            for item in request["communities"]:
                c1 = item["community"]
                c2 = item["target"]
                tr = int(item["trust"])
                self.put_community((c1, c2), max(-100, min(100, tr)))

        results = []
        for op_val in ops:
            op = op_val.get("op")
            args = op_val.get("args", {})
            if op == "init":
                self.sheet_relationships.clear()
                self.personal_attitudes.clear()
                self.community_trust.clear()
                for item in args.get("sheet_relationships", []):
                    s = item["subject"]
                    w = item["with"]
                    k = item["kind"]
                    self.put_sheet(s, w, k)
                for item in args.get("attitudes", []):
                    s = item["subject"]
                    t = item["target"]
                    k = item["kind"]
                    lvl = int(item["level"])
                    self.put_attitude((s, t, k), max(-100, min(100, lvl)))
                for item in args.get("communities", []):
                    c1 = item["community"]
                    c2 = item["target"]
                    tr = int(item["trust"])
                    self.put_community((c1, c2), max(-100, min(100, tr)))
                results.append({"ok": True})
            elif op == "assess":
                subj = args["subject"]
                filter_target = args.get("target")
                att_list = []
                for (s, t, k), lvl in self.personal_attitudes.items():
                    if s != subj:
                        continue
                    if filter_target and t != filter_target:
                        continue
                    att_list.append({
                        "kind": k,
                        "level": lvl,
                        "target": t,
                    })
                att_list.sort(key=lambda x: (x["target"], x["kind"]))

                sheet_list = []
                if subj in self.sheet_relationships:
                    for w, k in self.sheet_relationships[subj].items():
                        if filter_target and w != filter_target:
                            continue
                        sheet_list.append({
                            "kind": k,
                            "with": w,
                        })
                sheet_list.sort(key=lambda x: x["with"])

                results.append({
                    "attitudes": att_list,
                    "sheet_relationships": sheet_list,
                    "subject": subj,
                })
            elif op == "resolve":
                subj = args["subject"]
                target = args["target"]

                personal_trust = self.personal_attitudes.get((subj, target, "trust"))
                personal_affection = self.personal_attitudes.get((subj, target, "affection"))

                sheet_kind = None
                if "sheet_relationship" in args and args["sheet_relationship"] is not None:
                    sr = args["sheet_relationship"]
                    if sr["with"] != target:
                        raise ValueError("target mismatch in sheet_relationship")
                    sheet_kind = sr["kind"]
                elif subj in self.sheet_relationships and target in self.sheet_relationships[subj]:
                    sheet_kind = self.sheet_relationships[subj][target]

                sheet_trust, sheet_affection = sheet_kind_baselines(sheet_kind) if sheet_kind else (0, 0)

                base_trust = personal_trust if personal_trust is not None else sheet_trust
                base_affection = personal_affection if personal_affection is not None else sheet_affection

                community_mod = 0
                comm1 = args.get("community")
                comm2 = args.get("target_community")
                if comm1 and comm2 and (comm1, comm2) in self.community_trust:
                    ct = self.community_trust[(comm1, comm2)]
                    # Truncation toward zero: int(ct / 4)
                    raw_mod = int(ct / 4)
                    if abs(base_trust) >= 50:
                        community_mod = int(raw_mod / 2)
                    else:
                        community_mod = raw_mod

                eff_trust = max(-100, min(100, base_trust + community_mod))
                eff_affection = max(-100, min(100, base_affection))
                dominant_stance = stance_from_trust(eff_trust)

                vp = args.get("viewpoint")

                if personal_trust is not None:
                    if community_mod != 0:
                        reason = f"personal_attitude:trust({base_trust}) + community_bias({community_mod})"
                    else:
                        reason = f"personal_attitude:trust({base_trust})"
                elif sheet_kind is not None:
                    if community_mod != 0:
                        reason = f"sheet_relation:{sheet_kind}({base_trust}) + community_bias({community_mod})"
                    else:
                        reason = f"sheet_relation:{sheet_kind}({base_trust})"
                elif community_mod != 0:
                    reason = f"community_bias({community_mod})"
                else:
                    reason = "default_neutral"

                out = {
                    "dominant_stance": dominant_stance,
                    "effective_affection": eff_affection,
                    "effective_trust": eff_trust,
                    "precedence_reason": reason,
                    "subject": subj,
                    "target": target,
                }
                if sheet_kind:
                    out["sheet_kind"] = sheet_kind
                if vp:
                    out["viewpoint_stance"] = vp
                out = {k: out[k] for k in sorted(out.keys())}
                results.append(out)
            elif op == "appraise_action":
                actor = args["actor"]
                target = args["target"]
                action = args["action"]
                success = args.get("success", True)

                deltas: List[Tuple[str, int, str]] = []
                if action == "barter":
                    if success:
                        deltas.append(("trust", 5, "successful_trade"))
                        deltas.append(("gratitude", 5, "fair_exchange"))
                    else:
                        deltas.append(("trust", -10, "failed_barter"))
                        deltas.append(("resentment", 5, "unfair_terms"))
                elif action == "broken_promise":
                    deltas.append(("trust", -20, "broken_promise"))
                    deltas.append(("resentment", 25, "neglected_obligation"))
                elif action == "fulfilled_promise":
                    deltas.append(("trust", 25, "kept_promise"))
                    deltas.append(("respect", 15, "reliable_partner"))
                    deltas.append(("gratitude", 10, "honored_commitment"))
                elif action == "attack":
                    deltas.append(("trust", -60, "physical_aggression"))
                    deltas.append(("resentment", 60, "violent_attack"))
                    deltas.append(("fear", 30, "threat_to_safety"))
                elif action == "help":
                    deltas.append(("trust", 20, "received_assistance"))
                    deltas.append(("gratitude", 25, "helpful_deed"))
                    deltas.append(("affection", 15, "kindness"))
                elif action == "gift":
                    deltas.append(("gratitude", 15, "received_gift"))
                    deltas.append(("affection", 15, "generosity"))
                elif action == "teach":
                    deltas.append(("respect", 15, "learned_skill"))
                    deltas.append(("gratitude", 15, "guided_instruction"))
                elif action == "deception_exposed":
                    deltas.append(("trust", -25, "caught_in_lie"))
                    deltas.append(("resentment", 15, "dishonesty"))
                else:
                    if not success:
                        deltas.append(("trust", -5, "action_failure"))

                applied = []
                for kind, delta, d_reason in deltas:
                    room(self.personal_attitudes, (target, actor, kind), self.limits["attitudes"])
                    nxt = self.set_attitude(target, actor, kind, delta)
                    applied.append({
                        "attitude": kind,
                        "delta": delta,
                        "new_level": nxt,
                        "reason": d_reason,
                        "subject": target,
                        "target": actor,
                    })
                applied.sort(key=lambda x: x["attitude"])
                results.append({
                    "action": action,
                    "actor": actor,
                    "applied_effects": applied,
                    "ok": True,
                    "target": target,
                })
            elif op == "aggregate_community_trust":
                comm1 = args["community"]
                comm2 = args["target_community"]
                members = args["members"]
                tgt_members = args["target_members"]

                sum_tr = 0
                cnt = 0
                for m1 in members:
                    for m2 in tgt_members:
                        tr = self.personal_attitudes.get((m1, m2, "trust"), 0)
                        sum_tr += tr
                        cnt += 1

                avg_tr = int(sum_tr / cnt) if cnt > 0 else 0
                avg_tr = max(-100, min(100, avg_tr))
                self.put_community((comm1, comm2), avg_tr)
                results.append({
                    "community": comm1,
                    "derived_stance": stance_from_trust(avg_tr),
                    "ok": True,
                    "target_community": comm2,
                    "trust": avg_tr,
                })
            else:
                raise ValueError(f"unknown relationship op {op}")

        return {"results": results, "version": 1}


def process_local(kind: str, args: Dict[str, Any]) -> Dict[str, Any]:
    if kind == "mind":
        mgr = MindReference()
        return mgr.run(args)
    elif kind == "deception":
        mgr = DeceptionReference()
        return mgr.run(args)
    elif kind == "relationship":
        mgr = RelationshipReference()
        return mgr.run(args)
    else:
        raise ValueError(f"unknown local kind {kind}")
