#!/usr/bin/env python3
"""Does letting the game compute arithmetic help? Compare a model answering arithmetic
questions directly with the model writing a gamectx formula expression that the game
evaluates exactly (`gamectx calc`). Reuses model_eval's data, API and spending cap.

    python3 -I eval/calc_eval.py --dry-run
    python3 -I eval/calc_eval.py --key-file PATH --cap 0.05
"""

import argparse
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import model_eval as m  # noqa: E402

CALC_RULE = (' Do not compute it yourself. Reply as {"calc": "<expression>"} using + - * / ( ) and the '
             'exact numbers from the game data; the game evaluates it.')
OUT_TOKENS = 500


def questions(inventory, roster):
    kind = lambda k: [i for i in inventory if i["kind"] == k]
    keepers = [n["trust"] for n in roster if n["faction"] == "keepers"]
    guards = [n["trust"] for n in roster if n["role"] == "guard"]
    heaviest = sorted((i["weight"] for i in inventory), reverse=True)[:3]
    return [
        ("What is the total weight of all food items?", sum(i["weight"] for i in kind("food"))),
        ("What is the total count of all reagent items?", sum(i["count"] for i in kind("reagent"))),
        ("What is the average trust of NPCs in faction keepers?", sum(keepers) / len(keepers)),
        ("What is the total weight of all scroll items, each weight multiplied by its count?",
         sum(i["weight"] * i["count"] for i in kind("scroll"))),
        ("What is the total count of all items in the inventory?", sum(i["count"] for i in inventory)),
        ("What is the combined weight of the three heaviest items?", sum(heaviest)),
        ("What is the difference between the highest and lowest trust among guards?", max(guards) - min(guards)),
        ("What is the total weight of all rare items?", sum(i["weight"] for i in inventory if i["rarity"] == "rare")),
    ]


def close(value, want):
    try:
        return abs(float(value) - want) <= 0.01 + 1e-6 * abs(want)
    except (TypeError, ValueError):
        return False


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--base-url", default="https://api.venice.ai/api/v1")
    ap.add_argument("--key-file", default=os.path.expanduser("~/.config/gamectx-eval/venice.key"))
    ap.add_argument("--models", default=m.DEFAULT_MODELS)
    ap.add_argument("--cap", type=float, default=0.05)
    ap.add_argument("--gamectx", default=os.path.join(m.ROOT, "target", "release", "gamectx"))
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    tools, inventory, roster, _ = m.build_data()
    workdir = os.path.join(m.ROOT, "eval", "results", "calc")
    packets, _ = m.build_packets(workdir, args.gamectx)
    packet = packets["gamectx-grouped"]
    qs = questions(inventory, roster)
    for q, want in qs:
        print(f"  {q} -> {want:.4f}")
    if args.dry_run:
        return

    key = open(args.key_file).read().strip()
    venice = "venice.ai" in args.base_url
    prices = m.get_prices(args.base_url, key) if venice else {}
    spent, rows = 0.0, []
    for model in args.models.split(","):
        price = prices.get(model, m.FALLBACK_PRICE)
        for q, want in qs:
            for fmt, rule in (("direct", m.ANSWER_RULE), ("calc", CALC_RULE)):
                system, user = m.SYSTEM + packet, q + rule
                worst = (len((system + user).encode()) * price["input"] + OUT_TOKENS * price["output"]) / 1e6
                if spent + worst > args.cap:
                    sys.exit(f"stopping: spending cap ${args.cap:.2f} (spent ${spent:.4f})")
                reply = m.chat(args.base_url, key, model, system, user, venice, OUT_TOKENS)
                usage = reply.get("usage", {})
                spent += (usage.get("prompt_tokens", 0) * price["input"] + usage.get("completion_tokens", 0) * price["output"]) / 1e6
                text = reply["choices"][0]["message"].get("content") or ""
                data = m.extract_json(text) or {}
                if fmt == "direct":
                    got = data.get("answer")
                else:
                    expr = data.get("calc")
                    run = subprocess.run([args.gamectx, "calc", str(expr)], capture_output=True, text=True)
                    got = run.stdout.strip() if run.returncode == 0 else None
                rows.append((model, fmt, close(got, want), q, want, got, usage.get("completion_tokens", 0)))
    print(f"\n{'model':<34} {'format':<8} {'correct':>8} {'avg output tokens':>18}")
    for model in args.models.split(","):
        for fmt in ("direct", "calc"):
            r = [x for x in rows if x[0] == model and x[1] == fmt]
            print(f"{model:<34} {fmt:<8} {sum(x[2] for x in r):>4}/{len(r):<3} {sum(x[6] for x in r) / len(r):>18.0f}")
    for x in rows:
        if not x[2]:
            print(f"  wrong [{x[0]} / {x[1]}] {x[3]} wanted {x[4]:.4f}, got {x[5]!r}")
    print(f"\nspent ${spent:.4f}")


if __name__ == "__main__":
    main()
