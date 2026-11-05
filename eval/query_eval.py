#!/usr/bin/env python3
"""Does letting the model write a query help? Filtering and arithmetic questions are asked
three ways over the same packet: answered directly, as a small SQL SELECT that the game
runs exactly, or as a JSON query with the same meaning. Reuses model_eval's data, API and
spending cap. The queries the models wrote are saved, so `--replay` can check that
`gamectx query` gives byte-identical results to this prototype.

    python3 -I eval/query_eval.py --dry-run
    python3 -I eval/query_eval.py --key-file PATH --cap 0.08
    python3 -I eval/query_eval.py --replay eval/results/query-<run>/results.json
    python3 -I eval/query_eval.py --differential 3000     # free: generated queries
"""

import argparse
import decimal
import json
import math
import os
from pathlib import Path
import re
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import calc_eval  # noqa: E402
import model_eval as m  # noqa: E402

SQL_RULE = (' Do not answer from the data yourself. Reply as {"sql": "<query>"}: one SELECT over one table of '
            "the game data, which the game runs exactly. Supported: SELECT columns, COUNT(*), SUM(x), AVG(x), "
            "MIN(x), MAX(x), where x may be arithmetic on columns; FROM table; WHERE conditions joined by AND "
            "(= != < <= > >= IN); ORDER BY column [DESC]; LIMIT n, applied before aggregates.")
JSON_RULE = (' Do not answer from the data yourself. Reply as {"query": {"table": "<name>", "where": {"<column>": '
             'value or {"<op>": value}}, "sort_by": "<column> or -<column>", "limit": n, "get": "<column>" or '
             '"count" or "sum(x)" or "avg(x)" or "min(x)" or "max(x)"}}, where only table and get are required, '
             "ops are = != > >= < <= in, and x may be arithmetic on columns. The game runs it exactly; limit "
             "applies before aggregates.")
TOOL_RULE = (' Do not answer from the data yourself; use the query action. Reply as '
             '{"action": "query", "args": {"sql": "<query>"}}.')
OUT_TOKENS = 300
KINDS = ("inv_filter", "inv_numeric", "npc_filter", "npc_cross", "whole_table")


class QueryError(Exception):
    pass


# ---------------------------------------------------------------- prototype semantics

TOKEN = re.compile(r"[ \t\r\n]*(?:([0-9]+(?:\.[0-9]+)?)|([A-Za-z_][A-Za-z0-9_]*)|'([^']*)'|\"([^\"]*)\"|(!=|<>|<=|>=|[-+*/%(),=<>;]))")
AGGREGATES = ("count", "sum", "avg", "min", "max")
LIMITS = {"chars": 500, "items": 16, "conditions": 16, "in_values": 64, "nodes": 200, "depth": 32}
HINTS = {
    "or": "OR is not supported; use IN (...) or one query per case",
    "group": "GROUP BY is not supported; use WHERE and one query per group",
    "having": "HAVING is not supported; use WHERE",
    "join": "JOIN is not supported; query one table at a time",
    "union": "UNION is not supported; use one query per table",
    "select": "subqueries are not supported; ORDER BY and LIMIT apply before aggregates",
}


def tokenize(text):
    if len(text) > LIMITS["chars"]:
        raise QueryError(f"query is longer than {LIMITS['chars']} characters")
    out, i = [], 0
    while i < len(text):
        if text[i:].strip(" \t\r\n") == "":
            break
        match = TOKEN.match(text, i)
        if not match:
            raise QueryError(f"unexpected {text[i:].lstrip(' \t\r\n')[0]!r}")
        num, ident, s1, s2, sym = match.groups()
        if num is not None:
            out.append(("num", float(num)))
        elif ident is not None:
            out.append(("id", ident))
        elif s1 is not None or s2 is not None:
            out.append(("str", s1 if s1 is not None else s2))
        else:
            out.append(("sym", "!=" if sym == "<>" else sym))
        i = match.end()
    return out


class Parser:
    def __init__(self, tokens):
        self.t, self.i = tokens, 0
        self.nodes, self.depth, self.in_agg = 0, 0, False

    def node(self, n):
        self.nodes += 1
        if self.nodes > LIMITS["nodes"]:
            raise QueryError("query is too complex")
        return n

    def unexpected(self):
        kind, value = self.peek()
        if kind is None:
            return QueryError("unexpected end of query")
        if kind == "id" and value.lower() in HINTS:
            return QueryError(HINTS[value.lower()])
        return QueryError(f"unexpected {value!r}")

    def peek(self):
        return self.t[self.i] if self.i < len(self.t) else (None, None)

    def keyword(self, word):
        kind, value = self.peek()
        if kind == "id" and value.lower() == word:
            self.i += 1
            return True
        return False

    def sym(self, s):
        if self.peek() == ("sym", s):
            self.i += 1
            return True
        return False

    def expect_keyword(self, word):
        if not self.keyword(word):
            raise QueryError(f"expected {word.upper()}")

    def ident(self):
        kind, value = self.peek()
        if self.peek() == ("sym", "("):
            raise QueryError(HINTS["select"])
        if kind != "id":
            raise QueryError("expected a name")
        self.i += 1
        return value

    # expressions: ("num", x) ("col", name) ("neg", e) ("bin", op, a, b) ("agg", name, e or None) ("fn", name, args)
    def expr(self):
        left = self.term()
        while self.peek() in (("sym", "+"), ("sym", "-")):
            op = self.t[self.i][1]
            self.i += 1
            left = self.node(("bin", op, left, self.term()))
        return left

    def term(self):
        left = self.unary()
        while self.peek() in (("sym", "*"), ("sym", "/"), ("sym", "%")):
            op = self.t[self.i][1]
            self.i += 1
            left = self.node(("bin", op, left, self.unary()))
        return left

    def unary(self):
        if self.sym("-"):
            self.depth += 1
            if self.depth > LIMITS["depth"]:
                raise QueryError("query nests too deeply")
            e = self.node(("neg", self.unary()))
            self.depth -= 1
            return e
        return self.atom()

    def nested(self, parse):
        self.depth += 1
        if self.depth > LIMITS["depth"]:
            raise QueryError("query nests too deeply")
        e = parse()
        self.depth -= 1
        return e

    def atom(self):
        kind, value = self.peek()
        if kind == "num":
            self.i += 1
            return self.node(("num", value))
        if self.sym("("):
            if self.peek()[0] == "id" and self.peek()[1].lower() == "select":
                raise QueryError(HINTS["select"])
            e = self.nested(self.expr)
            if not self.sym(")"):
                raise QueryError("expected )")
            return e
        if self.peek() == ("sym", "*"):
            raise QueryError("SELECT * is not supported; name the columns")
        if kind == "id":
            self.i += 1
            if not self.sym("("):
                return self.node(("col", value))
            name = value.lower()
            if name in AGGREGATES:
                if self.in_agg:
                    raise QueryError("aggregates cannot be nested")
                if name == "count" and self.sym("*"):
                    inner = None
                else:
                    self.in_agg = True
                    inner = self.nested(self.expr)
                    self.in_agg = False
                if not self.sym(")"):
                    raise QueryError(f"{name.upper()} takes one argument")
                return self.node(("agg", name, inner))
            if name == "abs":
                arg = self.nested(self.expr)
                if not self.sym(")"):
                    raise QueryError("ABS takes one argument")
                return self.node(("fn", "abs", [arg]))
            if name == "round":
                arg, places = self.nested(self.expr), 0
                if self.sym(","):
                    kind, places = self.peek()
                    if kind != "num" or not math.isfinite(places) or places != int(places) or places > 12:
                        raise QueryError("ROUND places must be a whole number from 0 to 12")
                    self.i += 1
                if not self.sym(")"):
                    raise QueryError("ROUND takes a value and optional places")
                return self.node(("fn", "round", [arg, int(places)]))
            raise QueryError(f"unknown function {value}; use COUNT, SUM, AVG, MIN, MAX, ROUND or ABS")
        raise self.unexpected()

    def literal(self):
        kind, value = self.peek()
        negative = False
        if self.peek() == ("sym", "-"):
            self.i += 1
            negative = True
            kind, value = self.peek()
            if kind != "num":
                raise QueryError("expected a number after -")
        if kind == "id" and value.lower() == "select":
            raise QueryError(HINTS["select"])
        nxt = self.t[self.i + 1] if self.i + 1 < len(self.t) else (None, None)
        if (kind, value) == ("sym", "(") and nxt[0] == "id" and nxt[1].lower() == "select":
            raise QueryError(HINTS["select"])
        if kind not in ("num", "str", "id"):
            raise QueryError("expected a value")
        self.i += 1
        if kind == "num":
            return ("num", -value if negative else value)
        return ("str", value)


def parse_sql(text):
    p = Parser(tokenize(text))
    p.expect_keyword("select")
    items = [p.expr()]
    if p.keyword("as"):
        p.ident()
    while p.sym(","):
        items.append(p.expr())
        if len(items) > LIMITS["items"]:
            raise QueryError(f"at most {LIMITS['items']} selected values")
        if p.keyword("as"):
            p.ident()
    p.expect_keyword("from")
    table = p.ident()
    conds = []
    if p.keyword("where"):
        while True:
            column = p.ident()
            if p.keyword("in"):
                if not p.sym("("):
                    raise QueryError("IN needs a list in parentheses")
                options = [p.literal()]
                while p.sym(","):
                    options.append(p.literal())
                    if len(options) > LIMITS["in_values"]:
                        raise QueryError(f"IN takes at most {LIMITS['in_values']} values")
                if not p.sym(")"):
                    raise QueryError("expected )")
                conds.append((column, "in", options))
            else:
                kind, op = p.peek()
                if kind != "sym" or op not in ("=", "!=", "<", "<=", ">", ">="):
                    raise QueryError(f"expected a comparison after {column}")
                p.i += 1
                conds.append((column, op, p.literal()))
            if len(conds) > LIMITS["conditions"]:
                raise QueryError(f"at most {LIMITS['conditions']} conditions")
            if not p.keyword("and"):
                break
    order = None
    if p.keyword("order"):
        p.expect_keyword("by")
        column = p.ident()
        descending = p.keyword("desc")
        if not descending:
            p.keyword("asc")
        order = (column, descending)
    limit = None
    if p.keyword("limit"):
        kind, value = p.peek()
        if kind != "num" or not math.isfinite(value) or value != int(value) or value >= 2 ** 32:
            raise QueryError("LIMIT needs a whole number")
        p.i += 1
        limit = int(value)
    p.sym(";")
    if p.i != len(p.t):
        raise p.unexpected()
    if any(has_agg(e) for e in items) and any(loose_column(e) for e in items):
        raise QueryError(f"column {loose_column(next(e for e in items if loose_column(e)))} must be inside an "
                         "aggregate such as SUM(...), or select it in a separate query")
    return items, table, conds, order, limit


def loose_column(e):
    """The first column used outside any aggregate, or None."""
    if e[0] == "col":
        return e[1]
    if e[0] in ("num", "agg"):
        return None
    if e[0] == "neg":
        return loose_column(e[1])
    if e[0] == "bin":
        return loose_column(e[2]) or loose_column(e[3])
    return loose_column(e[2][0])


def columns_of(e, out):
    if e is None or e[0] == "num":
        return
    if e[0] == "col":
        out.append(e[1])
    elif e[0] in ("neg", "agg"):
        columns_of(e[2] if e[0] == "agg" else e[1], out)
    elif e[0] == "bin":
        columns_of(e[2], out)
        columns_of(e[3], out)
    else:
        columns_of(e[2][0], out)


def number(v):
    return float(v) if isinstance(v, (int, float)) and not isinstance(v, bool) else None


def meets(cell, op, lit):
    kind, value = lit
    if op == "in":
        return any(meets(cell, "=", o) for o in value)
    x = number(cell)
    if op in ("=", "!="):
        same = (x == value) if (kind == "num" and x is not None) else (kind == "str" and cell == value)
        return same if op == "=" else not same
    if x is None or kind != "num":
        return False
    return {"<": x < value, "<=": x <= value, ">": x > value, ">=": x >= value}[op]


def select_rows(rows, table, conds, order, limit):
    rows = [r for r in rows if all(meets(r[c], op, lit if op != "in" else ("list", lit)) for c, op, lit in conds)]
    if order:
        column, descending = order
        keys = [row[column] for row in rows]
        if any(k is not None and not isinstance(k, str) and number(k) is None for k in keys):
            raise QueryError(f'sort_by column "{column}" must hold numbers or strings')
        if any(isinstance(k, str) for k in keys) and any(number(k) is not None for k in keys):
            raise QueryError(f'sort_by column "{column}" mixes numbers and strings')
        present = [r for r in rows if r[column] is not None]
        rows = sorted(present, key=lambda r: r[column], reverse=descending) + [r for r in rows if r[column] is None]
    return rows[:limit] if limit is not None else rows


def has_agg(e):
    if e[0] == "agg":
        return True
    if e[0] == "col" or e[0] == "num":
        return False
    if e[0] == "neg":
        return has_agg(e[1])
    if e[0] == "bin":
        return has_agg(e[2]) or has_agg(e[3])
    return has_agg(e[2][0])


def rust_round(x):
    """Rounds half away from zero, like Rust's f64::round."""
    if not math.isfinite(x):
        return x
    r = math.floor(abs(x))
    if abs(x) - r >= 0.5:
        r += 1
    return math.copysign(r, x)


def evaluate(e, row, rows):
    """Number (float) or None. `row` is set for per-row expressions, `rows` for aggregates."""
    value = evaluate_value(e, row, rows)
    return value if value is not None and math.isfinite(value) else None


def evaluate_value(e, row, rows):
    tag = e[0]
    if tag == "num":
        return e[1]
    if tag == "col":
        return number(row[e[1]])
    if tag == "neg":
        v = evaluate(e[1], row, rows)
        return None if v is None else -v
    if tag == "bin":
        a, b = evaluate(e[2], row, rows), evaluate(e[3], row, rows)
        if a is None or b is None:
            return None
        if e[1] in "/%" and b == 0:
            return None
        return {"+": a + b, "-": a - b, "*": a * b, "/": a / b if b else 0, "%": math.fmod(a, b) if b else 0}[e[1]]
    if tag == "fn":
        args = [evaluate(e[2][0], row, rows)]
        if any(a is None for a in args):
            return None
        if e[1] == "abs":
            return abs(args[0])
        scale = 10.0 ** e[2][1]
        return rust_round(args[0] * scale) / scale
    name, inner = e[1], e[2]
    if inner is None:
        return float(len(rows))
    if name == "count" and inner[0] == "col":
        return float(sum(r[inner[1]] is not None for r in rows))
    values = [evaluate(inner, r, None) for r in rows]
    values = [v for v in values if v is not None]
    if name == "count":
        return float(len(values))
    if name == "sum":
        total = 0.0
        for v in values:
            total += v
        return total
    if not values:
        return None
    if name == "avg":
        total = 0.0
        for v in values:
            total += v
        return total / len(values)
    best = values[0]
    for v in values[1:]:
        if (v < best) if name == "min" else (v > best):
            best = v
    return best


class RustFloat(float):
    """Serializes like Rust's f64 Display: shortest round-trip digits, never an exponent."""

    def __repr__(self):
        text = float.__repr__(self)
        return format(decimal.Decimal(text), "f") if "e" in text else text


class KeptFloat(float):
    """A number read from JSON that serializes with its original text, as gamectx keeps it."""

    def __new__(cls, text):
        value = super().__new__(cls, text)
        value.text = text
        return value

    def __repr__(self):
        return self.text


class KeptInt(int):
    """Preserves integer spelling too, including JSON's negative zero."""

    def __new__(cls, text):
        value = super().__new__(cls, text)
        value.text = text
        return value

    def __repr__(self):
        return self.text


def load_tables(path):
    with open(path) as source:
        return json.load(source, parse_float=KeptFloat, parse_int=KeptInt)


def num_json(v):
    if v is None or not math.isfinite(v):
        return None
    if v == int(v) and abs(v) < 1e15:
        return int(v)
    return RustFloat(v)


def run(items, table, conds, order, limit, tables):
    if table not in tables:
        raise QueryError(f'no table "{table}"; tables: {", ".join(tables)}')
    rows = tables[table]
    if any(not isinstance(row, dict) for row in rows):
        raise QueryError("table rows must be objects")
    used = []
    for e in items:
        columns_of(e, used)
    used += [c for c, _, _ in conds] + ([order[0]] if order else [])
    for column in used:
        for row in rows:
            if column not in row:
                raise QueryError(f'no column "{column}" in table "{table}"; columns: {", ".join(rows[0])}')
    rows = select_rows(rows, table, conds, order, limit)
    if any(has_agg(e) for e in items):
        out = [num_json(evaluate(e, None, rows)) for e in items]
        return out[0] if len(items) == 1 else out
    out = []
    for row in rows:
        cells = []
        for e in items:
            if e[0] == "col":
                cells.append(row[e[1]])
            else:
                cells.append(num_json(evaluate(e, row, None)))
        out.append(cells[0] if len(items) == 1 else cells)
    return out


def run_sql(text, tables):
    return run(*parse_sql(text), tables)


def run_json(query, tables):
    """The JSON form, translated to the same parts as the SQL form."""
    if not isinstance(query, dict) or not isinstance(query.get("table"), str) or not isinstance(query.get("get"), str):
        raise QueryError("query needs table and get")
    conds = []
    for column, cond in (query.get("where") or {}).items():
        if isinstance(cond, dict) and len(cond) == 1:
            (op, value), = cond.items()
            op = {"==": "="}.get(op, op)
        else:
            op, value = "=", cond
        if op == "in":
            if not isinstance(value, list):
                raise QueryError("in needs an array")
            conds.append((column, "in", [("num", float(v)) if number(v) is not None else ("str", v) for v in value]))
        elif op in ("=", "!=", "<", "<=", ">", ">="):
            conds.append((column, op, ("num", float(value)) if number(value) is not None else ("str", value)))
        else:
            raise QueryError(f"unknown operator {op}")
    order = None
    if query.get("sort_by"):
        s = query["sort_by"]
        order = (s[1:], True) if s.startswith("-") else (s, False)
    limit = query.get("limit")
    get = query["get"].strip()
    items = [("agg", "count", None)] if get.lower() == "count" else Parser(tokenize(get)).expr_all()
    return run(items, query["table"], conds, order, limit, tables)


def _expr_all(self):
    items = [self.expr()]
    while self.sym(","):
        items.append(self.expr())
    if self.i != len(self.t):
        raise QueryError("unexpected text in get")
    return items


Parser.expr_all = _expr_all


def dumps(value):
    """Compact JSON, with computed numbers written as Rust writes them."""
    if isinstance(value, (RustFloat, KeptFloat, KeptInt)):
        return repr(value)
    if isinstance(value, list):
        return "[" + ",".join(dumps(v) for v in value) + "]"
    if isinstance(value, dict):
        return "{" + ",".join(dumps(k) + ":" + dumps(v) for k, v in value.items()) + "}"
    return json.dumps(value, separators=(",", ":"), ensure_ascii=False)


# ---------------------------------------------------------------- questions and scoring

def build_questions(tools, inventory, roster):
    qs, _ = m.build_questions(tools, inventory, roster)
    out = [(k, q, t, e) for k, q, t, e in qs if k in KINDS]
    out += [("calc", q, "float", want) for q, want in calc_eval.questions(inventory, roster)]
    return out


def correct(answer_type, expected, got):
    if answer_type == "set":
        return isinstance(got, list) and {m.norm(a) for a in got} == {m.norm(e) for e in expected}
    if isinstance(got, list) and len(got) == 1:
        got = got[0]
    if answer_type == "str":
        return m.norm(got) == m.norm(expected)
    return calc_eval.close(got, expected)


# ---------------------------------------------------------------- main

def replay(path, gamectx_bin, tables_path):
    data = json.loads(Path(path).read_text())
    same = differ = 0
    for row in data["rows"]:
        if row["format"] not in ("sql", "tool") or not isinstance(row.get("query"), str):
            continue
        try:
            want = dumps(run_sql(row["query"], load_tables(tables_path)))
        except QueryError:
            want = None
        proc = subprocess.run([gamectx_bin, "query", tables_path, row["query"]], capture_output=True, text=True)
        got = proc.stdout.strip() if proc.returncode == 0 else None
        if got == want:
            same += 1
        else:
            differ += 1
            print(f"  differs: {row['query']!r}\n    prototype {want}\n    gamectx   {got} {proc.stderr.strip()}")
    print(f"replay: {same} identical, {differ} different")
    return differ == 0


def random_queries(n, seed=3):
    """Grammar-generated queries, half of them then mutated, for comparing implementations."""
    import random
    rng = random.Random(seed)
    columns = {"inventory": ["id", "kind", "rarity", "count", "weight"], "npcs": ["name", "faction", "role", "trust"]}
    values = ["food", "'rare'", '"guard"', "keepers", "3", "15", "0.5", "-1", "2.25", "'x y'"]

    def expr(cols, depth=0):
        r = rng.random()
        if depth > 2 or r < 0.4:
            return rng.choice(cols + ["2", "0", "1.5"])
        if r < 0.7:
            return f"{expr(cols, depth + 1)} {rng.choice('+-*/%')} {expr(cols, depth + 1)}"
        if r < 0.8:
            return f"-{expr(cols, depth + 1)}"
        if r < 0.9:
            return f"ROUND({expr(cols, depth + 1)}, {rng.randint(0, 3)})"
        return f"(ABS({expr(cols, depth + 1)}))"

    out = []
    for _ in range(n):
        table = rng.choice(list(columns))
        cols = columns[table]
        if rng.random() < 0.5:
            items = [rng.choice(["COUNT(*)", f"COUNT({rng.choice(cols)})"] +
                                [f"{f}({expr(cols)})" for f in ("SUM", "AVG", "MIN", "MAX")])
                     for _ in range(rng.randint(1, 3))]
            if rng.random() < 0.3:
                items[0] = f"{items[0]} - {rng.choice(['MIN', 'MAX'])}({rng.choice(cols)})"
        else:
            items = [expr(cols) for _ in range(rng.randint(1, 3))]
        items = [f"{i} AS {rng.choice(['total', 'x', 'from'])}" if rng.random() < 0.15 else i for i in items]
        q = f"SELECT {', '.join(items)} FROM {table}"
        conds = []
        for _ in range(rng.randint(0, 3)):
            c = rng.choice(cols)
            if rng.random() < 0.2:
                conds.append(f"{c} IN ({', '.join(rng.sample(values, 3))})")
            else:
                conds.append(f"{c} {rng.choice(['=', '!=', '<>', '<', '<=', '>', '>='])} {rng.choice(values)}")
        if conds:
            q += " WHERE " + " AND ".join(conds)
        if rng.random() < 0.5:
            q += f" ORDER BY {rng.choice(cols)}{rng.choice(['', ' DESC', ' asc'])}"
        if rng.random() < 0.4:
            q += f" LIMIT {rng.randint(0, 6)}"
        if rng.random() < 0.5:
            chars = list(q)
            for _ in range(rng.randint(1, 3)):
                at = rng.randrange(len(chars) + 1)
                noise = rng.choice(list("()*,;'\"-+<>=!. 0123456789abcxyz_"))
                if rng.random() < 0.5 and at < len(chars):
                    chars[at] = noise
                else:
                    chars.insert(at, noise)
            q = "".join(chars)
        out.append(q)
    return out


def differential(n, gamectx_bin, tables_path):
    """Compares this prototype with `gamectx query` on generated queries: both must accept
    and reject the same queries, and accepted ones must print the same bytes."""
    tables = load_tables(tables_path)
    same = differ = accepted = 0
    for q in random_queries(n):
        try:
            want = dumps(run_sql(q, tables))
        except QueryError:
            want = None
        proc = subprocess.run([gamectx_bin, "query", tables_path, q], capture_output=True, text=True)
        got = proc.stdout.strip() if proc.returncode == 0 else None
        accepted += want is not None
        if got == want:
            same += 1
        else:
            differ += 1
            print(f"  differs: {q!r}\n    prototype {want}\n    gamectx   {got} {proc.stderr.strip()}")
    print(f"differential: {same} identical ({accepted} accepted), {differ} different")
    return differ == 0


def retry(path, args, tables):
    """Re-scores a run's queries with the current prototype, then sends each remaining
    error back once, as a game would, and scores the corrected query."""
    data = json.loads(Path(path).read_text())
    key = Path(args.key_file).read_text().strip()
    venice = "venice.ai" in args.base_url
    prices = m.get_prices(args.base_url, key) if venice else {}
    run_dir = os.path.dirname(path)
    packets, _ = m.build_packets(os.path.join(run_dir, "retry"), args.gamectx)
    systems = {"sql": m.SYSTEM + packets["gamectx-nested"]}
    if os.path.exists(os.path.join(run_dir, "manifest-tool.json")):
        tool_packet = subprocess.run([args.gamectx, "pack", os.path.join(run_dir, "manifest-tool.json")],
                                     check=True, capture_output=True, text=True).stdout.strip()
        systems["tool"] = m.SYSTEM + tool_packet
    rules = {"sql": SQL_RULE, "tool": TOOL_RULE}
    answer_types = {q: t for _, q, t, _ in build_questions(*m.build_data()[:3])}
    spent, first, fixed, tried, total = 0.0, 0, 0, 0, 0
    for row in data["rows"]:
        if row["format"] not in systems:
            continue
        total += 1
        try:
            got, error = run_sql(str(row["query"]), tables), ""
        except QueryError as e:
            got, error = None, str(e)
        if correct(answer_types[row["question"]], row["expected"], got):
            first += 1
            continue
        if not error:
            continue
        price = prices.get(row["model"], m.FALLBACK_PRICE)
        fix = 'Reply with a corrected {"action": "query", "args": {"sql": "<query>"}}.' if row["format"] == "tool" \
            else 'Reply with a corrected {"sql": "<query>"}.'
        messages = [{"role": "system", "content": systems[row["format"]]},
                    {"role": "user", "content": row["question"] + rules[row["format"]]},
                    {"role": "assistant", "content": row["reply"]},
                    {"role": "user", "content": f"The game replied: error: {error}. {fix}"}]
        worst = (sum(len(x["content"].encode()) for x in messages) * price["input"] + OUT_TOKENS * price["output"]) / 1e6
        if spent + worst > args.cap:
            print(f"stopped: spending cap ${args.cap:.2f}")
            break
        reply = m.chat_messages(args.base_url, key, row["model"], messages, venice, OUT_TOKENS)
        usage = reply.get("usage", {})
        spent += (usage.get("prompt_tokens", 0) * price["input"] + usage.get("completion_tokens", 0) * price["output"]) / 1e6
        text = reply["choices"][0]["message"].get("content") or ""
        data_ = m.extract_json(text) or {}
        query = (data_.get("args") or {}).get("sql") if row["format"] == "tool" else data_.get("sql")
        try:
            got, error = run_sql(str(query), tables), ""
        except QueryError as e:
            got, error = None, str(e)
        ok = correct(answer_types[row["question"]], row["expected"], got)
        tried += 1
        fixed += ok
        print(f"  [{row['model']}] {'fixed' if ok else 'still wrong'}: {query!r} {error or dumps(got)[:60]}")
    print(f"first try (current prototype): {first}/{total}; one retry after an error fixed {fixed}/{tried}; "
          f"after retry {first + fixed}/{total}; spent ${spent:.4f}")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--base-url", default="https://api.venice.ai/api/v1")
    ap.add_argument("--key-file", default=os.path.expanduser("~/.config/gamectx-eval/venice.key"))
    ap.add_argument("--models", default=m.DEFAULT_MODELS)
    ap.add_argument("--formats", default="direct,sql,json")
    ap.add_argument("--cap", type=float, default=0.08)
    ap.add_argument("--gamectx", default=os.path.join(m.ROOT, "target", "release", "gamectx"))
    ap.add_argument("--replay", help="check gamectx query against this prototype on a results.json")
    ap.add_argument("--differential", type=int, metavar="N", help="compare with gamectx query on N generated queries")
    ap.add_argument("--retry", help="send failed SQL queries' errors back once, from this results.json")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--tokens-only", action="store_true", help="measure input tokens and usage with 1-token replies")
    args = ap.parse_args()
    if not math.isfinite(args.cap) or args.cap < 0:
        ap.error("--cap must be a finite non-negative amount")

    run_dir = os.path.join(m.ROOT, "eval", "results", "query-" + time.strftime("%Y%m%d-%H%M%S"))
    os.makedirs(run_dir, exist_ok=True)
    tools, inventory, roster, _ = m.build_data()
    tables = {"inventory": inventory, "npcs": roster}
    tables_path = os.path.join(run_dir, "tables.json")
    Path(tables_path).write_text(json.dumps(tables, separators=(",", ":")))
    if args.replay:
        sys.exit(0 if replay(args.replay, args.gamectx, tables_path) else 1)
    if args.differential:
        sys.exit(0 if differential(args.differential, args.gamectx, tables_path) else 1)
    if args.retry:
        return retry(args.retry, args, tables)
    packets, _ = m.build_packets(run_dir, args.gamectx)
    packet = packets["gamectx-nested"]
    qs = build_questions(tools, inventory, roster)
    formats = args.formats.split(",")
    rules = {"direct": m.ANSWER_RULE, "sql": SQL_RULE, "json": JSON_RULE, "tool": TOOL_RULE}
    # "tool": the query tool from `gamectx query --tool` packed with the other actions.
    query_tool = json.loads(subprocess.run([args.gamectx, "query", "--tool"], check=True, capture_output=True,
                                           text=True).stdout)
    manifest = json.loads(Path(run_dir, "manifest-nested.json").read_text())
    manifest["sections"][0]["value"] = tools + [query_tool]
    tool_packet = m.pack(manifest, os.path.join(run_dir, "manifest-tool.json"), args.gamectx)
    print(f"{len(qs)} questions x {len(formats)} formats; packet {len(packet)} bytes, "
          f"with the query tool {len(tool_packet)} bytes")
    if args.dry_run:
        for kind, q, _, e in qs:
            print(f"  [{kind}] {q} -> {e}")
        return

    key = Path(args.key_file).read_text().strip()
    venice = "venice.ai" in args.base_url
    prices = m.get_prices(args.base_url, key) if venice else {}
    spent, rows, stopped = 0.0, [], None
    if args.tokens_only:
        for model in args.models.split(","):
            price = prices.get(model, m.FALLBACK_PRICE)
            for fmt in formats:
                system = m.SYSTEM + (tool_packet if fmt == "tool" else packet)
                user = "Reply with {}." + rules[fmt]
                worst = ((len((system + user).encode()) + 256) * price["input"] + price["output"]) / 1e6
                if spent + worst > args.cap:
                    stopped = f"stopped: spending cap ${args.cap:.2f}"
                    break
                reply = m.chat(args.base_url, key, model, system, user, venice, 1)
                usage = reply.get("usage", {})
                cost = (usage.get("prompt_tokens", 0) * price["input"]
                        + usage.get("completion_tokens", 0) * price["output"]) / 1e6
                spent += cost
                rows.append({"model": model, "format": fmt, "usage": usage, "cost": cost})
                cached = (usage.get("prompt_tokens_details") or {}).get("cached_tokens", "unreported")
                print(f"  {model} {fmt}: {usage.get('prompt_tokens', 0)} input, {cached} cached")
            if stopped:
                break
        Path(run_dir, "tokens.json").write_text(json.dumps(
            {"spent_usd": spent, "stopped": stopped, "rows": rows}, indent=1))
        print(f"spent ${spent:.6f}" + (f"; {stopped}" if stopped else ""))
        print(f"full usage: {run_dir}/tokens.json")
        return
    for model in args.models.split(","):
        price = prices.get(model, m.FALLBACK_PRICE)
        for n, (kind, q, answer_type, expected) in enumerate(qs):
            for fmt in formats if n % 2 == 0 else formats[::-1]:
                system, user = m.SYSTEM + (tool_packet if fmt == "tool" else packet), q + rules[fmt]
                worst = (len((system + user).encode()) * price["input"] + OUT_TOKENS * price["output"]) / 1e6
                if spent + worst > args.cap:
                    stopped = f"stopped: spending cap ${args.cap:.2f} (spent ${spent:.4f})"
                    break
                try:
                    reply = m.chat(args.base_url, key, model, system, user, venice, OUT_TOKENS)
                    text = reply["choices"][0]["message"].get("content") or ""
                    usage = reply.get("usage", {})
                except Exception as e:  # noqa: BLE001 - record and continue
                    text, usage = "", {}
                    print(f"  {model} {kind} {fmt}: request failed: {e}")
                spent += (usage.get("prompt_tokens", 0) * price["input"]
                          + usage.get("completion_tokens", 0) * price["output"]) / 1e6
                data = m.extract_json(text) or {}
                query, got, error = None, None, ""
                try:
                    if fmt == "direct":
                        got = data.get("answer")
                    elif fmt == "sql":
                        query = data.get("sql")
                        got = run_sql(str(query), tables)
                    elif fmt == "tool":
                        query = (data.get("args") or {}).get("sql") if data.get("action") == "query" else None
                        if query is None:
                            raise QueryError("no query action in reply")
                        got = run_sql(str(query), tables)
                    else:
                        query = data.get("query")
                        got = run_json(query, tables)
                except (QueryError, TypeError, ValueError, AttributeError) as e:
                    error = str(e)
                rows.append({"model": model, "kind": kind, "format": fmt, "question": q, "expected": expected,
                             "query": query, "got": got, "error": error, "ok": correct(answer_type, expected, got),
                             "reply": text[:300], "prompt_tokens": usage.get("prompt_tokens", 0),
                             "completion_tokens": usage.get("completion_tokens", 0)})
            if stopped:
                break
            print(f"  {model}: {n + 1}/{len(qs)}, spent ${spent:.4f}", end="\r")
        print()
        if stopped:
            break
    Path(run_dir, "results.json").write_text(json.dumps(
        {"spent_usd": spent, "stopped": stopped, "rows": rows}, indent=1))
    print(f"\n{'model':<34} {'format':<8} {'correct':>8} {'avg in':>7} {'avg out':>8}")
    for model in args.models.split(","):
        for fmt in formats:
            r = [x for x in rows if x["model"] == model and x["format"] == fmt]
            if r:
                print(f"{model:<34} {fmt:<8} {sum(x['ok'] for x in r):>4}/{len(r):<3} "
                      f"{sum(x['prompt_tokens'] for x in r) / len(r):>7.0f} "
                      f"{sum(x['completion_tokens'] for x in r) / len(r):>8.0f}")
    print(f"\nby question type ({' / '.join(formats)}):")
    for kind in dict.fromkeys(x["kind"] for x in rows):
        cells = []
        for fmt in formats:
            r = [x for x in rows if x["kind"] == kind and x["format"] == fmt]
            cells.append(f"{sum(x['ok'] for x in r)}/{len(r)}")
        print(f"  {kind:<12} " + "  ".join(f"{c:>6}" for c in cells))
    print("\nwrong answers:")
    for x in rows:
        if not x["ok"]:
            detail = x["error"] or f"got {dumps(x['got'])[:80]}"
            print(f"  [{x['model']} / {x['format']}] {x['kind']}: {dumps(x['query'])[:110] if x['query'] else ''} {detail}")
    print(f"\nspent ${spent:.4f}" + (f"; {stopped}" if stopped else ""))
    print(f"full results: {run_dir}/results.json")


if __name__ == "__main__":
    main()
