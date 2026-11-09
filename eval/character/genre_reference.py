"""Independent original reference for genre profiles and lexicon packs (pass 4, stage 1). Python
standard library only; nothing here calls a model.

A genre profile names the words a setting uses for each closed engine list (slot) and a few
setting axes. Every slot keeps the engines' fixed order and arity, so mechanisms and decisions
stay the same and only the words change:

  light     dark, dim, bright                                   (world conditions)
  access    public, customers_only, members_only, private, restricted
  privacy   public, semi_private, private, echoing
  severity  etiquette, custom, contract, taboo, crime           (social norms)
  contract  alibi_pact, apology_owed, employment, favor, keep_secret, loan, promise, trade_order
  address   stranger, acquaintance                              (free text, at most 40 characters)

Four genres are built in (`village` uses the engine words; `orbital_station`, `noir_city` and
`academy` are original settings). `define` adds a genre from a base with some slots renamed;
names are unique within a slot. `render` rewrites an engine output into a genre's words under the
keys the host names; `normalize` rewrites genre words back to engine words and refuses any word
outside the genre's vocabulary, so text from a model or a player never reaches an engine
unchecked. Lexicon packs add phrases for the pragmatic parser's acts (`station_crew`,
`noir_street`, `school_casual` built in); `lexicon_requests` turns packs into `pragmatics`
requests.
"""
import pathlib
import sys
from typing import Any, Dict, List, Tuple

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import ident, keys, need, opt_uint, text
from growth_reference import run_limited
from limits_reference import Limits
from pragmatics_reference import PRECEDENCE, words

SPEC = [("genres", 16, 100_000), ("packs", 16, 100_000), ("nodes", 4096, 1_000_000)]
MAX_DEPTH = 64
SLOTS: List[Tuple[str, List[str]]] = [
    ("light", ["dark", "dim", "bright"]),
    ("access", ["public", "customers_only", "members_only", "private", "restricted"]),
    ("privacy", ["public", "semi_private", "private", "echoing"]),
    ("severity", ["etiquette", "custom", "contract", "taboo", "crime"]),
    ("contract", ["alibi_pact", "apology_owed", "employment", "favor", "keep_secret", "loan", "promise", "trade_order"]),
    ("address", ["stranger", "acquaintance"]),
]
ENGINE = dict(SLOTS)
SLOT_NAMES = [s for s, _ in SLOTS]
AXES = ["tech", "magic", "law_reach", "scarcity"]
CYCLES = ["natural", "artificial", "none"]
BUILTIN_GENRES: Dict[str, dict] = {
    "village": {"axes": {"tech": 20, "magic": 30, "law_reach": 40, "scarcity": 40}, "day_cycle": "natural",
                "vocab": {}},
    "orbital_station": {
        "axes": {"tech": 90, "magic": 0, "law_reach": 70, "scarcity": 65}, "day_cycle": "artificial",
        "vocab": {"light": ["lights_out", "night_cycle", "full_lights"],
                  "access": ["open_deck", "crew_only", "officers_only", "quarters", "sealed"],
                  "privacy": ["common", "alcove", "cabin", "open_channel"],
                  "severity": ["protocol", "regulation", "contract_breach", "mutiny_taboo", "station_offense"],
                  "contract": ["cover_story_pact", "owed_apology", "duty_roster", "favor", "sealed_secret", "ration_loan",
                               "promise", "supply_order"],
                  "address": ["crewmate", "spacer"]}},
    "noir_city": {
        "axes": {"tech": 45, "magic": 0, "law_reach": 50, "scarcity": 55}, "day_cycle": "natural",
        "vocab": {"light": ["dark", "neon", "daylight"],
                  "access": ["street", "patrons_only", "club_members", "private", "police_line"],
                  "privacy": ["street", "booth", "back_room", "echoing"],
                  "severity": ["rude", "bad_form", "double_cross", "unforgivable", "felony"],
                  "contract": ["alibi", "owed_apology", "on_the_payroll", "favor", "keep_mum", "loan", "promise", "job"],
                  "address": ["pal", "friend"]}},
    "academy": {
        "axes": {"tech": 60, "magic": 10, "law_reach": 60, "scarcity": 20}, "day_cycle": "natural",
        "vocab": {"light": ["lights_out", "dim", "bright"],
                  "access": ["public", "visitors_only", "students_only", "private", "staff_only"],
                  "privacy": ["hallway", "study_corner", "dorm_room", "stairwell"],
                  "severity": ["manners", "house_rule", "honor_pledge", "forbidden", "expellable"],
                  "contract": ["cover_story", "owed_apology", "part_time_job", "favor", "keep_secret", "borrowed_notes",
                               "promise", "trade"],
                  "address": ["new kid", "classmate"]}},
}
BUILTIN_PACKS: Dict[str, Dict[str, List[str]]] = {
    "station_crew": {"threat": ["out the airlock"], "accusation": ["you jacked"], "request_help": ["need a hand"],
                     "greeting": ["morning shift", "hey crewmate"], "farewell": ["next shift", "clear skies"],
                     "compliment": ["clean work", "solid work"]},
    "noir_street": {"threat": ["you ll be sorry"], "accusation": ["you sold me out", "rat"],
                    "offer_or_transfer": ["got something for you"], "request_help": ["do me a favor"],
                    "greeting": ["evening pal"], "farewell": ["see you around"], "compliment": ["nice work"]},
    "school_casual": {"threat": ["you ll pay"], "accusation": ["you snitched"], "apology": ["my bad"],
                      "request_help": ["help me out", "can i borrow"], "greeting": ["yo", "sup"],
                      "farewell": ["later", "bye"], "compliment": ["nice", "so cool"]},
}


def full_vocab(vocab: Dict[str, List[str]]) -> Dict[str, List[str]]:
    return {s: list(vocab.get(s, ENGINE[s])) for s in SLOT_NAMES}


def phrase_of(v: Any) -> str:
    s = text(v, 64)
    ws = words(s)
    need(1 <= len(ws) <= 4 and " ".join(ws) == s)
    return s


class Genres:
    def __init__(self) -> None:
        self.lim = Limits(SPEC)
        self.genres: Dict[str, dict] = {}
        self.packs: Dict[str, Dict[str, List[str]]] = {}

    def genre(self, gid: str) -> dict:
        if gid in BUILTIN_GENRES:
            g = BUILTIN_GENRES[gid]
            return {"axes": g["axes"], "day_cycle": g["day_cycle"], "vocab": full_vocab(g["vocab"]), "builtin": True}
        need(gid in self.genres)
        return self.genres[gid]

    def pack(self, pid: str) -> Dict[str, List[str]]:
        if pid in BUILTIN_PACKS:
            return BUILTIN_PACKS[pid]
        need(pid in self.packs)
        return self.packs[pid]

    def op_list(self, a: dict) -> dict:
        keys(a, [], [])
        return {"genres": sorted(list(BUILTIN_GENRES) + list(self.genres)),
                "packs": sorted(list(BUILTIN_PACKS) + list(self.packs))}

    def op_show(self, a: dict) -> dict:
        keys(a, ["id"], [])
        gid = ident(a["id"])
        g = self.genre(gid)
        return {"axes": dict(g["axes"]), "builtin": g["builtin"], "customized": customized(g["vocab"]),
                "day_cycle": g["day_cycle"], "id": gid, "vocab": {s: list(v) for s, v in g["vocab"].items()}}

    def op_define(self, a: dict) -> dict:
        keys(a, ["id"], ["base", "axes", "day_cycle", "vocab"])
        gid = ident(a["id"])
        need(gid not in BUILTIN_GENRES)
        base = self.genre("village" if a.get("base") is None else ident(a["base"]))
        axes = dict(base["axes"])
        ax = a.get("axes")
        if ax is not None:
            keys(ax, [], AXES)
            for k in AXES:
                axes[k] = opt_uint(ax, k, 0, 100, axes[k])
        cycle = base["day_cycle"]
        if a.get("day_cycle") is not None:
            cycle = ident(a["day_cycle"])
            need(cycle in CYCLES)
        vocab = {s: list(v) for s, v in base["vocab"].items()}
        vo = a.get("vocab")
        if vo is not None:
            keys(vo, [], SLOT_NAMES)
            for s in SLOT_NAMES:
                if vo.get(s) is None:
                    continue
                rows = vo[s]
                need(isinstance(rows, list) and len(rows) == len(ENGINE[s]))
                names = [text(x, 40) if s == "address" else ident(x) for x in rows]
                need(len(set(names)) == len(names))
                vocab[s] = names
        need(gid in self.genres or len(self.genres) < self.lim["genres"])
        self.genres[gid] = {"axes": axes, "day_cycle": cycle, "vocab": vocab, "builtin": False}
        return {"customized": customized(vocab), "id": gid}

    def op_pack(self, a: dict) -> dict:
        keys(a, ["id", "phrases"], [])
        pid = ident(a["id"])
        need(pid not in BUILTIN_PACKS)
        ph = a["phrases"]
        need(isinstance(ph, dict) and len(ph) >= 1)
        keys(ph, [], PRECEDENCE)
        out: Dict[str, List[str]] = {}
        seen = set()
        for act in PRECEDENCE:
            if act not in ph:
                continue
            rows = ph[act]
            need(isinstance(rows, list) and 1 <= len(rows) <= 16)
            ps = [phrase_of(p) for p in rows]
            need(all(p not in seen for p in ps) and len(set(ps)) == len(ps))
            seen.update(ps)
            out[act] = ps
        need(pid in self.packs or len(self.packs) < self.lim["packs"])
        self.packs[pid] = out
        return {"acts": [k for k in PRECEDENCE if k in out], "id": pid, "phrases": len(seen)}

    def op_lexicon_requests(self, a: dict) -> dict:
        keys(a, ["packs"], [])
        rows = a["packs"]
        need(isinstance(rows, list) and 1 <= len(rows) <= 8)
        ids = [ident(x) for x in rows]
        need(len(set(ids)) == len(ids))
        ops = []
        for pid in ids:
            p = self.pack(pid)
            for act in PRECEDENCE:
                if act in p:
                    ops.append({"args": {"act": act, "phrases": list(p[act])}, "op": "lexicon"})
        return {"ops": len(ops), "requests": [{"args": {"ops": ops}, "kind": "pragmatics"}]}

    def walk(self, v: Any, mapping: Dict[str, str], table: Dict[str, Dict[str, str]], strict: bool, state: dict,
             depth: int, slot: str = "") -> Any:
        state["nodes"] += 1
        need(state["nodes"] <= self.lim["nodes"] and depth <= MAX_DEPTH)
        if slot and isinstance(v, str):
            t = table[slot]
            if v in t:
                return t[v]
            need(not strict)
            state["unmapped"] += 1
            return v
        if isinstance(v, dict):
            return {k: self.walk(x, mapping, table, strict, state, depth + 1, mapping.get(k, "")) for k, x in v.items()}
        if isinstance(v, list):
            return [self.walk(x, mapping, table, strict, state, depth + 1, slot) for x in v]
        return v

    def translate(self, a: dict, strict: bool) -> dict:
        keys(a, ["genre", "value", "keys"], [])
        gid = ident(a["genre"])
        g = self.genre(gid)
        km = a["keys"]
        need(isinstance(km, dict) and 1 <= len(km) <= 16)
        mapping = {}
        for k, s in km.items():
            ident(k)
            need(isinstance(s, str) and s in ENGINE)
            mapping[k] = s
        if strict:
            table = {s: dict(zip(g["vocab"][s], ENGINE[s])) for s in SLOT_NAMES}
        else:
            table = {s: dict(zip(ENGINE[s], g["vocab"][s])) for s in SLOT_NAMES}
        state = {"nodes": 0, "unmapped": 0}
        out = self.walk(a["value"], mapping, table, strict, state, 0)
        return {"genre": gid, "unmapped": state["unmapped"], "value": out} if not strict else {"genre": gid, "value": out}

    def op_render(self, a: dict) -> dict:
        return self.translate(a, False)

    def op_normalize(self, a: dict) -> dict:
        return self.translate(a, True)

    def workflow(self, request: Any) -> dict:
        table = {"list": self.op_list, "show": self.op_show, "define": self.op_define, "pack": self.op_pack,
                 "lexicon_requests": self.op_lexicon_requests, "render": self.op_render,
                 "normalize": self.op_normalize}

        def f(op: str, args: Any) -> dict:
            need(op in table and isinstance(args, dict))
            return table[op](args)
        return run_limited(request, self.lim, f)


def customized(vocab: Dict[str, List[str]]) -> List[str]:
    return [s for s in SLOT_NAMES if vocab[s] != ENGINE[s]]


def workflow(request: Any) -> dict:
    return Genres().workflow(request)
