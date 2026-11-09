"""Durable save schemas and invariants for every stateful pass 2-4 manager (pass 5, stage 1).
Independent original reference; Python standard library only; nothing here calls a model.

Each kind names its manager reference, its limit spec, the schema of its `state` (see
`durable_reference`) and the invariants an imported save must meet: everything the ops enforce,
so an import can never produce a state the ops could not have reached.
"""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from durable_reference import (MAX_AT, MAX_INT, Kind, is_ident, is_text, need, register, strictly_sorted)

import chemistry_reference as chem

# ------------------------------------------------------------------ chemistry (pass 4 stage 6)

_CH_STYLE = {"assertiveness": ("u", 0, 100), "formality": ("u", 0, 100), "humour": ("in", chem.HUMOUR),
             "tempo": ("in", chem.TEMPO), "values": ("list", "s")}
_CH_JOKE = {"at": ("u", 0, MAX_AT), "freshness": ("u", 0, chem.FULL), "moment": "s", "retired": "b",
            "uses": ("u", 0, chem.RETIRE)}
_CH_PAIR = {"breaches": ("u", 0, 1000), "favors": ("u", 0, 1000), "jokes": ("map", "s", _CH_JOKE),
            "landmarks": ("u", 0, 1000)}
_CH_BEAT = {"at": ("u", 0, MAX_AT), "engagement": ("u", 0, 100), "novelty": ("u", 0, 100)}
_CH_SCENE = {"beats": ("list", _CH_BEAT), "last_delta": ("u", 0, chem.FULL), "silence": ("u", 0, 16),
             "tension": ("u", 0, chem.FULL), "threads": ("u", 0, 16)}


def _ch_settle(m) -> None:
    k = KIND_CHEM
    k.within("beings", len(m.styles))
    k.within("pairs", len(m.pairs))
    k.within("scenes", len(m.scenes))
    for b, s in m.styles.items():
        need(is_ident(b) and len(s["values"]) <= 8 and strictly_sorted(s["values"]))
        need(all(is_ident(v) for v in s["values"]))
    for (a, b), p in m.pairs.items():
        need(is_ident(a) and is_ident(b) and a.encode() < b.encode())
        k.within("jokes", len(p["jokes"]))
        need(not p["jokes"] or (a in m.styles and b in m.styles))
        for jid, j in p["jokes"].items():
            need(is_ident(jid) and is_text(j["moment"], 80) and j["at"] <= m.clock)
            need(j["retired"] == (j["uses"] >= chem.RETIRE))
    for sid, sc in m.scenes.items():
        need(is_ident(sid) and len(sc["beats"]) <= chem.WINDOW)
        need(all(x["at"] <= y["at"] for x, y in zip(sc["beats"], sc["beats"][1:])))
        need(all(x["at"] <= m.clock for x in sc["beats"]))


KIND_CHEM = register(Kind(
    "chemistry", chem.Chemistry, chem.SPEC,
    {"clock": ("u", 0, MAX_AT), "pairs": ("map", ("tup", "s", "s"), _CH_PAIR), "scenes": ("map", "s", _CH_SCENE),
     "styles": ("map", "s", _CH_STYLE)},
    _ch_settle))

# ------------------------------------------------------------------ minds (pass 4 stage 2)

import minds_reference as minds_ref

_MI_MIND = {"affect": ("list", ("in", minds_ref.CHANNELS)), "deception": ("in", minds_ref.DECEPTION),
            "fixation": ("opt", ("tup", "s", "u")), "language": ("in", minds_ref.LANGUAGE),
            "memory": ("in", minds_ref.MEMORY), "theory_of_mind": ("u", 0, 2), "time_scale": ("u", 1, 1_000_000_000)}


def _mi_get(m):
    out = {}
    for mid, x in m.minds.items():
        y = dict(x)
        y["fixation"] = None if x["fixation"] is None else (x["fixation"]["goal"], x["fixation"]["strength"])
        out[mid] = y
    return {"beings": m.beings, "clock": m.clock, "minds": out}


def _mi_put(m, st):
    m.beings, m.clock = st["beings"], st["clock"]
    m.minds = {}
    for mid, x in st["minds"].items():
        y = dict(x)
        y["fixation"] = None if x["fixation"] is None else {"goal": x["fixation"][0], "strength": x["fixation"][1]}
        m.minds[mid] = y


def _mi_settle(m) -> None:
    k = KIND_MINDS
    k.within("minds", len(m.minds))
    k.within("beings", len(m.beings))
    for mid, x in m.minds.items():
        need(is_ident(mid) and mid not in minds_ref.BUILTIN and strictly_sorted(x["affect"]))
        if x["fixation"] is not None:
            need(is_ident(x["fixation"]["goal"]) and 1 <= x["fixation"]["strength"] <= 100)
    for b, st in m.beings.items():
        need(is_ident(b) and (st["pool"] is None or is_ident(st["pool"])))
        mind = minds_ref.BUILTIN.get(st["mind"]) or m.minds.get(st["mind"])
        need(mind is not None)
        need((st["pool"] is not None) == (mind["memory"] == "shared"))


KIND_MINDS = register(Kind(
    "minds", minds_ref.Minds, minds_ref.SPEC,
    {"beings": ("map", "s", {"mind": "s", "pool": ("opt", "s"), "released": "b"}), "clock": ("u", 0, MAX_AT),
     "minds": ("map", "s", _MI_MIND)},
    _mi_settle, get=_mi_get, put=_mi_put))

# ------------------------------------------------------------------ defenses (pass 4 stage 3)

import defenses_reference as def_ref

_DE_PROFILE = {"at": ("u", 0, MAX_AT), "cooldown": ("u", 1, MAX_AT), "crisis": ("in", def_ref.CRISES),
               "crisis_at": ("opt", ("u", 0, MAX_AT)),
               "distortions": ("map", "s", {"hits": ("u", 1, MAX_AT), "mechanism": ("in", def_ref.MECHANISMS)}),
               "maturity": ("u", 0, 100), "protects": ("list", "s"), "strain": ("u", 0, def_ref.FULL),
               "style": ("list", ("in", def_ref.MECHANISMS)), "threshold": ("u", 1, def_ref.FULL),
               "tolerance": ("u", 0, 100)}
_DE_TAGGED = ["denial", "projection", "displacement", "rationalization"]


def _de_settle(m) -> None:
    k = KIND_DEF
    k.within("beings", len(m.beings))
    for b, p in m.beings.items():
        need(is_ident(b) and p["at"] <= m.clock and (p["crisis_at"] is None or p["crisis_at"] <= m.clock))
        need(len(set(p["style"])) == len(p["style"]))
        need(len(p["protects"]) <= 16 and strictly_sorted(p["protects"]) and all(is_ident(x) for x in p["protects"]))
        k.within("distortions", len(p["distortions"]))
        need(all(is_ident(f) and d["mechanism"] in _DE_TAGGED for f, d in p["distortions"].items()))


KIND_DEF = register(Kind("defenses", def_ref.Defenses, def_ref.SPEC,
                         {"beings": ("map", "s", _DE_PROFILE), "clock": ("u", 0, MAX_AT)}, _de_settle))

# ------------------------------------------------------------------ projects (pass 4 stage 5)

import projects_reference as proj_ref

PROJECT_STATUS = ["active", "paused", "completed", "abandoned"]
_PR_COND = {"kind": ("in", proj_ref.CONDITIONS), "level": ("opt", ("u", 0, 100)), "reference": "s"}
_PR_ALT = {"alternative": ("opt", "any"), "condition": _PR_COND, "expected": ("u", 1, MAX_AT), "id": "s"}
_PR_MS = {"alternative": ("opt", _PR_ALT), "condition": _PR_COND, "expected": ("u", 1, MAX_AT), "id": "s"}
_PR_PROJECT = {"adapted": ("u", 0, 32), "aim": "s", "at": ("u", 0, 32), "failures": ("u", 0, MAX_AT),
               "horizon": ("u", 0, MAX_AT), "milestones": ("list", _PR_MS), "money": ("u", 0, MAX_AT),
               "morale": ("u", 0, proj_ref.FULL), "owner": "s", "stake": ("u", 0, 100),
               "status": ("in", PROJECT_STATUS), "time": ("u", 0, MAX_AT)}


def _pr_ms_out(m):
    c = m["condition"]
    return {"alternative": _pr_ms_out(m["alternative"]) if m.get("alternative") is not None else None,
            "condition": {"kind": c["kind"], "level": c.get("level"), "reference": c["ref"]},
            "expected": m["expected"], "id": m["id"]}


def _pr_ms_in(m):
    c = m["condition"]
    out = {"id": m["id"], "condition": {"kind": c["kind"], "ref": c["reference"]}, "expected": m["expected"]}
    if c["level"] is not None:
        out["condition"]["level"] = c["level"]
    if m["alternative"] is not None:
        out["alternative"] = _pr_ms_in(m["alternative"]) if isinstance(m["alternative"], dict) else m["alternative"]
    return out


def _pr_get(m):
    return {"clock": m.clock, "projects": {pid: {**p, "milestones": [_pr_ms_out(x) for x in p["milestones"]]}
                                           for pid, p in m.projects.items()}}


def _pr_put(m, st):
    m.clock = st["clock"]
    m.projects = {pid: {**p, "milestones": [_pr_ms_in(x) for x in p["milestones"]]} for pid, p in st["projects"].items()}


def _pr_ms_ok(m) -> bool:
    c = m["condition"]
    return is_ident(m["id"]) and is_ident(c["ref"]) and (("level" in c) == (c["kind"] == "relationship"))


def _pr_settle(m) -> None:
    k = KIND_PROJ
    k.within("projects", len(m.projects))
    for pid, p in m.projects.items():
        need(is_ident(pid) and is_ident(p["owner"]) and is_text(p["aim"], 80))
        need(len(p["milestones"]) > 0 and p["at"] <= len(p["milestones"]))
        k.within("milestones", len(p["milestones"]))
        need((p["at"] == len(p["milestones"])) == (p["status"] == "completed"))
        ids = set()
        for x in p["milestones"]:
            need(_pr_ms_ok(x) and x["id"] not in ids)
            ids.add(x["id"])
            a = x.get("alternative")
            if a is not None:
                need(isinstance(a, dict) and _pr_ms_ok(a) and "alternative" not in a and a["id"] not in ids)
                ids.add(a["id"])


KIND_PROJ = register(Kind("projects", proj_ref.Projects, proj_ref.SPEC,
                          {"clock": ("u", 0, MAX_AT), "projects": ("map", "s", _PR_PROJECT)}, _pr_settle,
                          get=_pr_get, put=_pr_put))

# ------------------------------------------------------------------ culture (pass 4 stage 4)

import culture_reference as cul_ref

_CU_MEMBER = {"carefulness": ("u", 0, 100), "community": "s", "credulity": ("u", 0, 100), "links": ("list", "s"),
              "openness": ("u", 0, 100),
              "repertoire": ("map", "s", {"at": ("u", 0, MAX_AT), "strength": ("u", 0, cul_ref.FULL)}),
              "status": ("u", 0, 100)}
_CU_IDIOM = {"community": "s", "meaning": "s", "phrase": "s", "prestige": ("u", 0, 100)}
_CU_TRUTH = {"agent": "s", "amount": ("opt", ("u", 1, 1_000_000_000)), "details": ("list", "s"),
             "place": ("opt", "s"), "verb": "s", "witness": "s"}
_CU_VERSION = {"agent": "s", "amount": ("opt", ("u", 1, cul_ref.MAX_AMOUNT)), "chain": ("list", "s"),
               "details": ("list", "s"), "hops": ("u", 0, MAX_AT), "last": ("opt", ("in", cul_ref.OPERATORS)),
               "merged": ("list", "s"), "ops": ("u", 0, MAX_AT), "place": ("opt", "s"), "verb": "s"}


def _unique_idents(xs, mx: int) -> bool:
    return len(xs) <= mx and all(is_ident(x) for x in xs) and len(set(xs)) == len(xs)


def _cu_settle(m) -> None:
    k = KIND_CUL
    k.within("beings", len(m.members))
    k.within("idioms", len(m.idioms))
    k.within("topics", len(m.truths))
    for b, x in m.members.items():
        need(is_ident(b) and is_ident(x["community"]) and _unique_idents(x["links"], 8)
             and strictly_sorted(x["links"]))
        need(b not in x["links"])
        k.within("repertoire", len(x["repertoire"]))
        need(all(i in m.idioms and e["at"] <= m.clock for i, e in x["repertoire"].items()))
    for iid, i in m.idioms.items():
        need(is_ident(iid) and is_ident(i["community"]) and is_text(i["phrase"], 60) and is_text(i["meaning"], 80))
    for topic, t in m.truths.items():
        need(is_ident(topic) and is_ident(t["agent"]) and is_ident(t["verb"]) and t["witness"] in m.members)
        need((t["place"] is None or is_ident(t["place"])) and _unique_idents(t["details"], 4))
        need(topic in m.beliefs.get(t["witness"], {}))
    count = 0
    holders = {}
    for b, held in m.beliefs.items():
        need(b in m.members and len(held) > 0)
        for topic, v in held.items():
            need(topic in m.truths)
            t = m.truths[topic]
            need(is_ident(v["agent"]) and is_ident(v["verb"]) and (v["place"] is None or is_ident(v["place"])))
            need((v["amount"] is None) == (t["amount"] is None) and (v["last"] is None) == (v["ops"] == 0))
            need(_unique_idents(v["details"], 4) and all(d in t["details"] for d in v["details"]))
            need(len(v["chain"]) == min(v["hops"] + 1, cul_ref.CHAIN) and v["chain"] and v["chain"][-1] == b)
            need(all(is_ident(x) for x in v["chain"]))
            need(len(v["merged"]) <= 4 and strictly_sorted(v["merged"]))
            need(all(x != topic and x in m.truths for x in v["merged"]))
            holders.setdefault(topic, set()).add(b)
            count += 1
    k.within("beliefs", count)
    m.holders, m.count = holders, count


KIND_CUL = register(Kind("culture", cul_ref.Culture, cul_ref.SPEC,
                         {"beliefs": ("map", "s", ("map", "s", _CU_VERSION)), "clock": ("u", 0, MAX_AT),
                          "idioms": ("map", "s", _CU_IDIOM), "members": ("map", "s", _CU_MEMBER),
                          "truths": ("map", "s", _CU_TRUTH)}, _cu_settle))

# ------------------------------------------------------------------ genre (pass 4 stage 1)

import genre_reference as gen_ref

_GE_GENRE = {"axes": ("arr", ("u", 0, 100), 4), "builtin": "b", "day_cycle": ("in", gen_ref.CYCLES),
             "vocab": ("list", ("list", "s"))}


def _ge_get(m):
    genres = {gid: {"axes": [g["axes"][a] for a in gen_ref.AXES], "builtin": g["builtin"],
                    "day_cycle": g["day_cycle"], "vocab": [g["vocab"][s] for s in gen_ref.SLOT_NAMES]}
              for gid, g in m.genres.items()}
    packs = {pid: [(act, ps) for act, ps in p.items()] for pid, p in m.packs.items()}
    return {"genres": genres, "packs": packs}


def _ge_put(m, st):
    m.genres = {}
    for gid, g in st["genres"].items():
        need(len(g["vocab"]) == len(gen_ref.SLOT_NAMES))
        m.genres[gid] = {"axes": dict(zip(gen_ref.AXES, g["axes"])), "day_cycle": g["day_cycle"],
                         "vocab": dict(zip(gen_ref.SLOT_NAMES, g["vocab"])), "builtin": g["builtin"]}
    # Packs stay as ordered rows until `_ge_settle` has checked the act order.
    m.packs = {pid: list(acts) for pid, acts in st["packs"].items()}


def _ge_settle(m) -> None:
    k = KIND_GEN
    k.within("genres", len(m.genres))
    k.within("packs", len(m.packs))
    for gid, g in m.genres.items():
        need(is_ident(gid) and gid not in gen_ref.BUILTIN_GENRES and not g["builtin"])
        for slot in gen_ref.SLOT_NAMES:
            names = g["vocab"][slot]
            need(len(names) == len(gen_ref.ENGINE[slot]) and len(set(names)) == len(names))
            need(all(is_text(n, 40) if slot == "address" else is_ident(n) for n in names))
    packs = {}
    for pid, p in m.packs.items():
        rows = p
        need(is_ident(pid) and pid not in gen_ref.BUILTIN_PACKS and len(rows) > 0)
        order = [gen_ref.PRECEDENCE.index(a) for a, _ in rows]
        need(strictly_sorted(order))
        seen = set()
        for _, ps in rows:
            need(1 <= len(ps) <= 16)
            for ph in ps:
                try:
                    gen_ref.phrase_of(ph)
                except Exception:  # noqa: BLE001
                    need(False)
                need(ph not in seen)
                seen.add(ph)
        packs[pid] = {a: ps for a, ps in rows}
    m.packs = packs


KIND_GEN = register(Kind("genre", gen_ref.Genres, gen_ref.SPEC,
                         {"genres": ("map", "s", _GE_GENRE),
                          "packs": ("map", "s", ("list", ("tup", ("in", gen_ref.PRECEDENCE), ("list", "s"))))},
                         _ge_settle, get=_ge_get, put=_ge_put))

# ------------------------------------------------------------------ embodiment (pass 3 stage 4)

import embodiment_reference as emb_ref

POSTURES = ["steps_back", "squares_shoulders", "turns_away", "slumps", "leans_in"]
GAZES = ["avoids_eye_contact", "glances_at_exit", "watches_bystander", "stares_at_held_item", "meets_eyes"]
HANDS = ["reaches_for_weapon", "clenches_fist", "offers_item", "continues_working", "sets_down_tool", "pauses_work"]


def _em_settle(m) -> None:
    KIND_EMB.within("beings", len(m.last))
    need(all(is_ident(b) and x["at"] <= m.clock for b, x in m.last.items()))


KIND_EMB = register(Kind("embodiment", emb_ref.Embodiment, emb_ref.SPEC,
                         {"clock": ("u", 0, MAX_AT),
                          "last": ("map", "s", {"at": ("u", 0, MAX_AT), "gaze": ("opt", ("in", GAZES)),
                                                "hand": ("opt", ("in", HANDS)),
                                                "posture": ("opt", ("in", POSTURES))})}, _em_settle))

# ------------------------------------------------------------------ turn frame (pass 3 stage 1)

import frame_reference as frame_ref

_FR_M = 1_000_000


def _fr_settle(m) -> None:
    k = KIND_FRAME
    k.within("beings", len(m.beings))
    for b, x in m.beings.items():
        need(is_ident(b))
        k.within("keys", len(x["keys"]))
        for key, xs in x["keys"].items():
            need(is_ident(key) and len(xs) > 0)
            k.within("samples", len(xs))
            need(all(t <= m.clock and -_FR_M <= v <= _FR_M for t, v in xs))
            need(all(a[0] <= b2[0] for a, b2 in zip(xs, xs[1:])))


KIND_FRAME = register(Kind("turn-frame", frame_ref.Frame, frame_ref.SPEC,
                           {"beings": ("map", "s", {"keys": ("map", "s", ("list", ("tup", ("u", 0, MAX_AT), "i"))),
                                                    "streak": ("i", -1000, 1000)}),
                            "clock": ("u", 0, MAX_AT)}, _fr_settle))

# ------------------------------------------------------------------ pragmatics (pass 3 stage 3)

import pragmatics_reference as prag_ref

THREAD_STATUS = ["active", "waiting", "interrupted"]
_PG_THREAD = {"cause": ("opt", "s"), "id": "s", "interrupted_at": ("opt", ("u", 0, MAX_AT)),
              "opened_at": ("u", 0, MAX_AT), "question": ("opt", "s"), "speaker": "s",
              "status": ("in", THREAD_STATUS), "topic": "s"}


def _pg_phrase_ok(s) -> bool:
    if not is_text(s, 64):
        return False
    ws = prag_ref.words(s)
    return 1 <= len(ws) <= 4 and " ".join(ws) == s


def _pg_settle(m) -> None:
    k = KIND_PRAG
    need(len(m.custom) == len(prag_ref.PRECEDENCE))
    count = 0
    for act in prag_ref.PRECEDENCE:
        need(act in m.custom)
        xs = m.custom[act]
        need(len(set(xs)) == len(xs) and all(_pg_phrase_ok(p) and p not in prag_ref.LEXICON[act] for p in xs))
        count += len(xs)
    k.within("phrases", count)
    m.count = count
    m.custom = {a: m.custom[a] for a in prag_ref.PRECEDENCE}
    k.within("beings", len(m.stacks))
    for b, st in m.stacks.items():
        need(is_ident(b))
        k.within("threads", len(st))
        need(len({t["id"] for t in st}) == len(st))
        need(all(x["opened_at"] <= y["opened_at"] for x, y in zip(st, st[1:])))
        for i, t in enumerate(st):
            need(is_ident(t["id"]) and is_ident(t["topic"]) and is_ident(t["speaker"]) and t["opened_at"] <= m.clock)
            need(t["question"] is None or is_text(t["question"], 160))
            inter = t["status"] == "interrupted"
            need((t["cause"] is not None) == inter and (t["interrupted_at"] is not None) == inter)
            need((t["cause"] is None or is_ident(t["cause"]))
                 and (t["interrupted_at"] is None or t["interrupted_at"] <= m.clock))
            need(t["status"] != "active" or i + 1 == len(st))


KIND_PRAG = register(Kind("pragmatics", prag_ref.Pragmatics, prag_ref.SPEC,
                          {"clock": ("u", 0, MAX_AT), "custom": ("map", "s", ("list", "s")),
                           "stacks": ("map", "s", ("list", _PG_THREAD))}, _pg_settle))

# ------------------------------------------------------------------ cueing (pass 3 stage 2)

import cueing_reference as cue_ref

SOURCE_NAMES = ["corrected_belief", "direct_witness", "inference", "trusted_report", "unverified_rumor"]
_FEATURE_NAMES = [k for k, _ in cue_ref.FEATURES]
_CU_EPISODE = {"emotion": ("opt", "s"), "features": ("arr", ("list", "s"), 4), "secret": "b",
               "surfaced": ("map", "s", ("u", 0, MAX_AT)), "valence": ("i", -100, 100)}
_CU_FACT = {"claim": "s", "confidence": ("u", 0, 100), "from": ("opt", "s"), "keys": ("list", "s"),
            "previous": ("opt", "s"), "source": "s"}
_CU_COVER = {"fact": "s", "lie": "s", "listeners": ("list", "s")}


def _cq_get(m):
    beings = {}
    for b, x in m.beings.items():
        eps = {i: {**e, "features": [e["features"][k] for k in _FEATURE_NAMES]} for i, e in x["episodes"].items()}
        beings[b] = {"covers": x["covers"], "episodes": eps, "facts": x["facts"]}
    return {"beings": beings, "clock": m.clock, "secrets": m.secrets}


def _cq_put(m, st):
    m.clock, m.secrets = st["clock"], st["secrets"]
    m.beings = {}
    for b, x in st["beings"].items():
        eps = {i: {**e, "features": dict(zip(_FEATURE_NAMES, e["features"]))} for i, e in x["episodes"].items()}
        m.beings[b] = {"episodes": eps, "facts": x["facts"], "covers": x["covers"]}


def _cq_keys_ok(ks) -> bool:
    return (1 <= len(ks) <= cue_ref.MAX_KEYS and strictly_sorted(ks)
            and all(1 <= len(w) <= 32 and all(ch in "abcdefghijklmnopqrstuvwxyz0123456789" for ch in w) for w in ks))


def _cq_ids_ok(xs) -> bool:
    return len(xs) <= cue_ref.MAX_LIST and strictly_sorted(xs) and all(is_ident(x) for x in xs)


def _cq_settle(m) -> None:
    k = KIND_CUE
    k.within("beings", len(m.beings))
    k.within("secrets", len(m.secrets))
    need(all(is_ident(i) and _cq_keys_ok(ks) for i, ks in m.secrets.items()))
    for b, x in m.beings.items():
        need(is_ident(b) and not (not x["episodes"] and not x["facts"]))
        k.within("episodes", len(x["episodes"]))
        k.within("facts", len(x["facts"]))
        k.within("covers", len(x["covers"]))
        for i, e in x["episodes"].items():
            fs = [e["features"][n] for n in _FEATURE_NAMES]
            need(is_ident(i) and all(_cq_ids_ok(s) for s in fs) and any(fs))
            need(e["emotion"] is None or is_ident(e["emotion"]))
            k.within("listeners", len(e["surfaced"]))
            need(all(is_ident(li) and t <= m.clock for li, t in e["surfaced"].items()))
        for i, f in x["facts"].items():
            need(is_ident(i) and is_text(f["claim"], 160) and f["source"] in SOURCE_NAMES)
            reported = f["source"] in ("trusted_report", "unverified_rumor")
            need((f["from"] is not None) == reported and (f["from"] is None or is_ident(f["from"])))
            need((f["previous"] is not None) == (f["source"] == "corrected_belief"))
            need((f["previous"] is None or is_text(f["previous"], 160)) and _cq_keys_ok(f["keys"]))
        for i, cv in x["covers"].items():
            need(is_ident(i) and cv["fact"] in x["facts"] and is_text(cv["lie"], 160) and _cq_ids_ok(cv["listeners"]))


KIND_CUE = register(Kind("cueing", cue_ref.Cueing, cue_ref.SPEC,
                         {"beings": ("map", "s", {"covers": ("map", "s", _CU_COVER),
                                                  "episodes": ("map", "s", _CU_EPISODE),
                                                  "facts": ("map", "s", _CU_FACT)}),
                          "clock": ("u", 0, MAX_AT), "secrets": ("map", "s", ("list", "s"))},
                         _cq_settle, get=_cq_get, put=_cq_put))

# ------------------------------------------------------------------ fabric (pass 3 stage 5)

import fabric_reference as fab_ref

_FA_NORM = {"applies_when": ("list", "s"), "guilt_cost": ("u", 0, 100), "report_to": ("opt", "s"),
            "severity": ("u", 0, 100)}
_FA_CONTRACT = {"creditor": "s", "debtor": "s", "due_at": ("opt", ("u", 0, MAX_AT)), "kind": ("in", fab_ref.KINDS),
                "overdue": "b", "terms": "s", "weight": ("u", 1, 100), "witnesses": ("list", "s")}
_FA_MOMENT = {"at": ("u", 0, MAX_AT), "episode": "s", "kind": "s", "summary": ("opt", "s")}
_FA_DYAD = {"address": ("map", "s", "s"), "first": ("tup", ("u", 0, MAX_AT), "s", "s"),
            "landmarks": ("list", _FA_MOMENT), "threads": ("list", {"id": "s", "opened_by": "s", "text": "s"})}
_FA_ATT = {"bandwidth": ("u", 1, 16), "biases": ("map", "s", "u"), "vigilance": ("u", 0, 100)}
_FA_PRAG = {"conflict_style": ("in", fab_ref.STYLES), "cooperativeness": ("in", fab_ref.COOP),
            "coping": ("opt", "s"), "interrupt_tolerance": ("u", 0, 100), "secret_keeping": ("u", 0, 100),
            "small_talk_patience": ("u", 0, 100),
            "tells": ("list", {"cue": "s", "kind": ("in", fab_ref.TELL_KINDS), "min_insight": ("u", 0, 100)})}


def _fa_get(m):
    contracts = {i: {**{k: v for k, v in c.items() if k != "status"}, "overdue": c["status"] == "overdue"}
                 for i, c in m.contracts.items()}
    dyads = {k: {**d, "first": (d["first"]["at"], d["first"]["episode"], d["first"]["summary"])}
             for k, d in m.dyads.items()}
    profiles = {b: (p["attention"], p["pragmatic"]) for b, p in m.profiles.items()}
    return {"clock": m.clock, "contracts": contracts, "counts": m.counts, "dyads": dyads, "norms": m.norms,
            "profiles": profiles}


def _fa_put(m, st):
    m.clock, m.counts, m.norms = st["clock"], st["counts"], st["norms"]
    m.contracts = {i: {**{k: v for k, v in c.items() if k != "overdue"}, "status": "overdue" if c["overdue"] else "open"}
                   for i, c in st["contracts"].items()}
    m.dyads = {k: {**d, "first": {"at": d["first"][0], "episode": d["first"][1], "summary": d["first"][2]}}
               for k, d in st["dyads"].items()}
    m.profiles = {b: {"attention": p[0], "pragmatic": p[1]} for b, p in st["profiles"].items()}


def _fa_sorted_ids(xs, mx: int) -> bool:
    return len(xs) <= mx and strictly_sorted(xs) and all(is_ident(x) for x in xs)


def _fa_settle(m) -> None:
    k = KIND_FAB
    k.within("norms", len(m.norms))
    k.within("breaches", len(m.counts))
    k.within("contracts", len(m.contracts))
    k.within("dyads", len(m.dyads))
    k.within("beings", len(m.profiles))
    for nid, n in m.norms.items():
        need(is_ident(nid) and n["severity"] in fab_ref.SEVERITY.values() and _fa_sorted_ids(n["applies_when"], 8))
        need(n["report_to"] is None or is_ident(n["report_to"]))
    for (n, off, wit), c in m.counts.items():
        need(n in m.norms and is_ident(off) and is_ident(wit) and off != wit and 1 <= c <= MAX_AT)
    for cid, c in m.contracts.items():
        need(is_ident(cid) and is_ident(c["debtor"]) and is_ident(c["creditor"]) and c["debtor"] != c["creditor"])
        need(_fa_sorted_ids(c["witnesses"], 8) and c["debtor"] not in c["witnesses"]
             and c["creditor"] not in c["witnesses"])
        overdue = c["due_at"] is not None and c["due_at"] < m.clock
        need(is_text(c["terms"], 160) and (c["status"] == "overdue") == overdue)
    for (a, b), d in m.dyads.items():
        need(is_ident(a) and is_ident(b) and a.encode() < b.encode())
        f = d["first"]
        need(f["at"] <= m.clock and is_ident(f["episode"]) and is_text(f["summary"], 160)
             and len(d["landmarks"]) <= fab_ref.LANDMARKS)
        kinds, last = set(), f["at"]
        for mo in d["landmarks"]:
            need(is_ident(mo["kind"]) and mo["kind"] not in kinds and is_ident(mo["episode"]) and mo["at"] >= last)
            need(mo["at"] <= m.clock and (mo["summary"] is None or is_text(mo["summary"], 160)))
            kinds.add(mo["kind"])
            last = mo["at"]
        need(all(w in (a, b) and is_text(t, 40) for w, t in d["address"].items()))
        k.within("threads", len(d["threads"]))
        ids = set()
        for t in d["threads"]:
            need(is_ident(t["id"]) and t["id"] not in ids and t["opened_by"] in (a, b) and is_text(t["text"], 160))
            ids.add(t["id"])
    for b, p in m.profiles.items():
        att, prag = p["attention"], p["pragmatic"]
        need(is_ident(b) and len(att["biases"]) <= 16 and all(is_ident(x) and v <= 300 for x, v in att["biases"].items()))
        need(len(prag["tells"]) <= 8 and all(is_ident(t["cue"]) for t in prag["tells"]))
        need(prag["coping"] is None or is_ident(prag["coping"]))


KIND_FAB = register(Kind("fabric", fab_ref.Fabric, fab_ref.SPEC,
                         {"clock": ("u", 0, MAX_AT), "contracts": ("map", "s", _FA_CONTRACT),
                          "counts": ("map", ("tup", "s", "s", "s"), "u"), "dyads": ("map", ("tup", "s", "s"), _FA_DYAD),
                          "norms": ("map", "s", _FA_NORM), "profiles": ("map", "s", ("tup", _FA_ATT, _FA_PRAG))},
                         _fa_settle, get=_fa_get, put=_fa_put))

# ------------------------------------------------------------------ surroundings (pass 3 stage 5)

import surroundings_reference as sur_ref

PRIVACY_NAMES = ["public", "semi_private", "private", "echoing"]
_SU_PHASE = {"end": ("u", 1, 1_000_000), "id": "s", "light": ("u", 0, 2), "start": ("u", 0, 1_000_000),
             "temp_delta": ("i", -100, 100)}
_SU_WEATHER = {"light_delta": ("i", -2, 2), "need_rates": ("map", "s", "u"), "noise": ("u", 0, 100),
               "seek_tags": ("list", "s"), "travel_pct": ("u", 50, 500)}
_SU_ENV = {"day_length": ("u", 1, 1_000_000), "phases": ("list", _SU_PHASE), "weather": ("map", "s", _SU_WEATHER)}
_SU_STATION = {"max_users": ("u", 1, 64), "queue": ("list", "s"), "users": ("list", "s")}
_SU_PLACE = {"access": ("in", sur_ref.ACCESS), "capacity": ("opt", ("u", 1, 10_000)), "exterior": "b",
             "hours": ("opt", {"end": ("u", 0, 1_000_000), "outside": ("in", sur_ref.ACCESS),
                               "start": ("u", 0, 1_000_000)}),
             "occupants": ("list", "s"), "owner": ("opt", "s"), "privacy": ("in", PRIVACY_NAMES),
             "stations": ("map", "s", _SU_STATION)}
_SU_KIND = {"cleared_by": ("list", "s"), "created_by": ("list", "s"), "implies": "s", "public": "b",
            "sight": ("opt", "s"), "ttl": ("u", 1, MAX_AT)}
_SU_ITEM = {"condition": ("u", 0, 100), "created": ("u", 0, MAX_AT), "held_as": ("in", sur_ref.HELD_AS),
            "maker": ("opt", "s"), "owner": ("opt", "s"), "perishable": ("opt", ("u", 1, MAX_AT)),
            "possessor": ("opt", "s"), "sentimental_to": ("list", "s")}


def _su_get(m):
    env = None
    if m.env is not None:
        env = {"day_length": m.env["day"], "weather": m.env["weather"],
               "phases": [{**p, "light": sur_ref.LIGHT.index(p["light"])} for p in m.env["phases"]]}
    places = {}
    for pid, p in m.places.items():
        st = {s: {"max_users": x["max"], "queue": x["queue"], "users": x["users"]} for s, x in p["stations"].items()}
        places[pid] = {**p, "stations": st}
    kinds = {k: {**{f: v for f, v in x.items() if f != "visibility"}, "public": x["visibility"] == "public"}
             for k, x in m.kinds.items()}
    return {"clock": m.clock, "env": env, "items": m.items, "kinds": kinds, "places": places, "traces": m.traces,
            "weather": m.weather}


def _su_put(m, st):
    m.clock, m.items, m.traces, m.weather = st["clock"], st["items"], st["traces"], st["weather"]
    e = st["env"]
    m.env = None if e is None else {"day": e["day_length"], "weather": e["weather"],
                                    "phases": [{**p, "light": sur_ref.LIGHT[p["light"]]} for p in e["phases"]]}
    m.places = {}
    for pid, p in st["places"].items():
        stations = {s: {"max": x["max_users"], "users": x["users"], "queue": x["queue"]} for s, x in p["stations"].items()}
        m.places[pid] = {**p, "stations": stations}
    m.kinds = {k: {**{f: v for f, v in x.items() if f != "public"}, "visibility": "public" if x["public"] else "inspect"}
               for k, x in st["kinds"].items()}


def _su_distinct(xs, lo: int, hi: int) -> bool:
    return lo <= len(xs) <= hi and all(is_ident(x) for x in xs) and len(set(xs)) == len(xs)


def _su_settle(m) -> None:
    k = KIND_SUR
    e = m.env
    if e is not None:
        need(1 <= len(e["phases"]) <= 24 and len(e["weather"]) <= 16)
        edge, ids = 0, set()
        for p in e["phases"]:
            need(is_ident(p["id"]) and p["id"] not in ids and p["start"] == edge and p["end"] > p["start"])
            ids.add(p["id"])
            edge = p["end"]
        need(edge == e["day"])
        for w, x in e["weather"].items():
            need(is_ident(w) and len(x["need_rates"]) <= 8 and _su_distinct(x["seek_tags"], 0, 8))
            need(all(is_ident(n) and v <= 100 for n, v in x["need_rates"].items()))
    need(m.weather is None or (e is not None and m.weather in e["weather"]))
    k.within("places", len(m.places))
    for pid, p in m.places.items():
        need(is_ident(pid) and (p["owner"] is None or is_ident(p["owner"])) and len(p["stations"]) <= 16)
        h = p["hours"]
        need(h is None or (h["start"] != h["end"] and e is not None))
        k.within("occupants", len(p["occupants"]))
        need(_su_distinct(p["occupants"], 0, 10 ** 18))
        for sid, s in p["stations"].items():
            need(is_ident(sid) and len(s["users"]) <= s["max"])
            need(not s["queue"] or len(s["users"]) == s["max"])
            both = s["users"] + s["queue"]
            need(_su_distinct(both, 0, 10 ** 18) and all(b in p["occupants"] for b in both))
    k.within("trace_kinds", len(m.kinds))
    for kid, x in m.kinds.items():
        need(is_ident(kid) and _su_distinct(x["created_by"], 1, 8) and _su_distinct(x["cleared_by"], 0, 8))
        need(is_ident(x["implies"]) and (x["sight"] is None or is_text(x["sight"], 160)))
    k.within("traces", len(m.traces))
    for (pid, kid), at in m.traces.items():
        need(pid in m.places and kid in m.kinds and at <= m.clock)
    k.within("items", len(m.items))
    for iid, it in m.items.items():
        need(is_ident(iid) and all(it[f] is None or is_ident(it[f]) for f in ("owner", "possessor", "maker")))
        need(_su_distinct(it["sentimental_to"], 0, 8) and strictly_sorted(it["sentimental_to"])
             and it["created"] <= m.clock)


KIND_SUR = register(Kind("surroundings", sur_ref.Surroundings, sur_ref.SPEC,
                         {"clock": ("u", 0, MAX_AT), "env": ("opt", _SU_ENV), "items": ("map", "s", _SU_ITEM),
                          "kinds": ("map", "s", _SU_KIND), "places": ("map", "s", _SU_PLACE),
                          "traces": ("map", ("tup", "s", "s"), ("u", 0, MAX_AT)), "weather": ("opt", "s")},
                         _su_settle, get=_su_get, put=_su_put))

# ------------------------------------------------------------------ explain (pass 2 D4)

import explain_stability_reference as es_ref

_EX_TRACE = {"action": "s", "at": ("u", 0, MAX_AT), "being": "s",
             "beliefs": ("list", {"about": "s", "confidence": ("u", 0, 100), "fact": "s", "source": "s"}),
             "id": "s",
             "motive": {"id": "s", "kind": "s", "rank": ("u", 1, 64), "reason": "s",
                        "runner_up": ("opt", ("tup", "s", "u")), "score": ("u", 0, 100)},
             "perceptions": ("list", {"source": "s", "text": "s"}), "seq": ("u", 1, MAX_AT), "status": "s",
             "summary": ("opt", "s"), "target": ("opt", "s"),
             "task": ("opt", {"id": "s", "index": ("u", 0, 63), "step": "s"})}


def _ex_out(t):
    m = t["motive"]
    ru = None if m["runner_up"] is None else (m["runner_up"]["id"], m["runner_up"]["score"])
    return {**{k: v for k, v in t.items() if k != "outcome"}, "motive": {**m, "runner_up": ru},
            "status": t["outcome"]["status"], "summary": t["outcome"]["summary"]}


def _ex_in(t):
    m = t["motive"]
    ru = None if m["runner_up"] is None else {"id": m["runner_up"][0], "score": m["runner_up"][1]}
    return {**{k: v for k, v in t.items() if k not in ("status", "summary")}, "motive": {**m, "runner_up": ru},
            "outcome": {"status": t["status"], "summary": t["summary"]}}


def _ex_trace_ok(b, t) -> bool:
    m = t["motive"]
    ru = m["runner_up"]
    return (t["being"] == b and t["id"] == f"tr_{t['seq']}" and is_ident(t["action"])
            and (t["target"] is None or is_ident(t["target"])) and len(t["perceptions"]) <= 8
            and all(is_ident(p["source"]) and is_text(p["text"], 160) for p in t["perceptions"])
            and len(t["beliefs"]) <= 8
            and all(is_ident(x["about"]) and is_text(x["fact"], 160) and x["source"] in es_ref.BELIEF_SOURCES
                    for x in t["beliefs"])
            and is_ident(m["id"]) and m["kind"] in es_ref.MOTIVE_KINDS and is_text(m["reason"], 200)
            and (ru is None or (is_ident(ru["id"]) and ru["score"] <= 100))
            and (t["task"] is None or (is_ident(t["task"]["id"]) and is_ident(t["task"]["step"])))
            and t["outcome"]["status"] in es_ref.RECORD_STATUSES
            and (t["outcome"]["summary"] is None or is_text(t["outcome"]["summary"], 200)))


def _ex_settle(m) -> None:
    k = KIND_EXPLAIN
    k.within("beings", len(m.beings))
    seqs = set()
    for b, h in m.beings.items():
        need(is_ident(b) and len(h) > 0)
        k.within("traces", len(h))
        need(all(x["seq"] < y["seq"] for x, y in zip(h, h[1:])))
        for t in h:
            need(_ex_trace_ok(b, t) and t["seq"] < m.next_seq and t["seq"] not in seqs)
            seqs.add(t["seq"])


KIND_EXPLAIN = register(Kind(
    "explain", es_ref.ExplainReference, [("beings", 64, 1_000_000), ("traces", 128, 4096)],
    {"beings": ("map", "s", ("list", _EX_TRACE)), "next_seq": ("u", 1, MAX_AT)}, _ex_settle,
    get=lambda m: {"beings": {b: [_ex_out(t) for t in h] for b, h in m.beings.items()}, "next_seq": m.next_seq},
    put=lambda m, st: (setattr(m, "next_seq", st["next_seq"]),
                       setattr(m, "beings", {b: [_ex_in(t) for t in h] for b, h in st["beings"].items()})),
    lim="limits", run="run"))

# ------------------------------------------------------------------ stability (pass 2 D1)

_ST_MAX_TURN = 1_000_000_000_000
_ST_MAX_TOTAL = 1_000_000_000
_ST_CONFIG = {"critical": ("u", 1, 100), "dither_margin": ("u", 0, 100), "dither_switches": ("u", 1, 31),
              "herd_min": ("u", 2, 64), "herd_pct": ("u", 51, 100), "runaway_floor": ("u", 0, _ST_MAX_TOTAL),
              "runaway_turns": ("u", 1, 64), "spiral_turns": ("u", 1, 64), "window": ("u", 2, 32)}
_ST_WATCH = {"history": ("list", ("tup", "s", "u")), "last_severity": ("opt", ("u", 0, 100)),
             "spiral_streak": ("u", 0, _ST_MAX_TURN)}
_ST_TOTAL = {"last": ("opt", ("u", 0, _ST_MAX_TOTAL)), "streak": ("u", 0, _ST_MAX_TURN)}


def _st_get(m):
    tw = m.totals_watch
    return {"beings": {b: {"history": w["history"], "last_severity": w["last"], "spiral_streak": w["streak"]}
                       for b, w in m.watch.items()},
            "config": m.config, "grudges": tw["grudge"], "last_turn": m.last_turn, "last_turn_kinds": m.last_kinds,
            "next_warning": m.next_warning, "rumors": tw["rumor"], "totals": m.totals,
            "turns_audited": m.turns_audited, "warnings": [dict(sorted(w.items())) for w in m.log]}


def _st_put(m, st):
    m.watch = {b: {"history": list(w["history"]), "streak": w["spiral_streak"], "last": w["last_severity"]}
               for b, w in st["beings"].items()}
    m.config = dict(st["config"])
    m.totals_watch = {"grudge": dict(st["grudges"]), "rumor": dict(st["rumors"])}
    m.last_turn, m.turns_audited, m.next_warning = st["last_turn"], st["turns_audited"], st["next_warning"]
    m.last_kinds = sorted(st["last_turn_kinds"])
    m.totals, m.log = st["totals"], st["warnings"]


def _st_number(x):
    if not isinstance(x, str) or not x.startswith("w_"):
        return None
    n = x[2:]
    if not n or not all(ch in "0123456789" for ch in n) or n.startswith("0"):
        return None
    return int(n)


def _st_settle(m) -> None:
    k = KIND_STAB
    cfg = m.config
    need(cfg["dither_switches"] < cfg["window"] and (m.last_turn is not None) == (m.turns_audited > 0))
    k.within("tracked_beings", len(m.watch))
    for b, w in m.watch.items():
        need(is_ident(b) and 0 < len(w["history"]) <= cfg["window"])
        need(all(is_ident(mo) and mg <= 100 for mo, mg in w["history"]) and w["last"] is not None)
        need(w["streak"] <= m.turns_audited)
    need(all(x["streak"] <= m.turns_audited for x in m.totals_watch.values()))
    need(all(x in es_ref.KINDS for x in m.last_kinds))
    need(all(x in es_ref.KINDS and n >= 1 for x, n in m.totals.items()))
    need(sum(m.totals.values()) + 1 == m.next_warning)
    k.within("warnings", len(m.log))
    last_id = last_turn = 0
    for w in m.log:
        need(isinstance(w, dict) and strictly_sorted(list(w)))
        kind, wid, turn = w.get("kind"), _st_number(w.get("id")), w.get("turn")
        need(isinstance(kind, str) and wid is not None and isinstance(turn, int) and not isinstance(turn, bool))
        need(0 <= turn <= _ST_MAX_TURN and kind in es_ref.KINDS and last_id < wid < m.next_warning
             and turn >= last_turn)
        need(m.last_turn is not None and turn <= m.last_turn)
        last_id, last_turn = wid, turn


KIND_STAB = register(Kind(
    "stability", es_ref.StabilityReference, [("tracked_beings", 256, 1_000_000), ("audit_beings", 64, 4096),
                                             ("warnings", 256, 65_536)],
    {"beings": ("map", "s", _ST_WATCH), "config": _ST_CONFIG, "grudges": _ST_TOTAL,
     "last_turn": ("opt", ("u", 0, _ST_MAX_TURN)), "last_turn_kinds": ("set", "s"),
     "next_warning": ("u", 1, _ST_MAX_TURN), "rumors": _ST_TOTAL, "totals": ("map", "s", "u"),
     "turns_audited": ("u", 0, _ST_MAX_TURN), "warnings": ("list", "any")},
    _st_settle, get=_st_get, put=_st_put, lim="limits", run="run"))

# ------------------------------------------------------------------ beliefs and speech acts (pass 2 D2)

import beliefs_speech_reference as bs_ref

_BS_BELIEF = {"about": "s", "at": ("u", 0, bs_ref.MAX_AT), "confidence": ("u", 0, 100), "from": ("opt", "s"),
              "holds": "b", "relation": "s", "source": "s", "value": "s"}
_BS_STORE = {"beings": ("list", ("tup", "s", ("list", _BS_BELIEF)))}
_BS_SPEC = [("beings", 64, 1_000_000), ("beliefs", 128, 4096)]


def _bs_out(beings):
    rows = []
    for b in sorted(beings, key=lambda x: x.encode()):
        m = beings[b]
        rows.append((b, [{"about": t[0], "relation": t[1], "value": t[2], **m[t]}
                         for t in sorted(m, key=lambda t: tuple(x.encode() for x in t))]))
    return {"beings": rows}


def _bs_in(st):
    """Rows must already be in canonical order (beings by name, beliefs by triple), with no repeats."""
    need(strictly_sorted([b for b, _ in st["beings"]]))
    out = {}
    for b, xs in st["beings"]:
        need(strictly_sorted([(x["about"], x["relation"], x["value"]) for x in xs]))
        m = {}
        for x in xs:
            m[(x["about"], x["relation"], x["value"])] = {k: x[k] for k in ("at", "confidence", "from", "holds",
                                                                              "source")}
        out[b] = m
    return out


def _bs_check(beings) -> None:
    ceiling = dict((n, c) for n, _, c in _BS_SPEC)
    need(len(beings) <= ceiling["beings"])
    for b, m in beings.items():
        need(is_ident(b) and len(m) <= ceiling["beliefs"])
        for t, x in m.items():
            need(all(is_ident(p) for p in t) and len(bs_ref.key(t)) <= 64)
            need(bs_ref.rank(x["source"]) > 0 and (x["from"] is None or is_ident(x["from"])))


def _bs_settle(m) -> None:
    _bs_check(m.beings)


KIND_BELIEFS = register(Kind(
    "beliefs", bs_ref.BeliefsRef, _BS_SPEC, _BS_STORE, _bs_settle,
    get=lambda m: _bs_out(m.beings), put=lambda m, st: setattr(m, "beings", _bs_in(st)),
    lim="limits", run="run"))

_SP_SPEC = [("pairs", 256, 1_000_000), ("want_beings", 64, 1_000_000), ("wants", 32, 1024),
            ("beings", 64, 1_000_000), ("beliefs", 128, 4096)]
_SP_SCHEMA = {"beliefs": _BS_STORE,
              "pairs": ("map", ("tup", "s", "s"), {"authority": ("u", 0, 100), "trust": ("u", 0, 100)}),
              "wants": ("map", "s", ("map", "s", {"from": "s", "priority": ("u", 0, 100)}))}


def _sp_get(m):
    return {"beliefs": _bs_out(m.beliefs.beings), "pairs": m.pairs, "wants": m.wants}


def _sp_put(m, st):
    m.beliefs.beings = _bs_in(st["beliefs"])
    m.pairs = {tuple(k): dict(v) for k, v in st["pairs"].items()}
    m.wants = st["wants"]


def _sp_settle(m) -> None:
    k = KIND_SPEECH
    _bs_check(m.beliefs.beings)
    k.within("pairs", len(m.pairs))
    k.within("want_beings", len(m.wants))
    for s, l in m.pairs:
        need(is_ident(s) and is_ident(l) and s != l)
    for holder, ws in m.wants.items():
        need(is_ident(holder) and len(ws) > 0)
        k.within("wants", len(ws))
        for goal, w in ws.items():
            need(is_ident(goal) and is_ident(w["from"]) and w["from"] != holder)


KIND_SPEECH = register(Kind("speech_act", bs_ref.SpeechRef, _SP_SPEC, _SP_SCHEMA, _sp_settle,
                            get=_sp_get, put=_sp_put, lim="limits", run="run"))

# ------------------------------------------------------------------ diet (pass 2 D3)

import diet_reference as diet_ref

_DI_FOOD = {"cooked_bonus": ("u", 0, 100), "hunger": ("u", 0, 100), "id": "s", "joy": ("i", -20, 20), "label": "s",
            "preserved_spoils_after": ("u", 0, diet_ref.MAX_SHELF), "raw_risk": ("u", 0, 100),
            "spoils_after": ("u", 0, diet_ref.MAX_SHELF), "tags": ("set", "s"), "thirst": ("u", 0, 100)}
_DI_PROFILE = {"allergies": ("set", "s"), "diet": "s", "dislikes": ("set", "s"), "forbids": ("set", "s"),
               "likes": ("set", "s"), "meals": "u", "recent": ("list", ("tup", "s", "u"))}
_DI_SETS = ("allergies", "dislikes", "forbids", "likes")


def _di_put(m, st):
    m.foods = {k: {**f, "tags": sorted(f["tags"], key=str.encode)} for k, f in st["foods"].items()}
    m.beings = {b: {**p, **{k: sorted(p[k], key=str.encode) for k in _DI_SETS}, "recent": list(p["recent"])}
                for b, p in st["beings"].items()}


def _di_settle(m) -> None:
    k = KIND_DIET
    k.within("foods", len(m.foods))
    k.within("beings", len(m.beings))
    tags_ok = lambda xs: all(t in diet_ref.FOOD_TAGS for t in xs)  # noqa: E731
    for fid, f in m.foods.items():
        need(fid == f["id"] and is_ident(fid) and is_text(f["label"], 48))
        need(0 < len(f["tags"]) <= 8 and tags_ok(f["tags"]))
    for b, p in m.beings.items():
        need(is_ident(b) and p["diet"] in diet_ref.DIETS)
        need(tags_ok(p["forbids"]) and tags_ok(p["likes"]) and tags_ok(p["dislikes"])
             and not set(p["likes"]) & set(p["dislikes"]))
        need(len(p["allergies"]) <= 8 and all(is_ident(a) for a in p["allergies"]))
        k.within("recent", len(p["recent"]))
        need(len(p["recent"]) <= p["meals"] <= MAX_AT)
        need(all(f in m.foods and at <= MAX_AT for f, at in p["recent"]))


KIND_DIET = register(Kind("diet", diet_ref.DietRef, [("foods", 256, 65_536), ("beings", 64, 1_000_000),
                                                     ("recent", 16, 256)],
                          {"beings": ("map", "s", _DI_PROFILE), "foods": ("map", "s", _DI_FOOD)}, _di_settle,
                          put=_di_put, lim="limits", run="run"))

# ------------------------------------------------------------------ creed (pass 2 D3)

import creed_reference as creed_ref

_CR_TENET = {"action": ("opt", "s"), "food_tag": ("opt", "s"), "id": "s", "label": "s", "sacred_only": "b",
             "severity": ("u", 1, 100)}
_CR_OBS = {"action": "s", "every": ("u", 1, creed_ref.MAX_PERIOD), "id": "s", "label": "s",
           "offset": ("u", 0, creed_ref.MAX_PERIOD), "severity": ("u", 1, 100), "window": ("u", 1, creed_ref.MAX_PERIOD)}
_CR_CREED = {"id": "s", "kind": "s", "label": "s", "observances": ("list", _CR_OBS), "rivals": ("map", "s", "s"),
             "sacred": ("set", "s"), "tenets": ("list", _CR_TENET)}
_CR_ADH = {"creed": ("opt", "s"), "devotion": ("u", 0, 100), "joined_at": ("u", 0, MAX_AT),
           "norms": ("list", _CR_TENET), "observed": ("map", "s", "u"), "settled": ("map", "s", "u"),
           "standing": ("map", "s", "u")}


def _cr_tenet_ok(t, allow_sacred: bool) -> bool:
    return (is_ident(t["id"]) and is_text(t["label"], 80) and (t["action"] is None) != (t["food_tag"] is None)
            and (t["action"] is None or is_ident(t["action"]))
            and (t["food_tag"] is None or t["food_tag"] in diet_ref.FOOD_TAGS)
            and (not t["sacred_only"] or (allow_sacred and t["action"] is not None)))


def _distinct(xs) -> bool:
    xs = list(xs)
    return len(set(xs)) == len(xs)


def _cr_put(m, st):
    m.creeds = {k: {**c, "sacred": sorted(c["sacred"], key=str.encode)} for k, c in st["creeds"].items()}
    m.beings = st["beings"]


def _cr_settle(m) -> None:
    k = KIND_CREED
    k.within("creeds", len(m.creeds))
    k.within("beings", len(m.beings))
    for cid, cr in m.creeds.items():
        need(cid == cr["id"] and is_ident(cid) and is_text(cr["label"], 80) and cr["kind"] in creed_ref.CREED_KINDS)
        need(len(cr["tenets"]) <= 16 and all(_cr_tenet_ok(t, True) for t in cr["tenets"]))
        need(_distinct(t["id"] for t in cr["tenets"]) and _distinct(o["id"] for o in cr["observances"]))
        need(len(cr["observances"]) <= 8)
        for o in cr["observances"]:
            need(is_ident(o["id"]) and len(o["id"]) <= 56 and is_ident(o["action"]) and is_text(o["label"], 80))
            need(o["window"] <= o["every"] and o["offset"] < o["every"])
        need(len(cr["sacred"]) <= 16 and all(is_ident(s) for s in cr["sacred"]))
        need(len(cr["rivals"]) <= 8)
        need(all(is_ident(r) and r != cid and s in creed_ref.STANCES for r, s in cr["rivals"].items()))
    for b, a in m.beings.items():
        need(is_ident(b) and (a["creed"] is None or a["creed"] in m.creeds))
        need(a["creed"] is not None or (a["devotion"] == 0 and not a["observed"] and not a["settled"]))
        need(len(a["norms"]) <= 16 and all(_cr_tenet_ok(t, False) for t in a["norms"]))
        need(_distinct(t["id"] for t in a["norms"]))
        need(all(cr in m.creeds and v <= 100 for cr, v in a["standing"].items()))
        need(all(is_ident(o) and len(o) <= 56 and p <= MAX_AT
                 for o, p in list(a["observed"].items()) + list(a["settled"].items())))


KIND_CREED = register(Kind("creed", creed_ref.CreedRef, [("creeds", 32, 4096), ("beings", 128, 1_000_000)],
                           {"beings": ("map", "s", _CR_ADH), "creeds": ("map", "s", _CR_CREED)}, _cr_settle,
                           put=_cr_put, lim="limits", run="run"))

# ------------------------------------------------------------------ expectation (pass 2 D3)

import expect_attend_reference as ea_ref

_EX_EXP = {"about": "s", "confidence": ("u", 1, 100), "due": ("u", 0, MAX_AT), "dread": "b", "event": "s",
           "grace": ("u", 0, ea_ref.MAX_GRACE), "importance": ("u", 1, 100), "label": "s", "relies": "b",
           "state": ("in", ["open", "worried", "broken"])}


def _xp_settle(m) -> None:
    k = KIND_EXPECT
    k.within("beings", len(m.beings))
    for b, mind in m.beings.items():
        need(is_ident(b))
        k.within("expectations", len(mind["exp"]))
        k.within("subjects", len(mind["rel"]))
        for eid, e in mind["exp"].items():
            need(is_ident(eid) and is_ident(e["about"]) and is_ident(e["event"])
                 and (not e["dread"] or e["state"] == "open"))
            need(is_text(e["label"], 80) or e["label"] == f"{e['about']} {e['event']}")
        need(all(is_ident(a) and kb[0] + kb[1] > 0 for a, kb in mind["rel"].items()))


KIND_EXPECT = register(Kind(
    "expectation", ea_ref.ExpectRef, [("beings", 64, 1_000_000), ("expectations", 32, 1024), ("subjects", 64, 4096)],
    {"beings": ("map", "s", {"expectations": ("map", "s", _EX_EXP), "reliability": ("map", "s", ("tup", "u", "u"))})},
    _xp_settle,
    get=lambda m: {"beings": {b: {"expectations": x["exp"], "reliability": {a: tuple(v) for a, v in x["rel"].items()}}
                              for b, x in m.beings.items()}},
    put=lambda m, st: setattr(m, "beings", {b: {"exp": x["expectations"],
                                                "rel": {a: list(v) for a, v in x["reliability"].items()}}
                                            for b, x in st["beings"].items()}),
    lim="limits", run="run"))

# ------------------------------------------------------------------ attention (pass 2 D3)

_AT_PROFILE = {"alertness": ("u", 0, 100), "capacity": ("u", 1, ea_ref.MAX_EVENTS), "cares": ("set", "s"),
               "perception": ("u", 0, 100)}


def _at_settle(m) -> None:
    KIND_ATTEND.within("profiles", len(m.profiles))
    for b, p in m.profiles.items():
        need(is_ident(b) and len(p["cares"]) <= 16 and all(is_ident(x) for x in p["cares"]))


KIND_ATTEND = register(Kind(
    "attention", ea_ref.AttendRef, [("profiles", 64, 1_000_000), ("notice_beings", 64, 4096), ("notice_events", 16, 256)],
    {"profiles": ("map", "s", _AT_PROFILE)}, _at_settle,
    put=lambda m, st: setattr(m, "profiles", {b: {**p, "cares": sorted(p["cares"], key=str.encode)}
                                              for b, p in st["profiles"].items()}),
    lim="limits", run="run"))

# ------------------------------------------------------------------ aspiration (pass 2 A4)

import growth_reference as gr_ref

_AS_PRESSURE = {"count": ("u", 1, MAX_AT), "last_at": ("u", 0, MAX_AT), "total": ("u", 1, 1000)}
_AS_GOAL = {"action": "s", "formed_at": ("u", 0, MAX_AT), "label": "s", "origin": ("in", ["proposal", "template"]),
            "priority": ("u", 1, 90), "reason": "s", "target": ("opt", "s")}


def _as_key_ok(k: str) -> bool:
    kind, sep, s = k.partition(":")
    return sep == ":" and kind in gr_ref.KINDS and is_ident(s) and len(s) <= 48


def _as_goal_ok(gid: str, g) -> bool:
    if not is_ident(gid):
        return False
    if g["origin"] == "template":
        subject = g["target"]
        if subject is None or not is_ident(subject) or len(subject) > 48:
            return False
        for kind, (gfmt, lfmt, action) in gr_ref.TEMPLATES.items():
            suffix = f" {kind} experiences about {subject}"
            n = g["reason"][:-len(suffix)] if g["reason"].endswith(suffix) else None
            if (gfmt.format(subject) == gid and lfmt.format(subject) == g["label"] and action == g["action"]
                    and n is not None and n.isascii() and n.isdigit() and str(int(n)) == n and int(n) >= 1):
                return True
        return False
    r = g["reason"]
    cites = r[len("proposed from "):].split(", ") if r.startswith("proposed from ") else []
    return (is_text(g["label"], 80) and is_ident(g["action"]) and (g["target"] is None or is_ident(g["target"]))
            and 1 <= len(cites) <= 4 and len(set(cites)) == len(cites) and all(_as_key_ok(k) for k in cites))


def _as_settle(m) -> None:
    k = KIND_ASPIRE
    k.within("beings", len(m.beings))
    for b, d in m.beings.items():
        need(is_ident(b))
        k.within("pressures", len(d["pressures"]))
        k.within("goals", len(d["goals"]))
        need(all(_as_key_ok(key) and p["total"] <= 100 * p["count"] for key, p in d["pressures"].items()))
        need(all(_as_goal_ok(gid, g) for gid, g in d["goals"].items()))


KIND_ASPIRE = register(Kind("aspiration", gr_ref.AspireRef, gr_ref.ASP_LIMITS,
                            {"beings": ("map", "s", {"goals": ("map", "s", _AS_GOAL),
                                                     "pressures": ("map", "s", _AS_PRESSURE)})},
                            _as_settle, lim="limits", run="run"))

# ------------------------------------------------------------------ habit (pass 2 A4)

_HA_ENTRY = {"action": "s", "habit": "b", "history": ("list", ("tup", ("u", 0, MAX_AT), "i", "s")), "slot": "s",
             "strength": ("u", 0, 100)}


def _ha_entry_ok(eid: str, e) -> bool:
    floor = 1 if e["habit"] else 30
    return (is_ident(eid) and is_ident(e["slot"]) and len(e["slot"]) <= 24 and is_ident(e["action"])
            and len(e["action"]) <= 32 and e["strength"] >= floor
            and (eid == f"habit_{e['slot']}_{e['action']}" if e["habit"] else not eid.startswith("habit_"))
            and len(e["history"]) <= 8
            and all(d != 0 and -100 <= d <= 100 and is_text(r, 80) for _, d, r in e["history"]))


def _ha_settle(m) -> None:
    k = KIND_HABIT
    k.within("beings", len(m.beings))
    for b, r in m.beings.items():
        need(is_ident(b))
        k.within("entries", len(r["entries"]))
        k.within("slots", len(r["tallies"]))
        need(all(_ha_entry_ok(eid, e) for eid, e in r["entries"].items()))
        pairs = [(e["slot"], e["action"]) for e in r["entries"].values() if not e["habit"]]
        need(len(set(pairs)) == len(pairs))
        need(all(is_ident(s) and len(s) <= 24 and is_ident(a) and len(a) <= 32 and 1 <= n <= MAX_AT
                 for s, (a, n) in r["tallies"].items()))


def _ha_put(m, st):
    m.beings = {b: {"entries": {k: {**e, "history": list(e["history"])} for k, e in r["entries"].items()},
                    "tallies": dict(r["tallies"])}
                for b, r in st["beings"].items()}


KIND_HABIT = register(Kind("habit", gr_ref.HabitRef, gr_ref.HAB_LIMITS,
                           {"beings": ("map", "s", {"entries": ("map", "s", _HA_ENTRY),
                                                    "tallies": ("map", "s", ("tup", "s", "u"))})},
                           _ha_settle, put=_ha_put, lim="limits", run="run"))

# ------------------------------------------------------------------ lesson (pass 2 A4)

_LE_TOPIC = {"episodes": ("list", {"at": ("u", 0, MAX_AT), "good": "b", "id": "s"}), "last_at": ("u", 0, MAX_AT),
             "verdict": ("opt", ("in", ["avoid", "seek"]))}


def _le_settle(m) -> None:
    k = KIND_LESSON
    k.within("beings", len(m.beings))
    for b, topics in m.beings.items():
        need(is_ident(b))
        k.within("topics", len(topics))
        for (about, activity), t in topics.items():
            need(is_ident(about) and is_ident(activity) and len(t["episodes"]) > 0)
            k.within("episodes", len(t["episodes"]))
            ids = [e["id"] for e in t["episodes"]]
            need(all(is_ident(i) for i in ids) and len(set(ids)) == len(ids))
            need(all(e["at"] <= t["last_at"] for e in t["episodes"]))
            need(t["verdict"] == gr_ref.verdict_of(t["episodes"]))


KIND_LESSON = register(Kind("lesson", gr_ref.LessonRef, gr_ref.LES_LIMITS,
                            {"beings": ("map", "s", ("map", ("tup", "s", "s"), _LE_TOPIC))},
                            _le_settle, lim="limits", run="run"))

# ------------------------------------------------------------------ cooperation (pass 2 B2/B3)

import cooperation_reference as coop_ref

_CO_ROLE = {"being": ("opt", "s"), "done": "b", "via": ("opt", "s")}
_CO_TASK = {"goal": "s", "organizer": "s", "roles": ("map", "s", _CO_ROLE)}
_CO_PROMISE = {"due": ("u", 0, MAX_AT), "from": "s",
               "status": ("in", ["pending", "kept", "kept_late", "released", "broken"]), "to": "s", "what": "s"}
_CO_WAIT = {"at": ("u", 0, MAX_AT), "being": "s", "seq": ("u", 1, MAX_AT), "status": ("u", 0, 100), "turn": "b",
            "urgency": ("u", 0, 100)}
_CO_RES = {"holder": ("opt", "s"), "queue": ("list", _CO_WAIT), "since": ("u", 0, MAX_AT)}
_CO_GRUDGE = {"last": ("u", 0, MAX_AT), "reason": "s", "weight": ("u", 1, coop_ref.M)}
_CO_REASONS = ["broken_promise", "gave_way", "left_task", "refused"]


def _co_get(m):
    return {"clock": m.clock, "debts": m.debts, "promises": m.promises, "resources": m.resources, "seq": m.seq,
            "tasks": m.tasks,
            "grudges": {h: {a: {"last": g[1], "reason": g[2], "weight": g[0]} for a, g in book.items()}
                        for h, book in m.grudges.items()}}


def _co_put(m, st):
    m.clock, m.seq, m.debts, m.promises, m.tasks = st["clock"], st["seq"], st["debts"], st["promises"], st["tasks"]
    m.resources = st["resources"]
    m.grudges = {h: {a: [g["weight"], g["last"], g["reason"]] for a, g in book.items()}
                 for h, book in st["grudges"].items()}


def _co_settle(m) -> None:
    k = KIND_COOP
    for name, xs in (("tasks", m.tasks), ("promises", m.promises), ("resources", m.resources),
                     ("beings", m.grudges), ("debts", m.debts)):
        k.within(name, len(xs))
    for tid, t in m.tasks.items():
        need(is_ident(tid) and is_ident(t["organizer"]) and is_text(t["goal"], 64) and len(t["roles"]) > 0)
        k.within("roles", len(t["roles"]))
        held = set()
        for rid, r in t["roles"].items():
            b, v = r["being"], r["via"]
            need(is_ident(rid) and (b is None or (is_ident(b) and b not in held)))
            if b is not None:
                held.add(b)
            need((not r["done"] or b is not None) and (v is None or (is_ident(v) and v != b)))
            need(v is None or b is not None)
    for pid, p in m.promises.items():
        need(is_ident(pid) and is_ident(p["from"]) and is_ident(p["to"]) and is_ident(p["what"])
             and p["from"] != p["to"])
    for rid, r in m.resources.items():
        h = r["holder"]
        need(is_ident(rid) and (h is None or is_ident(h)) and (h is not None or not r["queue"]))
        k.within("queue", len(r["queue"]))
        names = [w["being"] for w in r["queue"]]
        need(all(is_ident(x) and x != h for x in names) and len(set(names)) == len(names))
        seqs = [w["seq"] for w in r["queue"]]
        need(all(a < b for a, b in zip(seqs, seqs[1:])) and all(s <= m.seq for s in seqs))
    for holder, book in m.grudges.items():
        need(is_ident(holder) and len(book) > 0)
        k.within("grudges", len(book))
        need(all(is_ident(a) and a != holder and g[2] in _CO_REASONS for a, g in book.items()))
    for (debtor, creditor), n in m.debts.items():
        need(is_ident(debtor) and is_ident(creditor) and debtor != creditor and 1 <= n <= MAX_AT)
        need((creditor, debtor) not in m.debts)


KIND_COOP = register(Kind("cooperation", coop_ref.Cooperation, coop_ref.SPEC,
                          {"clock": ("u", 0, MAX_AT), "debts": ("map", ("tup", "s", "s"), ("u", 0, MAX_INT)),
                           "grudges": ("map", "s", ("map", "s", _CO_GRUDGE)),
                           "promises": ("map", "s", _CO_PROMISE), "resources": ("map", "s", _CO_RES),
                           "seq": ("u", 0, MAX_AT), "tasks": ("map", "s", _CO_TASK)},
                          _co_settle, get=_co_get, put=_co_put))

# ------------------------------------------------------------------ cognition (pass 2 A7/A8)

import cognition_reference as cog_ref

_CG_PROFILE = {"horizon": ("u", 1, 10_000), "impulsiveness": ("u", 0, 100), "memory_span": ("u", 1, 1_000_000),
               "plan_depth": ("u", 1, 8), "slip": ("u", 0, cog_ref.M), "temperament": "s"}
_CG_MEMORY = {"about": "s", "day": ("u", 0, MAX_AT), "importance": ("u", 0, 100), "last": ("u", 0, MAX_AT),
              "people": ("list", "s"), "strength": ("u", 1, cog_ref.M), "what": "s"}


def _cg_settle(m) -> None:
    k = KIND_COG
    for b, p in m.profiles.items():
        need(is_ident(b) and p["temperament"] in cog_ref.TEMPERAMENTS)
    for b, book in m.memories.items():
        need(is_ident(b) and len(book) > 0)
        k.within("memories", len(book))
        for mid, x in book.items():
            need(is_ident(mid) and is_ident(x["about"]) and is_ident(x["what"]))
            k.within("people", len(x["people"]))
            need(all(is_ident(p) for p in x["people"]) and len(set(x["people"])) == len(x["people"]))
    k.within("beings", len(set(m.profiles) | set(m.memories)))


KIND_COG = register(Kind("cognition", cog_ref.Cognition, cog_ref.SPEC,
                         {"clock": ("u", 0, MAX_AT), "memories": ("map", "s", ("map", "s", _CG_MEMORY)),
                          "profiles": ("map", "s", _CG_PROFILE)}, _cg_settle))

# ------------------------------------------------------------------ chatter (pass 2 B4)

import chatter_reference as chat_ref

_CH2_KNOWN = {"secret": "b", "value": "s", "weight": ("u", 0, 100)}
_CH2_TURN = {"kind": ("in", ["dispute", "small_talk", "tell"]), "speaker": "s", "topic": ("opt", "s"),
             "value": ("opt", "s")}
_CH2_RECORD = {"a": "s", "at": ("u", 0, MAX_AT), "b": "s", "first": "s", "id": ("u", 1, MAX_AT),
               "outcome": ("in", ["alliance", "gossip", "quarrel", "small_talk"]), "turns": ("list", _CH2_TURN),
               "warmth_after": ("i", -100, 100), "warmth_before": ("i", -100, 100)}


def _ch2_record_ok(r) -> bool:
    second = r["b"] if r["first"] == r["a"] else r["a"]
    if not (is_ident(r["a"]) and is_ident(r["b"]) and r["a"] != r["b"] and r["first"] in (r["a"], r["b"])
            and r["turns"]):
        return False
    for i, t in enumerate(r["turns"]):
        talk = t["kind"] == "small_talk"
        if t["speaker"] != (r["first"] if i % 2 == 0 else second):
            return False
        for f in ("topic", "value"):
            if (t[f] is None) != talk or (t[f] is not None and not is_ident(t[f])):
                return False
    return True


def _ch2_settle(m) -> None:
    k = KIND_CHATTER
    k.within("beings", len(m.minds))
    k.within("bonds", len(m.bonds))
    k.within("records", len(m.records))
    for b, x in m.minds.items():
        need(is_ident(b) and (x["knows"] or x["wants"]))
        k.within("topics", len(x["knows"]))
        k.within("wants", len(x["wants"]))
        need(all(is_ident(t) and is_ident(v[0]) and t not in x["wants"] for t, v in x["knows"].items()))
        need(all(is_ident(t) and 1 <= w <= 100 for t, w in x["wants"].items()))
    need(all(is_ident(a) and is_ident(b) and a.encode() < b.encode() and -100 <= w <= 100
             for (a, b), w in m.bonds.items()))
    for r in m.records:
        k.within("turns", len(r["turns"]))
        need(_ch2_record_ok(r) and r["id"] <= m.seq)
    need(all(x["id"] < y["id"] for x, y in zip(m.records, m.records[1:])))


def _ch2_get(m):
    return {"bonds": m.bonds, "records": m.records, "seq": m.seq,
            "minds": {b: {"knows": {t: {"secret": v[2], "value": v[0], "weight": v[1]} for t, v in x["knows"].items()},
                          "wants": x["wants"]} for b, x in m.minds.items()}}


def _ch2_put(m, st):
    m.bonds, m.records, m.seq = st["bonds"], st["records"], st["seq"]
    m.minds = {b: {"knows": {t: [v["value"], v["weight"], v["secret"]] for t, v in x["knows"].items()},
                   "wants": x["wants"]} for b, x in st["minds"].items()}


KIND_CHATTER = register(Kind("chatter", chat_ref.Chatter, chat_ref.SPEC,
                             {"bonds": ("map", ("tup", "s", "s"), ("i", -MAX_INT, MAX_INT)),
                              "minds": ("map", "s", {"knows": ("map", "s", _CH2_KNOWN), "wants": ("map", "s", "u")}),
                              "records": ("list", _CH2_RECORD), "seq": ("u", 0, MAX_AT)},
                             _ch2_settle, get=_ch2_get, put=_ch2_put))

# ------------------------------------------------------------------ survival (pass 2 C1/C2)

import survival_reference as surv_ref

_SV_BEING = {"alive": "b", "belongings": ("list", "s"), "born": ("u", 0, MAX_AT), "cause": ("opt", "s"),
             "children": ("list", "s"), "died": ("opt", ("u", 0, MAX_AT)), "last": ("u", 0, MAX_AT),
             "lifespan": ("u", 1, MAX_AT), "parents": ("list", "s"), "partner": ("opt", "s"),
             "safety": ("u", 0, 100), "stage": ("in", ["child", "adult", "elder"])}


def _sv_distinct(xs) -> bool:
    return all(is_ident(x) for x in xs) and len(set(xs)) == len(xs)


def _sv_settle(m) -> None:
    k = KIND_SURV
    k.within("beings", len(m.beings))
    for b, x in m.beings.items():
        need(is_ident(b) and x["alive"] == (x["died"] is None) and x["alive"] == (x["cause"] is None))
        need(x["cause"] is None or is_ident(x["cause"]))
        k.within("belongings", len(x["belongings"]))
        k.within("children", len(x["children"]))
        need(_sv_distinct(x["belongings"]) and (x["alive"] or not x["belongings"]))
        need(len(x["parents"]) <= 2 and _sv_distinct(x["parents"]) and b not in x["parents"])
        need(all(p in m.beings and b in m.beings[p]["children"] for p in x["parents"]))
        need(_sv_distinct(x["children"]))
        need(all(c in m.beings and b in m.beings[c]["parents"] for c in x["children"]))
        p = x["partner"]
        if p is not None:
            y = m.beings.get(p)
            need(x["alive"] and p != b and y is not None and y["alive"] and y["partner"] == b)


KIND_SURV = register(Kind("survival", surv_ref.Survival, surv_ref.SPEC,
                          {"beings": ("map", "s", _SV_BEING), "clock": ("u", 0, MAX_AT)}, _sv_settle))

# ------------------------------------------------------------------ detail (pass 2 C3)

import detail_reference as det_ref

_DT_NEED = {"level": ("u", 0, det_ref.M), "rate": ("u", 0, det_ref.M), "threshold": ("u", 1, det_ref.M)}
_DT_TRACKED = {"at": ("u", 0, MAX_AT), "detail": ("in", det_ref.LEVELS), "distance": ("u", 0, MAX_AT),
               "group": ("opt", "s"), "needs": ("map", "s", _DT_NEED)}


def _dt_settle(m) -> None:
    k = KIND_DETAIL
    need(m.near <= m.far)
    k.within("beings", len(m.beings))
    groups = set()
    for b, x in m.beings.items():
        need(is_ident(b) and len(x["needs"]) > 0 and all(is_ident(n) for n in x["needs"]))
        k.within("needs", len(x["needs"]))
        need(x["detail"] == m.level_of(x["distance"]) and x["at"] <= m.clock)
        need((x["group"] is not None) == (x["detail"] == "crowd") and (x["group"] is None or is_ident(x["group"])))
        if x["detail"] == "full":
            need(x["at"] == m.clock)
        elif x["detail"] == "coarse":
            need(m.clock - x["at"] < m.stride)
        if x["group"] is not None:
            groups.add(x["group"])
    k.within("groups", len(groups))


KIND_DETAIL = register(Kind("detail", det_ref.Detail, det_ref.SPEC,
                            {"beings": ("map", "s", _DT_TRACKED), "clock": ("u", 0, MAX_AT), "far": ("u", 0, MAX_AT),
                             "near": ("u", 0, MAX_AT), "stride": ("u", 1, 1_000_000)}, _dt_settle))

# ------------------------------------------------------------------ retention (pass 2 C4)

import retention_reference as ret_ref

_RT_ENTRY = {"at": ("u", 0, MAX_AT), "bytes": ("u", 1, 16_777_216), "importance": ("u", 0, 100), "pinned": "b",
             "used": ("u", 0, MAX_AT)}
_RT_RULE = {"grace": ("u", 0, MAX_AT), "max_age": ("opt", ("u", 0, MAX_AT)),
            "max_bytes": ("opt", ("u", 1, ret_ref.MAX_BYTES)), "max_entries": ("opt", ("u", 1, 1_000_000)),
            "min_importance": ("u", 0, 100)}
_RT_FIELDS = {"rename": ["op", "path", "to"], "add": ["op", "path", "value"], "remove": ["op", "path"],
              "clamp": ["max", "min", "op", "path"]}


def _rt_change_out(ch):
    out = {**ch, "path": ".".join(ch["path"])}
    return {k: out[k] for k in _RT_FIELDS[ch["op"]]}


def _rt_change_in(v):
    """A saved change row, checked in full: exact sorted fields, a valid path and bounds."""
    need(isinstance(v, dict) and v.get("op") in _RT_FIELDS and list(v) == _RT_FIELDS[v["op"]])
    out = {**v, "path": ret_ref.parse_path(v["path"])}
    if v["op"] == "rename":
        need(is_ident(v["to"]))
    elif v["op"] == "clamp":
        lo, hi = v["min"], v["max"]
        need(ret_ref.is_int(lo) and ret_ref.is_int(hi) and -ret_ref.MAX_BYTES <= lo <= hi <= ret_ref.MAX_BYTES)
    return {k: out[k] for k in ("op", "path", "to", "value", "min", "max") if k in out}


def _rt_get(m):
    return {"clock": m.clock, "collections": m.cols,
            "migrations": [[_rt_change_out(ch) for ch in mig] for mig in m.migrations]}


def _rt_put(m, st):
    m.clock, m.cols = st["clock"], st["collections"]
    need(isinstance(st["migrations"], list) and all(isinstance(x, list) for x in st["migrations"]))
    m.migrations = [[_rt_change_in(ch) for ch in mig] for mig in st["migrations"]]


def _rt_settle(m) -> None:
    k = KIND_RET
    k.within("collections", len(m.cols))
    k.within("migrations", len(m.migrations))
    for name, col in m.cols.items():
        need(is_ident(name))
        k.within("entries", len(col["entries"]))
        need(all(is_ident(i) and e["at"] <= e["used"] <= m.clock for i, e in col["entries"].items()))
        need(col["bytes"] == sum(e["bytes"] for e in col["entries"].values()))
    for mig in m.migrations:
        need(len(mig) > 0)
        k.within("changes", len(mig))


KIND_RET = register(Kind("retention", ret_ref.Retention, ret_ref.SPEC,
                         {"clock": ("u", 0, MAX_AT),
                          "collections": ("map", "s", {"bytes": "u", "entries": ("map", "s", _RT_ENTRY),
                                                       "rule": _RT_RULE}),
                          "migrations": "any"},
                         _rt_settle, get=_rt_get, put=_rt_put))

# ------------------------------------------------------------------ roles (pass 2 C5)

import roles_reference as roles_ref

_RO_DUTY = {"action": "s", "at": ("u", 0, MAX_AT), "every": ("u", 1, MAX_AT), "place": ("opt", "s"),
            "priority": ("u", 0, 100)}
_RO_ROLE = {"duties": ("map", "s", _RO_DUTY), "seats": ("opt", ("u", 1, 1_000_000)), "status": ("u", 0, 100)}
_RO_HOLDER = {"held": ("map", "s", ("u", 0, MAX_AT)), "standing": ("i", -50, 50)}


def _ro_settle(m) -> None:
    k = KIND_ROLES
    k.within("roles", len(m.roles))
    k.within("beings", len(m.beings))
    for rid, r in m.roles.items():
        need(is_ident(rid))
        k.within("duties", len(r["duties"]))
        need(all(is_ident(d) and is_ident(x["action"]) and (x["place"] is None or is_ident(x["place"]))
                 for d, x in r["duties"].items()))
    for b, x in m.beings.items():
        need(is_ident(b))
        k.within("held", len(x["held"]))
        need(all(r in m.roles and since <= m.clock for r, since in x["held"].items()))


KIND_ROLES = register(Kind("roles", roles_ref.Roles, roles_ref.SPEC,
                           {"beings": ("map", "s", _RO_HOLDER), "clock": ("u", 0, MAX_AT),
                            "roles": ("map", "s", _RO_ROLE)}, _ro_settle))

# ------------------------------------------------------------------ economy (pass 2 C6)

import economy_reference as econ_ref

_EC_GOOD = {"base": ("u", 1, 1_000_000_000), "ceiling": ("u", 1000, 100_000), "elasticity": ("u", 0, 10_000),
            "floor": ("u", 1, 1000), "rate": ("u", 1, 1000), "spoil": ("u", 0, 1000)}
_EC_STALL = {"demand": ("u", 0, MAX_AT), "made": ("map", "s", "u"), "mult": ("i", 1, 100_000),
             "sold": ("u", 0, MAX_AT), "stock": ("u", 0, MAX_AT), "wants": ("map", "s", "u")}


def _ec_settle(m) -> None:
    k = KIND_ECON
    k.within("goods", len(m.goods))
    k.within("markets", len(m.markets))
    need(all(is_ident(g) for g in m.goods))
    for mk, stalls in m.markets.items():
        need(is_ident(mk) and len(stalls) > 0)
        for g, s in stalls.items():
            need(g in m.goods)
            need(all(is_ident(b) for b in list(s["wants"]) + list(s["made"])) and all(n >= 1 for n in s["made"].values()))
            need(sum(s["wants"].values()) <= s["demand"] and sum(s["made"].values()) <= s["stock"] + s["sold"])
            k.within("traders", len(set(s["wants"]) | set(s["made"])))


KIND_ECON = register(Kind("economy", econ_ref.Economy, econ_ref.SPEC,
                          {"clock": ("u", 0, MAX_AT), "goods": ("map", "s", _EC_GOOD),
                           "markets": ("map", "s", ("map", "s", _EC_STALL))}, _ec_settle))

# ------------------------------------------------------------------ space (pass 2 C7)

import space_reference as space_ref

_SP2_SPOT = {"at": ("tup", "i", "i"), "capacity": ("u", 1, 1024), "queue": ("list", "s"), "users": ("list", "s")}
_SP2_ROOM = {"height": ("i", 1, 256), "spots": ("map", "s", _SP2_SPOT), "walls": ("set", ("tup", "i", "i")),
             "width": ("i", 1, 256)}
_SP2_PLACE = {"at": ("tup", "i", "i"), "room": "s"}


def _sp2_get(m):
    return {"beings": {b: {"at": (p["x"], p["y"]), "room": p["room"]} for b, p in m.beings.items()},
            "rooms": {r: {"height": x["h"], "width": x["w"], "walls": set(x["walls"]),
                          "spots": {s: {"at": (sp["x"], sp["y"]), "capacity": sp["capacity"], "queue": sp["queue"],
                                        "users": sp["users"]} for s, sp in x["spots"].items()}}
                      for r, x in m.rooms.items()}}


def _sp2_put(m, st):
    m.beings = {b: {"room": p["room"], "x": p["at"][0], "y": p["at"][1]} for b, p in st["beings"].items()}
    m.rooms = {r: {"w": x["width"], "h": x["height"], "walls": set(x["walls"]),
                   "spots": {s: {"x": sp["at"][0], "y": sp["at"][1], "capacity": sp["capacity"],
                                 "users": list(sp["users"]), "queue": list(sp["queue"])}
                             for s, sp in x["spots"].items()}}
               for r, x in st["rooms"].items()}


def _sp2_settle(m) -> None:
    k = KIND_SPACE
    k.within("rooms", len(m.rooms))
    k.within("beings", len(m.beings))
    for rid, room in m.rooms.items():
        inside = lambda x, y: 0 <= x < room["w"] and 0 <= y < room["h"]  # noqa: E731
        need(is_ident(rid) and len(room["walls"]) <= 4096 and all(inside(x, y) for x, y in room["walls"]))
        k.within("spots", len(room["spots"]))
        for s, sp in room["spots"].items():
            need(is_ident(s) and inside(sp["x"], sp["y"]) and (sp["x"], sp["y"]) not in room["walls"])
            need(len(sp["users"]) <= sp["capacity"])
            k.within("queue", len(sp["queue"]))
            names = sp["users"] + sp["queue"]
            need(all(is_ident(b) for b in names) and len(set(names)) == len(names))
            need(all(b in m.beings and m.beings[b]["room"] == rid for b in names))
    cells = set()
    for b, p in m.beings.items():
        room = m.rooms.get(p["room"])
        need(room is not None and is_ident(b))
        need(0 <= p["x"] < room["w"] and 0 <= p["y"] < room["h"] and (p["x"], p["y"]) not in room["walls"])
        need((p["room"], p["x"], p["y"]) not in cells)
        cells.add((p["room"], p["x"], p["y"]))


KIND_SPACE = register(Kind("space", space_ref.Space, space_ref.SPEC,
                           {"beings": ("map", "s", _SP2_PLACE), "rooms": ("map", "s", _SP2_ROOM)},
                           _sp2_settle, get=_sp2_get, put=_sp2_put))

# ------------------------------------------------------------------ drift (pass 2 C8)

import drift_reference as drift_ref

_DR_SHIFT = {"at": ("u", 0, MAX_AT), "delta": ("i", -100_000, 100_000), "reasons": ("list", "s"),
             "value": ("i", -drift_ref.M, drift_ref.M)}
_DR_TRAIT = {"at": ("u", 0, MAX_AT), "base": ("i", -drift_ref.M, drift_ref.M), "cap": ("u", 0, 300_000),
             "fade": ("u", 0, 100_000), "history": ("list", _DR_SHIFT), "locked": "b",
             "pressure": ("i", -100_000, 100_000), "reasons": ("list", "s"), "step": ("u", 1, 100_000),
             "threshold": ("u", 1, 100_000), "value": ("i", -drift_ref.M, drift_ref.M)}


def _dr_trait_ok(x, clock: int) -> bool:
    lo, hi = max(x["base"] - x["cap"], -drift_ref.M), min(x["base"] + x["cap"], drift_ref.M)
    reasons_ok = lambda rs: len(rs) <= 8 and all(is_ident(r) for r in rs)  # noqa: E731
    h = x["history"]
    return (lo <= x["value"] <= hi and abs(x["pressure"]) < x["threshold"] and x["at"] <= clock
            and reasons_ok(x["reasons"])
            and all(s["delta"] != 0 and abs(s["delta"]) <= x["step"] and s["at"] <= x["at"] and s["reasons"]
                    and reasons_ok(s["reasons"]) for s in h)
            and all(a["at"] <= b["at"] for a, b in zip(h, h[1:])))


def _dr_settle(m) -> None:
    k = KIND_DRIFT
    k.within("beings", len(m.beings))
    for b, mind in m.beings.items():
        need(is_ident(b) and len(mind["traits"]) > 0)
        k.within("traits", len(mind["traits"]))
        for t, x in mind["traits"].items():
            k.within("history", len(x["history"]))
            need(is_ident(t) and _dr_trait_ok(x, m.clock))


KIND_DRIFT = register(Kind("drift", drift_ref.Drift, drift_ref.SPEC,
                           {"beings": ("map", "s", {"locked": "b", "traits": ("map", "s", _DR_TRAIT)}),
                            "clock": ("u", 0, MAX_AT)}, _dr_settle))

# ------------------------------------------------------------------ packs (pass 2 C9)

import json as _json

import packs_reference as packs_ref


def _pk_sorted(v):
    return _json.loads(_json.dumps(v, sort_keys=True))


def _pk_settle(m) -> None:
    need(list(m.custom) == sorted(packs_ref.KINDS))
    count = 0
    for k, packs in m.custom.items():
        for i, p in packs.items():
            need(is_ident(i) and i not in packs_ref.BUILTIN[k])
            need(_json.dumps(p, sort_keys=True) == _json.dumps(p) and packs_ref.READ[k](p) == p)
            count += 1
    KIND_PACKS.within("custom", count)
    m.count = count


KIND_PACKS = register(Kind("packs", packs_ref.Packs, packs_ref.SPEC, {"custom": ("map", "s", ("map", "s", "any"))},
                           _pk_settle,
                           get=lambda m: {"custom": {k: {i: _pk_sorted(p) for i, p in v.items()}
                                                     for k, v in m.custom.items()}},
                           put=lambda m, st: setattr(m, "custom", {k: st["custom"][k] for k in st["custom"]})))
