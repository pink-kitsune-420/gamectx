#!/usr/bin/env python3
"""Summarize saved workflow runs without API calls. Pass completed resume files instead
of their incomplete parent: cumulative spend includes the parent, rows are not duplicated.
"""
import argparse
import collections
import json
from pathlib import Path


def metrics(rows):
    calls = [call for row in rows for call in row["calls"]]
    return {
        "questions": len(rows),
        "first_correct": sum(row["first_ok"] for row in rows),
        "after_retry_correct": sum(row["executed_ok"] for row in rows),
        "final_correct": sum(row["ok"] for row in rows),
        "retries": sum(len(row["attempts"]) - 1 for row in rows),
        "recovered": sum(not row["first_ok"] and row["executed_ok"] for row in rows),
        "terminal_query_errors": sum("result" not in row["attempts"][-1]["feedback"] for row in rows),
        "semantic_errors": sum("result" in row["attempts"][-1]["feedback"] and not row["executed_ok"] for row in rows),
        "answer_copy_errors": sum(row["executed_ok"] and not row["ok"] for row in rows),
        "calls": len(calls),
        "avg_total_input_tokens": sum(c["usage"].get("prompt_tokens", 0) for c in calls) / len(rows),
        "avg_total_output_tokens": sum(c["usage"].get("completion_tokens", 0) for c in calls) / len(rows),
        "completed_exchange_spend_usd": sum(c["cost_usd"] for c in calls),
    }


def summarize(paths):
    rows, runs = [], []
    for path in paths:
        data = json.loads(Path(path).read_text())
        if data["stopped"]:
            raise ValueError("use a completed run or completed resume")
        rows += data["rows"]
        runs.append({"run": Path(path).parent.name, "cap_usd": data["cap_usd"],
                     "spent_usd": data["spent_usd"],
                     "cumulative_spent_usd": data.get("cumulative_spent_usd", data["spent_usd"]),
                     "resumed_from": data.get("resumed_from")})
    identities = [(r["model"], r["seed"], r["mode"], r["question"]) for r in rows]
    if len(identities) != len(set(identities)):
        raise ValueError("duplicate completed questions")
    by_mode, by_state = collections.defaultdict(list), collections.defaultdict(list)
    for row in rows:
        by_mode[row["mode"]].append(row)
        by_state[row["model"], row["seed"], row["mode"]].append(row)
    complete_cost = sum(c["cost_usd"] for r in rows for c in r["calls"])
    total_cost = sum(r["cumulative_spent_usd"] for r in runs)
    return {
        "runs": runs,
        "total_spend_usd": total_cost,
        "incomplete_exchange_spend_usd": max(0, total_cost - complete_cost),
        "by_mode": {mode: metrics(rs) for mode, rs in by_mode.items()},
        "by_state": [{"model": model, "seed": seed, "mode": mode, **metrics(rs)}
                     for (model, seed, mode), rs in sorted(by_state.items())],
        "failures": [{k: r[k] for k in ("model", "seed", "mode", "kind", "question", "expected", "answer", "attempts")}
                     for r in rows if not r["ok"]],
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--development", required=True)
    ap.add_argument("--holdout", nargs="+", required=True)
    ap.add_argument("--tokens", nargs="+", required=True)
    ap.add_argument("--out", default=str(Path(__file__).with_name("query-workflow-results.json")))
    args = ap.parse_args()
    summary = {"date": "2026-11-05", "development_seed": 7, "holdout_seeds": [101, 202, 303],
               "scope": "One synthetic game family, different seeds and table sizes; frozen prompts and scoring on holdout.",
               "development": summarize([args.development]), "holdout": summarize(args.holdout)}
    checks = [json.loads(Path(path).read_text()) for path in args.tokens]
    summary["token_checks"] = {"spent_usd": sum(d["spent_usd"] for d in checks),
                               "rows": [r for d in checks for r in d["rows"]]}
    summary["total_spend_usd"] = (summary["development"]["total_spend_usd"] + summary["holdout"]["total_spend_usd"]
                                  + summary["token_checks"]["spent_usd"])
    Path(args.out).write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n")
    for phase in ("development", "holdout"):
        print(phase)
        for mode, result in summary[phase]["by_mode"].items():
            print(mode, json.dumps(result))
    print(f"total spend ${summary['total_spend_usd']:.6f}; summary: {args.out}")


if __name__ == "__main__":
    main()
