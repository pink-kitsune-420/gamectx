"""Independent original reference for durable state (pass 5, stage 1, design `O`). Python standard
library only; nothing here calls a model.

A save is `{"format": "gamectx-state", "kind", "limits", "state", "version"}` with sorted keys and
integers only. Inside `state`, struct fields are sorted by name, maps are arrays of `[key, value]`
pairs in ascending key order, sets are ascending arrays, absent optionals are null and tuples are
arrays. Each kind declares its state as a schema in the small language below; `import` checks the
envelope, every type, closed word and range, the canonical form (the parsed state must save back
to the same document, key order included) and then the kind's own invariants, and only then
replaces the manager. Limits are never taken from a save.

Schema language:
  "u" / ("u", lo, hi)       unsigned integer (0..MAX_INT, or the inclusive range)
  "i" / ("i", lo, hi)       signed integer (|n| <= MAX_INT, or the inclusive range)
  "b"                       boolean
  "s"                       string of at most MAX_TEXT UTF-8 bytes
  "any"                     any JSON value
  ("in", WORDS)             one word of a closed list
  ("opt", T)                null or T
  ("list", T)               array of T
  ("arr", T, n)             array of exactly n T
  ("set", T)                ascending array of distinct T (a Python set or list inside)
  ("map", K, V)             array of [K, V] pairs, ascending by key (a Python dict inside)
  ("tup", T1, T2, ...)      fixed array (a Python tuple inside)
  {"field": T, ...}         struct (a Python dict inside)
"""
import json
import pathlib
import sys
from typing import Any, Callable, Dict, List, Optional, Tuple

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from beliefs_speech_reference import Bad, ident, keys, need, text
from limits_reference import Limits

FORMAT = "gamectx-state"
MAX_SAVE_BYTES = 67_108_864
MAX_TEXT = 4096
MAX_INT = 9_007_199_254_740_991
MAX_OPS = 64


def is_int(v: Any) -> bool:
    return isinstance(v, int) and not isinstance(v, bool)


def compact(v: Any) -> str:
    return json.dumps(v, separators=(",", ":"), ensure_ascii=False)


def depth_ok(v: Any, depth: int = 0) -> bool:
    if depth > 64:
        return False
    if isinstance(v, list):
        return all(depth_ok(x, depth + 1) for x in v)
    if isinstance(v, dict):
        return all(depth_ok(x, depth + 1) for x in v.values())
    return True


def texts_within(v: Any) -> bool:
    """Every string and object key inside v is at most MAX_TEXT UTF-8 bytes."""
    if isinstance(v, str):
        return len(v.encode()) <= MAX_TEXT
    if isinstance(v, list):
        return all(texts_within(x) for x in v)
    if isinstance(v, dict):
        return all(len(k.encode()) <= MAX_TEXT and texts_within(x) for k, x in v.items())
    return True


def size_ok(v: Any, cap: int) -> bool:
    return depth_ok(v) and len(compact(v).encode()) <= cap


# ------------------------------------------------------------------ schema load / save

def load(t: Any, v: Any) -> Any:
    if isinstance(t, dict):
        need(isinstance(v, dict) and set(v) == set(t) and len(v) == len(t))
        return {k: load(t[k], v[k]) for k in t}
    if t == "u":
        need(is_int(v) and 0 <= v <= MAX_INT)
        return v
    if t == "i":
        need(is_int(v) and -MAX_INT <= v <= MAX_INT)
        return v
    if t == "b":
        need(isinstance(v, bool))
        return v
    if t == "s":
        need(isinstance(v, str) and len(v.encode()) <= MAX_TEXT)
        return v
    if t == "any":
        need(size_ok(v, MAX_SAVE_BYTES) and texts_within(v))
        return v
    tag = t[0]
    if tag in ("u", "i"):
        x = load(tag, v)
        need(t[1] <= x <= t[2])
        return x
    if tag == "in":
        need(isinstance(v, str) and v in t[1])
        return v
    if tag == "opt":
        return None if v is None else load(t[1], v)
    if tag == "list":
        need(isinstance(v, list))
        return [load(t[1], x) for x in v]
    if tag == "arr":
        need(isinstance(v, list) and len(v) == t[2])
        return [load(t[1], x) for x in v]
    if tag == "set":
        need(isinstance(v, list))
        return {hashable(load(t[1], x)) for x in v}
    if tag == "map":
        need(isinstance(v, list))
        out = {}
        for row in v:
            need(isinstance(row, list) and len(row) == 2)
            out[hashable(load(t[1], row[0]))] = load(t[2], row[1])
        return out
    if tag == "tup":
        need(isinstance(v, list) and len(v) == len(t) - 1)
        return tuple(load(s, x) for s, x in zip(t[1:], v))
    raise ValueError(f"bad schema {t!r}")


def hashable(x: Any) -> Any:
    return tuple(hashable(y) for y in x) if isinstance(x, (list, tuple)) else x


def order_key(x: Any) -> Any:
    """Rust orders strings by UTF-8 bytes, tuples and arrays element-wise, integers numerically."""
    if isinstance(x, str):
        return (1, x.encode())
    if isinstance(x, (list, tuple)):
        return (2, tuple(order_key(y) for y in x))
    if isinstance(x, bool):
        return (0, int(x))
    return (0, x)


def save(t: Any, x: Any) -> Any:
    if isinstance(t, dict):
        return {k: save(t[k], x[k]) for k in sorted(t)}
    if t in ("u", "i", "b", "s", "any"):
        return x
    tag = t[0]
    if tag in ("u", "i", "in"):
        return x
    if tag == "opt":
        return None if x is None else save(t[1], x)
    if tag in ("list", "arr"):
        return [save(t[1], y) for y in x]
    if tag == "set":
        return [save(t[1], y) for y in sorted(x, key=order_key)]
    if tag == "map":
        return [[save(t[1], k), save(t[2], x[k])] for k in sorted(x, key=order_key)]
    if tag == "tup":
        return [save(s, y) for s, y in zip(t[1:], x)]
    raise ValueError(f"bad schema {t!r}")


# ------------------------------------------------------------------ kinds

class Kind:
    """One durable manager: how to build it, its schema, its state mapping and invariants."""

    def __init__(self, name: str, make: Callable[[], Any], spec: List[Tuple[str, int, int]], schema: Dict[str, Any],
                 settle: Callable[[Any], None], version: int = 1,
                 get: Optional[Callable[[Any], Dict[str, Any]]] = None,
                 put: Optional[Callable[[Any, Dict[str, Any]], None]] = None,
                 migrate: Optional[Callable[[int, Any], Any]] = None, lim: str = "lim",
                 run: str = "workflow") -> None:
        self.name, self.make, self.spec, self.schema, self.settle = name, make, spec, schema, settle
        self.version = version
        self.get = get or (lambda m: {k: getattr(m, k) for k in schema})
        self.put = put or (lambda m, st: [setattr(m, k, v) for k, v in st.items()])
        self.migrate = migrate
        self.lim_attr, self.run_attr = lim, run

    def ceiling(self, name: str) -> int:
        return next(c for n, _, c in self.spec if n == name)

    def within(self, name: str, n: int) -> None:
        need(n <= self.ceiling(name))


KINDS: Dict[str, Kind] = {}


def register(k: Kind) -> Kind:
    KINDS[k.name] = k
    return k


def sorted_limits(kind: Kind, lim: Limits) -> Dict[str, int]:
    return {n: lim.values[n] for n in sorted(n for n, _, _ in kind.spec)}


def export(kind: Kind, m: Any) -> Dict[str, Any]:
    return {"format": FORMAT, "kind": kind.name, "limits": sorted_limits(kind, getattr(m, kind.lim_attr)),
            "state": save(kind.schema, kind.get(m)), "version": kind.version}


def restore(kind: Kind, doc: Any, lim: Limits) -> Tuple[Any, int, Dict[str, int]]:
    need(size_ok(doc, MAX_SAVE_BYTES))
    need(isinstance(doc, dict) and list(doc) == ["format", "kind", "limits", "state", "version"])
    need(doc["format"] == FORMAT and doc["kind"] == kind.name)
    version = doc["version"]
    need(is_int(version) and 1 <= version <= kind.version)
    saved = doc["limits"]
    names = sorted(n for n, _, _ in kind.spec)
    need(isinstance(saved, dict) and list(saved) == names)
    sl = Limits(kind.spec)
    sl.apply(saved)
    state = doc["state"]
    for v in range(version, kind.version):
        need(kind.migrate is not None)
        state = kind.migrate(v, state)
        need(size_ok(state, MAX_SAVE_BYTES))
    native = load(kind.schema, state)
    need(compact(save(kind.schema, native)) == compact(state))
    m = kind.make()
    kind.put(m, native)
    kind.settle(m)
    fresh = Limits(kind.spec)
    fresh.values = dict(lim.values)
    setattr(m, kind.lim_attr, fresh)
    return m, version, sorted_limits(kind, sl)


def op_export(kind: Kind, m: Any, args: Any) -> Dict[str, Any]:
    keys(args, [], [])
    return export(kind, m)


def op_import(kind: Kind, m: Any, args: Any) -> Tuple[Any, Dict[str, Any]]:
    keys(args, ["save"], [])
    fresh, frm, saved = restore(kind, args["save"], getattr(m, kind.lim_attr))
    return fresh, {"from_version": frm, "kind": kind.name, "limits": saved, "version": kind.version}


def run_on(kind: Kind, m: Any, request: Any) -> Tuple[Any, Dict[str, Any]]:
    """Runs a module request with `export` and `import` ops added on manager `m`; every other op
    runs on the module's own reference, one op at a time. Returns the (possibly replaced)
    manager and the response."""
    keys(request, ["ops"], ["limits"])
    supplied = "limits" in request
    if request.get("limits") is not None:
        getattr(m, kind.lim_attr).apply(request["limits"])
    ops = request["ops"]
    need(isinstance(ops, list) and 0 < len(ops) <= MAX_OPS)
    results = []
    for item in ops:
        keys(item, ["op"], ["args"])
        name = text(item["op"], 32)
        args = {} if item.get("args") is None else item["args"]
        if name == "export":
            results.append(op_export(kind, m, args))
        elif name == "import":
            m, out = op_import(kind, m, args)
            results.append(out)
        else:
            one = {"ops": [item]}
            results.append(getattr(m, kind.run_attr)(one)["results"][0])
    out = {"version": 1, "results": results}
    return m, ({"limits": getattr(m, kind.lim_attr).view(), **out} if supplied else out)


def workflow(kind_name: str, request: Any) -> Dict[str, Any]:
    """A fresh manager of `kind_name` running `request`."""
    kind = KINDS[kind_name]
    return run_on(kind, kind.make(), request)[1]


# ------------------------------------------------------------------ the state bundle

BUNDLE_FORMAT = "gamectx-bundle"
BUNDLE_VERSION = 1
BUNDLE_SPEC = [("sections", 64, 64)]


class Bundle:
    """At most one manager per durable kind, run in place and saved together."""

    def __init__(self) -> None:
        self.limits = Limits(BUNDLE_SPEC)
        self.sections: Dict[str, Any] = {}

    def op_run(self, a: Any) -> Dict[str, Any]:
        keys(a, ["kind", "request"], [])
        kind_name = a["kind"]
        need(isinstance(kind_name, str) and kind_name in KINDS)
        kind = KINDS[kind_name]
        if kind_name not in self.sections:
            need(len(self.sections) < self.limits["sections"])
            self.sections[kind_name] = kind.make()
        m, out = run_on(kind, self.sections[kind_name], a["request"])
        self.sections[kind_name] = m
        return out

    def export(self) -> Dict[str, Any]:
        rows = [[k, export(KINDS[k], self.sections[k])] for k in sorted(self.sections, key=str.encode)]
        return {"format": BUNDLE_FORMAT, "sections": rows, "version": BUNDLE_VERSION}

    def restore(self, doc: Any) -> Tuple[Dict[str, Any], Dict[str, Any]]:
        need(size_ok(doc, MAX_SAVE_BYTES))
        need(isinstance(doc, dict) and list(doc) == ["format", "sections", "version"])
        need(doc["format"] == BUNDLE_FORMAT)
        frm = doc["version"]
        need(is_int(frm) and 1 <= frm <= BUNDLE_VERSION)
        rows = doc["sections"]
        need(isinstance(rows, list) and len(rows) <= self.limits["sections"])
        out: Dict[str, Any] = {}
        report = []
        last = None
        for row in rows:
            need(isinstance(row, list) and len(row) == 2 and isinstance(row[0], str))
            kind_name, section = row
            need(last is None or last.encode() < kind_name.encode())
            last = kind_name
            need(kind_name in KINDS)
            kind = KINDS[kind_name]
            current = self.sections[kind_name] if kind_name in self.sections else kind.make()
            m, version, saved = restore(kind, section, getattr(current, kind.lim_attr))
            report.append({"from_version": version, "kind": kind_name, "limits": saved})
            out[kind_name] = m
        return out, {"from_version": frm, "sections": report, "version": BUNDLE_VERSION}

    def op_import(self, a: Any, replace: bool) -> Dict[str, Any]:
        keys(a, ["save"], [])
        sections, report = self.restore(a["save"])
        if replace:
            self.sections = sections
        return report

    def workflow(self, request: Any) -> Dict[str, Any]:
        keys(request, ["ops"], ["limits"])
        supplied = "limits" in request
        if request.get("limits") is not None:
            self.limits.apply(request["limits"])
        ops = request["ops"]
        need(isinstance(ops, list) and 0 < len(ops) <= MAX_OPS)
        results = []
        for item in ops:
            keys(item, ["op"], ["args"])
            name = text(item["op"], 32)
            a = {} if item.get("args") is None else item["args"]
            if name == "run":
                results.append(self.op_run(a))
            elif name == "export":
                keys(a, [], [])
                results.append(self.export())
            elif name in ("import", "inspect"):
                results.append(self.op_import(a, name == "import"))
            else:
                need(False)
        out = {"version": 1, "results": results}
        return {"limits": self.limits.view(), **out} if supplied else out


def is_ident(s: Any) -> bool:
    try:
        ident(s)
        return True
    except Bad:
        return False


def is_text(s: Any, n: int) -> bool:
    try:
        text(s, n)
        return True
    except Bad:
        return False


def strictly_sorted(xs: List[Any]) -> bool:
    return all(order_key(a) < order_key(b) for a, b in zip(xs, xs[1:]))


MAX_AT = 1_000_000_000_000
