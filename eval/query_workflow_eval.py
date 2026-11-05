#!/usr/bin/env python3
"""Prototype schema-only query packets and one bounded retry, including the answer turn.

The historical eval stays unchanged. `legacy` preserves its ambiguous weight questions;
`full` and `schema` use explicit unit_weight/stack_weight fields and precise questions.
Development state: 7. Holdout states: 101,202,303 (different rows and table sizes).
Standard library only. Every paid invocation requires --cap; --dry-run/--check are free.
"""

import argparse
import json
import math
from pathlib import Path
import re
import subprocess
import sys
import time

sys.path.insert(0, str(Path(__file__).resolve().parent))
import query_eval as q

m = q.m
ROOT = Path(m.ROOT)
RETRY_HINT = ("Use only the tables and columns below. No subqueries; one SELECT only. "
              "For the largest row use ORDER BY column DESC LIMIT 1. "
              "ORDER BY and LIMIT apply before SUM/AVG/MIN/MAX, including top-N sums.")
FINAL_RULE = ' Reply as {"answer": <result>}, preserving the result values. Do not query again.'
TYPE_ORDER = ("null", "boolean", "number", "string", "array", "object")


def feedback_error(error):
    text = str(error)
    if text.startswith("no table "):
        return "Unknown table. Use a table name listed below."
    if text.startswith("no column "):
        return "Unknown column. Use columns from the selected table."
    if text.startswith("query result exceeds "):
        return text
    return "Query must use the supported SELECT subset."


def kind(value):
    if value is None:
        return "null"
    if isinstance(value, bool):
        return "boolean"
    if isinstance(value, (int, float)):
        return "number"
    if isinstance(value, str):
        return "string"
    if isinstance(value, list):
        return "array"
    return "object"


def schema(tables):
    """No rows or numeric samples; complete small string domains only, never partial enums."""
    if len(tables) > 16:
        raise q.QueryError("at most 16 query tables")
    out = {}
    for name, rows in tables.items():
        if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]{0,63}", name):
            raise q.QueryError("query names must be ASCII identifiers of at most 64 characters")
        if any(not isinstance(row, dict) for row in rows):
            raise q.QueryError("table rows must be objects")
        columns = list(rows[0]) if rows else []
        if len(columns) > 64:
            raise q.QueryError("at most 64 query columns")
        if any(set(row) != set(columns) for row in rows):
            raise q.QueryError("query table rows must have the same columns")
        typed, values = [], {}
        for column in columns:
            if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]{0,63}", column):
                raise q.QueryError("query names must be ASCII identifiers of at most 64 characters")
            types = {kind(row[column]) for row in rows}
            typed.append([column, "|".join(t for t in TYPE_ORDER if t in types)])
            if types == {"string"}:
                domain = []
                for row in rows:
                    value = row[column]
                    if len(value) > 64:
                        domain = None
                        break
                    if value not in domain:
                        domain.append(value)
                        if len(domain) > 8:
                            domain = None
                            break
                if domain:
                    values[column] = domain
        out[name] = {"columns": typed}
        if values:
            out[name]["values"] = values
    return out


class Session:
    """One request and at most one correction; success ends the session too."""

    def __init__(self, result_bytes=4096):
        self.attempts = 0
        self.finished = False
        self.result_bytes = result_bytes

    def submit(self, sql, tables):
        if self.finished:
            raise q.QueryError("query session is finished")
        self.attempts += 1
        try:
            result = q.run_sql(sql, tables)
            if len(q.dumps(result).encode()) > self.result_bytes:
                raise q.QueryError(f"query result exceeds {self.result_bytes} bytes; select fewer columns or add LIMIT")
            self.finished = True
            return {"result": result}
        except q.QueryError as error:
            retry = self.attempts < 2
            self.finished = not retry
            reply = {"error": feedback_error(error), "retry": retry}
            if retry:
                reply["tables"] = schema(tables)
                reply["hint"] = RETRY_HINT
            return reply

    def apply_call(self, args, tables):
        sql = args["sql"] if isinstance(args, dict) and set(args) == {"sql"} and isinstance(args["sql"], str) else ""
        return self.submit(sql, tables)


def state(seed):
    tools, inventory, npcs, rules = m.build_data(seed)
    if seed == 101:
        inventory, npcs = inventory[:24], npcs[:10]
    elif seed == 303:
        _, more, people, _ = m.build_data(seed + 1)
        inventory += [dict(row, id=row["id"] + "_b") for row in more]
        npcs += [dict(row, name=row["name"] + "b") for row in people]
    explicit = []
    for row in inventory:
        explicit.append({**{k: v for k, v in row.items() if k != "weight"},
                         "unit_weight": row["weight"], "stack_weight": q.num_json(row["count"] * row["weight"])})
    return tools, {"inventory": inventory, "npcs": npcs}, {"inventory": explicit, "npcs": npcs}, rules


def questions(tools, tables, explicit):
    inv, people = tables["inventory"], tables["npcs"]
    base, _ = m.build_questions(tools, inv, people)
    out = [(k, text, typ, expected) for k, text, typ, expected in base if k in q.KINDS]
    # Accept any tied maximum; a stable ORDER BY/LIMIT is a valid answer.
    for n, (k, text, typ, expected) in enumerate(out):
        if "highest trust" in text:
            best = max(row["trust"] for row in people)
            out[n] = (k, text, "one_of", [row["name"] for row in people if row["trust"] == best])
        elif "heaviest item" in text:
            best = max(row["weight"] for row in inv)
            out[n] = (k, text.replace("by weight", "by unit_weight") if explicit else text,
                      "one_of", [row["id"] for row in inv if row["weight"] == best])
    # These are new, explicit questions, rather than a silent re-score of historical data.
    for text, want in q.calc_eval.questions(inv, people):
        if explicit:
            text = {
                "What is the total weight of all food items?":
                    "What is the sum of unit_weight across food rows, without multiplying by count?",
                "What is the total weight of all scroll items, each weight multiplied by its count?":
                    "What is the sum of stack_weight across scroll rows?",
                "What is the combined weight of the three heaviest items?":
                    "What is the sum of unit_weight for the three rows with largest unit_weight?",
                "What is the total weight of all rare items?":
                    "What is the sum of unit_weight across rare rows, without multiplying by count?",
            }.get(text, text)
        out.append(("calc", text, "float", want))
    return out


def correct(typ, expected, value):
    if typ == "one_of":
        if isinstance(value, list) and len(value) == 1:
            value = value[0]
        return any(m.norm(value) == m.norm(x) for x in expected)
    return q.correct(typ, expected, value)


def packet(tools, tables, rules, mode, binary, directory):
    tool = json.loads(subprocess.run([binary, "query", "--tool"], check=True, capture_output=True, text=True).stdout)
    sections = [
        {"name": "actions", "pack": "signatures", "value": tools + [tool], "stable": True, "required": True},
        {"name": "rules", "value": rules, "stable": True, "required": True},
    ]
    if mode != "legacy":
        sections.append({"name": "field_notes", "value": {
            "inventory": {"unit_weight": "Weight of one item, independent of count.",
                          "stack_weight": "count * unit_weight; weight of the whole row."}}})
    if mode == "schema":
        sections.append({"name": "query_tables", "value": schema(tables)})
    else:
        sections += [{"name": name, "pack": "table", "value": rows,
                      "group_by": ["kind", "rarity"] if name == "inventory" else "faction"}
                     for name, rows in tables.items()]
    return m.pack({"sections": sections}, str(directory / f"manifest-{mode}.json"), binary)


def ask(messages, args, key, prices, accounting, max_tokens):
    price = prices.get(accounting["model"], m.FALLBACK_PRICE)
    worst = ((sum(len(x["content"].encode()) for x in messages) + 512) * price["input"]
             + max_tokens * price["output"]) / 1e6
    if accounting["spent"] + worst > args.cap:
        raise StopIteration("spending cap")
    try:
        reply = m.chat_messages(args.base_url, key, accounting["model"], messages,
                                "venice.ai" in args.base_url, max_tokens)
    except Exception:
        accounting["spent"] += worst  # uncertain failed requests are never treated as free
        raise
    usage = reply.get("usage", {})
    # Reserve the worst-case cost when usage is absent; never silently count an unknown call as free.
    cost = ((usage["prompt_tokens"] * price["input"] + usage["completion_tokens"] * price["output"]) / 1e6
            if "prompt_tokens" in usage and "completion_tokens" in usage else worst)
    accounting["spent"] += cost
    text = reply["choices"][0]["message"].get("content") or ""
    return text, {"usage": usage, "cost_usd": cost}


def extract_sql(text):
    data = m.extract_json(text)
    if not isinstance(data, dict) or data.get("action") != "query":
        return ""
    args = data.get("args")
    return args["sql"] if isinstance(args, dict) and set(args) == {"sql"} and isinstance(args["sql"], str) else ""


def evaluate(args, bundles, directory):
    key = Path(args.key_file).read_text().strip()
    if not key:
        raise ValueError("key file is empty")
    prices = m.get_prices(args.base_url, key) if "venice.ai" in args.base_url else {}
    account = {"spent": 0.0, "model": ""}
    prior = json.loads(Path(args.resume).read_text()) if args.resume else {}
    rows, stopped = list(prior.get("rows", [])), None
    completed = {(r["model"], r["seed"], r["mode"], r.get("question")) for r in rows}
    try:
        for model in args.models.split(","):
            account["model"] = model
            for seed, mode, tables, prompt, qs in bundles:
                if args.tokens_only:
                    _, call = ask([{"role": "system", "content": m.SYSTEM + prompt},
                                   {"role": "user", "content": "Reply with {}." + q.TOOL_RULE}],
                                  args, key, prices, account, 1)
                    rows.append({"model": model, "seed": seed, "mode": mode, "calls": [call]})
                    print(f"{model} state {seed} {mode}: {call['usage'].get('prompt_tokens')} input tokens", flush=True)
                    continue
                for question_number, (kind_, question, typ, expected) in enumerate(qs, 1):
                    if (model, seed, mode, question) in completed:
                        continue
                    messages = [{"role": "system", "content": m.SYSTEM + prompt},
                                {"role": "user", "content": question + q.TOOL_RULE}]
                    session, calls, attempts = Session(), [], []
                    result, feedback = None, None
                    while not session.finished:
                        text, call = ask(messages, args, key, prices, account, 300)
                        calls.append(call)
                        sql = extract_sql(text)
                        feedback = session.submit(sql, tables)
                        attempts.append({"sql": sql, "feedback": feedback, "reply": text})
                        messages.append({"role": "assistant", "content": text})
                        if "result" in feedback:
                            result = feedback["result"]
                        elif feedback["retry"]:
                            messages.append({"role": "user", "content": q.dumps(feedback) + q.TOOL_RULE})
                    first = "result" in attempts[0]["feedback"] and correct(typ, expected, attempts[0]["feedback"]["result"])
                    executed = "result" in feedback and correct(typ, expected, result)
                    answer, final_ok = None, False
                    if "result" in feedback:
                        messages.append({"role": "user", "content": q.dumps(feedback) + FINAL_RULE})
                        text, call = ask(messages, args, key, prices, account, 300)
                        calls.append(call)
                        parsed = m.extract_json(text)
                        answer = parsed.get("answer") if isinstance(parsed, dict) else None
                        final_ok = correct(typ, expected, answer)
                    rows.append({"model": model, "seed": seed, "mode": mode, "kind": kind_,
                                 "question": question, "answer_type": typ, "expected": expected,
                                 "attempts": attempts, "first_ok": first, "executed_ok": executed,
                                 "answer": answer, "ok": final_ok, "calls": calls})
                    if question_number % 5 == 0:
                        print(f"  {model} state {seed} {mode}: {question_number}/{len(qs)}; "
                              f"spent ${account['spent']:.6f}", flush=True)
                subset = [r for r in rows if r["model"] == model and r["seed"] == seed and r["mode"] == mode]
                print(f"{model} state {seed} {mode}: first {sum(r['first_ok'] for r in subset)}/{len(subset)}, "
                      f"after retry {sum(r['executed_ok'] for r in subset)}/{len(subset)}, "
                      f"final {sum(r['ok'] for r in subset)}/{len(subset)}; spent ${account['spent']:.6f}", flush=True)
    except StopIteration as error:
        stopped = str(error)
    finally:
        data = {"cap_usd": args.cap, "spent_usd": account["spent"], "stopped": stopped,
                "tokens_only": args.tokens_only, "seeds": args.seeds, "rows": rows}
        if args.resume:
            data["resumed_from"] = Path(args.resume).parent.name
            data["cumulative_spent_usd"] = prior.get("cumulative_spent_usd", prior["spent_usd"]) + account["spent"]
        (directory / "results.json").write_text(q.dumps(data) + "\n")
        print(f"spent ${account['spent']:.6f}; stopped: {stopped}; results: {directory / 'results.json'}", flush=True)


def check(args, bundles):
    same = 0
    for _, _, tables, _, _ in bundles:
        proc = subprocess.run([args.gamectx, "query", "--schema", "-"], input=q.dumps(tables),
                              capture_output=True, text=True)
        if proc.returncode or proc.stdout.strip() != q.dumps(schema(tables)):
            raise AssertionError(f"schema differs: {proc.stdout} {proc.stderr}")
        same += 1
    print(f"schemas: {same} byte-identical")


def replay(path, args):
    data = json.loads(Path(path).read_text())
    same = 0
    for row in data["rows"]:
        if "attempts" not in row:
            continue
        tables_path = Path(path).parent / str(row["seed"]) / ("legacy.json" if row["mode"] == "legacy" else "tables.json")
        tables = q.load_tables(tables_path)
        session = Session()
        expected = []
        for attempt in row["attempts"]:
            feedback = session.submit(attempt["sql"], tables)
            if q.dumps(feedback) != q.dumps(attempt["feedback"]):
                raise AssertionError("prototype drifted from measured feedback")
            expected.append(q.dumps(feedback))
        calls = "".join(q.dumps({"sql": a["sql"]}) + "\n" for a in row["attempts"])
        proc = subprocess.run([args.gamectx, "query", "--session", str(tables_path)], input=calls,
                              capture_output=True, text=True)
        if proc.returncode or proc.stdout.splitlines() != expected:
            raise AssertionError(f"session differs: {row['model']} {row['mode']} {row['question']}: {proc.stdout} {proc.stderr}")
        same += len(expected)
    print(f"replay: {same} attempt envelopes byte-identical to measured prototype")


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--cap", required=True, type=float)
    ap.add_argument("--seeds", default="7")
    ap.add_argument("--modes", default="legacy,full,schema")
    ap.add_argument("--models", default=m.DEFAULT_MODELS)
    ap.add_argument("--tokens-only", action="store_true")
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--replay", help="free: compare saved attempt envelopes with the Rust session CLI")
    ap.add_argument("--resume", help="complete a capped run without repeating completed questions; cap applies to new spend")
    ap.add_argument("--gamectx", default=str(ROOT / "target/release/gamectx"))
    ap.add_argument("--base-url", default="https://api.venice.ai/api/v1")
    ap.add_argument("--key-file", default=str(Path.home() / ".config/gamectx-eval/venice.key"))
    args = ap.parse_args()
    if not math.isfinite(args.cap) or args.cap < 0:
        ap.error("--cap must be finite and non-negative")
    if set(args.modes.split(",")) - {"legacy", "full", "schema"}:
        ap.error("unknown mode")
    if args.replay:
        return replay(args.replay, args)
    directory = ROOT / "eval/results" / ("workflow-" + time.strftime("%Y%m%d-%H%M%S") + f"-{time.time_ns() % 1000000:06}")
    directory.mkdir(parents=True)
    bundles = []
    for seed in map(int, args.seeds.split(",")):
        tools, legacy, explicit, rules = state(seed)
        subdir = directory / str(seed)
        subdir.mkdir()
        (subdir / "legacy.json").write_text(q.dumps(legacy))
        (subdir / "tables.json").write_text(q.dumps(explicit))
        if args.resume:
            for filename in ("legacy.json", "tables.json"):
                if (Path(args.resume).parent / str(seed) / filename).read_text() != (subdir / filename).read_text():
                    raise ValueError("resume tables differ from the original run")
        for mode in args.modes.split(","):
            tables = legacy if mode == "legacy" else explicit
            prompt = packet(tools, tables, rules, mode, args.gamectx, subdir)
            if args.resume:
                original = json.loads((Path(args.resume).parent / str(seed) / f"manifest-{mode}.json").read_text())
                if original != json.loads((subdir / f"manifest-{mode}.json").read_text()):
                    raise ValueError("resume prompt differs from the original run")
            qs = questions(tools, legacy, mode != "legacy")
            bundles.append((seed, mode, tables, prompt, qs))
            print(f"state {seed} {mode}: {len(prompt.encode())} bytes, {len(qs)} questions", flush=True)
    if args.check:
        check(args, bundles)
    elif not args.dry_run:
        evaluate(args, bundles, directory)


if __name__ == "__main__":
    main()
