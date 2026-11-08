"""Pure-Python reference implementation of Off-screen Life Resolution (Gap 10)
and Curiosity & Exploration Model (Gap 11).

Produces byte-identical JSON matching src/character/offscreen.rs and src/character/curiosity.rs.
"""
from typing import Any, Dict, List, Optional, Set

from limits_reference import Limits, limited

TEMPERAMENTS = ["curious", "cautious", "apathetic", "calm"]
BASE_SCORE_PERSON = 40
BASE_SCORE_PLACE = 35
BASE_SCORE_THING = 30
BASE_SCORE_FACT = 25


def scale_score(temperament: str, base: int) -> int:
    if temperament == "curious":
        num, den = 15, 10
    elif temperament == "cautious":
        num, den = 7, 10
    elif temperament == "apathetic":
        num, den = 4, 10
    else:
        num, den = 10, 10
    return (base * num) // den


class OffscreenReference:
    def __init__(self):
        self.next_token = 1000
        self.records: Dict[str, Dict[str, Any]] = {}
        self.limits = Limits([("records", 128, 1_000_000), ("work_per_tick", 16, 4096)])

    def run(self, request: Dict[str, Any]) -> Dict[str, Any]:
        return limited(request, self.limits, self.run_inner)

    def run_inner(self, request: Dict[str, Any]) -> Dict[str, Any]:
        if "ops" not in request:
            raise ValueError("missing ops")
        ops = request["ops"]
        if not isinstance(ops, list) or len(ops) == 0 or len(ops) > 64:
            raise ValueError("invalid ops length")

        results = []
        for item in ops:
            op = item.get("op")
            args = item.get("args", {})
            if op == "init":
                clear = args.get("clear_records", False)
                if clear is True:
                    self.records.clear()
                results.append({"ok": True})
            elif op == "resolve_window":
                res = self._resolve_window(args)
                results.append(res)
            elif op == "batch_resolve":
                activities = args.get("activities", [])
                if not isinstance(activities, list) or len(activities) > self.limits["work_per_tick"]:
                    raise ValueError("invalid activities")
                batch_out = []
                for a in activities:
                    batch_out.append(self._resolve_window(a))
                results.append({
                    "count": len(batch_out),
                    "ok": True,
                    "resolved": batch_out,
                })
            elif op == "assess":
                filter_b = args.get("being")
                matched = []
                for rec_id in sorted(self.records.keys()):
                    rec = self.records[rec_id]
                    if filter_b is not None and rec["being"] != filter_b:
                        continue
                    matched.append(rec)
                results.append({"records": matched})
            else:
                raise ValueError(f"unknown offscreen op {op}")

        return {"results": results, "version": 1}

    def _resolve_window(self, args: Dict[str, Any]) -> Dict[str, Any]:
        being = args["being"]
        activity = args["activity"]
        action = args["action"]
        observed = args["observed"]

        if observed:
            return {
                "being": being,
                "reason": "observed_on_screen",
                "resolved": False,
            }

        at = args.get("at")
        at = 0 if at is None else int(at)
        place = args.get("place")
        place = "unknown" if place is None else str(place)
        visibility = args.get("visibility")
        visibility = "private" if visibility is None else str(visibility)
        if visibility not in ["private", "witnessed"]:
            raise ValueError(f"invalid visibility {visibility}")

        fatigue = args.get("fatigue")
        fatigue = 5 if fatigue is None else int(fatigue)
        if fatigue < -100 or fatigue > 100:
            raise ValueError("invalid fatigue")

        practice = args.get("practice")
        practice = 10 if practice is None else int(practice)
        if practice < 0 or practice > 100:
            raise ValueError("invalid practice")

        consumed = args.get("inputs")
        consumed = [] if consumed is None else list(consumed)
        produced = args.get("outputs")
        produced = [] if produced is None else list(produced)

        token = self.next_token
        self.next_token += 1

        summary = f"{being} completed off-screen {activity}."
        timeline_entry = {
            "actors": [being],
            "at": at,
            "kind": "offscreen_activity",
            "noise": 10,
            "place": place,
            "summary": summary,
            "visibility": visibility,
        }

        exp_text = f"Completed {activity} off-screen."
        life_experience = {
            "at": at,
            "audience": [being],
            "causes": [],
            "kind": "direct",
            "retain": True,
            "source": {
                "id": f"off_{token}",
                "speaker": being,
                "text": exp_text,
            },
            "subject": being,
            "tag": activity,
            "text": exp_text,
            "token": token,
        }

        rec_id = f"res_{token}"
        record = {
            "action": action,
            "activity": activity,
            "at": at,
            "being": being,
            "consumed": consumed,
            "fatigue_delta": fatigue,
            "id": rec_id,
            "life_experience": life_experience,
            "practice_gained": practice,
            "produced": produced,
            "timeline_entry": timeline_entry,
        }

        if len(self.records) >= self.limits["records"]:
            oldest_key = next(iter(sorted(self.records.keys())))
            del self.records[oldest_key]
        self.records[rec_id] = record

        return {
            "action": action,
            "activity": activity,
            "being": being,
            "consumed": consumed,
            "fatigue_delta": fatigue,
            "id": rec_id,
            "life_experience": life_experience,
            "practice_gained": practice,
            "produced": produced,
            "resolved": True,
            "timeline_entry": timeline_entry,
        }


class BeingCuriosityRef:
    def __init__(self, being: str):
        self.being = being
        self.temperament = "calm"
        self.known_people: Set[str] = set()
        self.known_things: Set[str] = set()
        self.known_places: Set[str] = set()
        self.known_facts: Set[str] = set()

    def to_dict(self) -> Dict[str, Any]:
        return {
            "being": self.being,
            "known_facts": sorted(list(self.known_facts)),
            "known_people": sorted(list(self.known_people)),
            "known_places": sorted(list(self.known_places)),
            "known_things": sorted(list(self.known_things)),
            "temperament": self.temperament,
        }


def bounded_list(args: Dict[str, Any], key: str, mx: int) -> List[str]:
    """A string list of at most `mx` items, sorted and de-duplicated (None or absent: empty)."""
    raw = args.get(key)
    if raw is None:
        return []
    if not isinstance(raw, list) or len(raw) > mx:
        raise ValueError(f"bad {key}")
    return sorted(set(str(s) for s in raw))


class CuriosityReference:
    def __init__(self):
        self.beings: Dict[str, BeingCuriosityRef] = {}
        self.limits = Limits([("beings", 64, 1_000_000), ("known", 256, 65536), ("perception_items", 32, 4096)])

    def get_or_create(self, being: str) -> BeingCuriosityRef:
        if being not in self.beings:
            if len(self.beings) >= self.limits["beings"]:
                raise ValueError("max beings exceeded")
            self.beings[being] = BeingCuriosityRef(being)
        return self.beings[being]

    def run(self, request: Dict[str, Any]) -> Dict[str, Any]:
        return limited(request, self.limits, self.run_inner)

    def run_inner(self, request: Dict[str, Any]) -> Dict[str, Any]:
        if "ops" not in request:
            raise ValueError("missing ops")
        ops = request["ops"]
        if not isinstance(ops, list) or len(ops) == 0 or len(ops) > 64:
            raise ValueError("invalid ops length")

        results = []
        for item in ops:
            op = item.get("op")
            args = item.get("args", {})
            being = args.get("being")
            if not being or not isinstance(being, str):
                raise ValueError("missing being")

            if op == "init":
                temp = args.get("temperament")
                if temp is not None and temp not in TEMPERAMENTS:
                    raise ValueError(f"invalid temperament {temp}")
                lists = {k: bounded_list(args, k, self.limits["known"]) for k in ("known_people", "known_things", "known_places", "known_facts")}
                if being not in self.beings and len(self.beings) >= self.limits["beings"]:
                    raise ValueError("max beings exceeded")
                cur = BeingCuriosityRef(being)
                if temp is not None:
                    cur.temperament = temp
                cur.known_people, cur.known_things = set(lists["known_people"]), set(lists["known_things"])
                cur.known_places, cur.known_facts = set(lists["known_places"]), set(lists["known_facts"])
                self.beings[being] = cur

                results.append({"being": being, "ok": True})

            elif op == "evaluate_novelty":
                cur = self.get_or_create(being)
                mx = self.limits["perception_items"]
                peop = bounded_list(args, "people", mx)
                thin = bounded_list(args, "things", mx)
                plac = bounded_list(args, "places", mx)
                fact = bounded_list(args, "facts", mx)

                novel_people = [p for p in peop if p not in cur.known_people]
                novel_things = [t for t in thin if t not in cur.known_things]
                novel_places = [pl for pl in plac if pl not in cur.known_places]
                novel_facts = [f for f in fact if f not in cur.known_facts]

                score = 0
                for _ in novel_people:
                    score += scale_score(cur.temperament, BASE_SCORE_PERSON)
                for _ in novel_things:
                    score += scale_score(cur.temperament, BASE_SCORE_THING)
                for _ in novel_places:
                    score += scale_score(cur.temperament, BASE_SCORE_PLACE)
                for _ in novel_facts:
                    score += scale_score(cur.temperament, BASE_SCORE_FACT)

                results.append({
                    "being": cur.being,
                    "novel_facts": novel_facts,
                    "novel_people": novel_people,
                    "novel_places": novel_places,
                    "novel_things": novel_things,
                    "novelty_score": score,
                    "temperament": cur.temperament,
                })

            elif op == "generate_candidates":
                cur = self.get_or_create(being)
                limit = args.get("limit")
                limit = 8 if limit is None else int(limit)
                auto_mark = bool(args.get("auto_mark", False))

                mx = self.limits["perception_items"]
                peop = bounded_list(args, "people", mx)
                thin = bounded_list(args, "things", mx)
                plac = bounded_list(args, "places", mx)
                fact = bounded_list(args, "facts", mx)

                novel_people = [p for p in peop if p not in cur.known_people]
                novel_things = [t for t in thin if t not in cur.known_things]
                novel_places = [pl for pl in plac if pl not in cur.known_places]
                novel_facts = [f for f in fact if f not in cur.known_facts]

                candidates = []

                # 1. people -> ask
                for p in novel_people:
                    if len(candidates) >= limit:
                        break
                    prio = max(1, min(100, scale_score(cur.temperament, BASE_SCORE_PERSON)))
                    candidates.append({
                        "attitudes": [],
                        "events": [],
                        "id": f"curious_ask_{p}",
                        "kind": "ask",
                        "priority": prio,
                        "promises": [],
                        "reason": f"Inquire about stranger {p}",
                        "recipient": p,
                        "requires": [],
                        "resource": None,
                    })

                # 2. facts -> ask
                for f in novel_facts:
                    if len(candidates) >= limit:
                        break
                    prio = max(1, min(100, scale_score(cur.temperament, BASE_SCORE_FACT)))
                    candidates.append({
                        "attitudes": [],
                        "events": [],
                        "id": f"curious_fact_{f}",
                        "kind": "ask",
                        "priority": prio,
                        "promises": [],
                        "reason": f"Ask about fact {f}",
                        "recipient": None,
                        "requires": [],
                        "resource": None,
                    })

                # 3. places -> activity
                for pl in novel_places:
                    if len(candidates) >= limit:
                        break
                    prio = max(1, min(100, scale_score(cur.temperament, BASE_SCORE_PLACE)))
                    candidates.append({
                        "attitudes": [],
                        "events": [],
                        "id": f"curious_place_{pl}",
                        "kind": "activity",
                        "priority": prio,
                        "promises": [],
                        "reason": f"Explore new location {pl}",
                        "recipient": None,
                        "requires": [],
                        "resource": None,
                    })

                # 4. things -> activity
                for t in novel_things:
                    if len(candidates) >= limit:
                        break
                    prio = max(1, min(100, scale_score(cur.temperament, BASE_SCORE_THING)))
                    candidates.append({
                        "attitudes": [],
                        "events": [],
                        "id": f"curious_thing_{t}",
                        "kind": "activity",
                        "priority": prio,
                        "promises": [],
                        "reason": f"Examine unfamiliar {t}",
                        "recipient": None,
                        "requires": [],
                        "resource": None,
                    })

                if auto_mark:
                    for p in novel_people:
                        if len(cur.known_people) < self.limits["known"]:
                            cur.known_people.add(p)
                    for f in novel_facts:
                        if len(cur.known_facts) < self.limits["known"]:
                            cur.known_facts.add(f)
                    for pl in novel_places:
                        if len(cur.known_places) < self.limits["known"]:
                            cur.known_places.add(pl)
                    for t in novel_things:
                        if len(cur.known_things) < self.limits["known"]:
                            cur.known_things.add(t)

                results.append({
                    "being": cur.being,
                    "candidates": candidates,
                    "count": len(candidates),
                })

            elif op == "mark_familiar":
                cur = self.get_or_create(being)
                for k, target in [
                    ("people", cur.known_people),
                    ("things", cur.known_things),
                    ("places", cur.known_places),
                    ("facts", cur.known_facts),
                ]:
                    for s in bounded_list(args, k, self.limits["known"]):
                        if len(target) < self.limits["known"]:
                            target.add(s)
                results.append({"being": cur.being, "ok": True})

            elif op == "assess":
                if being not in self.beings:
                    raise ValueError(f"unknown being {being}")
                results.append(self.beings[being].to_dict())

            else:
                raise ValueError(f"unknown curiosity op {op}")

        return {"results": results, "version": 1}


def process_local(kind: str, args: Dict[str, Any]) -> Dict[str, Any]:
    if kind == "offscreen":
        mgr = OffscreenReference()
        return mgr.run(args)
    elif kind == "curiosity":
        mgr = CuriosityReference()
        return mgr.run(args)
    else:
        raise ValueError(f"unknown local kind {kind}")
