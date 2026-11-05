#!/usr/bin/env python3
"""Compare sending a game's whole event history with gamectx memory recall.

Builds 60 events ("Velmi lost the iron key at the well"), asks where specific events
happened, and sends either every event as compact JSON or only the 8 events gamectx
recalls for that question. Reuses model_eval's API, scoring and spending cap.

    python3 -I eval/memory_eval.py --dry-run     # recall check only, no API calls
    python3 -I eval/memory_eval.py --key-file PATH --cap 0.05
"""

import argparse
import json
import os
import random
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import model_eval as m  # noqa: E402

ACTORS = ["Velmi", "Midor", "Kalo", "Lodor", "Beknu", "Mimi", "Nubek", "Sharen"]
VERBS = ["lost", "found", "hid", "sold", "repaired", "dropped"]
OBJECTS = ["iron key", "glass lantern", "salt pouch", "bone whistle", "moon map", "rust shears", "ember flask"]
PLACES = ["gate", "hall", "forge", "well", "tower", "cellar", "yard", "chapel"]
RECALL = 8


def build(seed=5):
    rng = random.Random(seed)
    events, seen = [], set()
    while len(events) < 60:
        actor, verb, obj, place = rng.choice(ACTORS), rng.choice(VERBS), rng.choice(OBJECTS), rng.choice(PLACES)
        if (actor, verb, obj) in seen:
            continue
        seen.add((actor, verb, obj))
        events.append({"turn": len(events) + 1, "text": f"{actor} {verb} the {obj} at the {place}.",
                       "tags": [actor.lower(), place], "importance": rng.randint(2, 8),
                       "actor": actor, "verb": verb, "object": obj, "place": place})
    questions = [(f"Where did {e['actor']} {e['verb'].replace('lost', 'lose').replace('found', 'find').replace('hid', 'hide').replace('sold', 'sell').replace('repaired', 'repair').replace('dropped', 'drop')} the {e['object']}? Give the place.", e["place"], e["turn"])
                 for e in rng.sample(events, 10)]
    return events, questions


def memory_json(events):
    return {"next_id": len(events) + 1, "records": [
        {"id": e["turn"], "turn": e["turn"], "importance": e["importance"], "text": e["text"], "tags": e["tags"]}
        for e in events]}


def recalled_packet(memory, question, workdir, gamectx_bin, n):
    manifest = {"sections": [{"name": "memory", "pack": "memory", "value": memory, "relevant_to": question, "count": RECALL}]}
    path = os.path.join(workdir, f"memory-{n}.json")
    json.dump(manifest, open(path, "w"))
    return subprocess.run([gamectx_bin, "pack", path], check=True, capture_output=True, text=True).stdout.strip()


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--base-url", default="https://api.venice.ai/api/v1")
    ap.add_argument("--key-file", default=os.path.expanduser("~/.config/gamectx-eval/venice.key"))
    ap.add_argument("--models", default=m.DEFAULT_MODELS)
    ap.add_argument("--cap", type=float, default=0.05)
    ap.add_argument("--out", default=os.path.join(m.ROOT, "eval", "results"))
    ap.add_argument("--gamectx", default=os.path.join(m.ROOT, "target", "release", "gamectx"))
    ap.add_argument("--formats", default="full-json,gamectx-recall")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    run_dir = os.path.join(args.out, "memory-" + time.strftime("%Y%m%d-%H%M%S"))
    os.makedirs(run_dir, exist_ok=True)
    events, questions = build()
    memory = memory_json(events)
    full = json.dumps({"memory": [{"turn": e["turn"], "text": e["text"], "tags": e["tags"]} for e in events]},
                      separators=(",", ":"))
    recalled = {q: recalled_packet(memory, q, run_dir, args.gamectx, n) for n, (q, _, _) in enumerate(questions)}
    hits = sum(f'"{q_turn},' in recalled[q] or f"[{q_turn}," in recalled[q] for q, _, q_turn in questions)
    avg = sum(len(p) for p in recalled.values()) / len(recalled)
    print(f"full history: {len(full)} bytes; recalled top {RECALL}: {avg:.0f} bytes on average")
    print(f"the asked-about event was recalled for {hits}/{len(questions)} questions")
    if args.dry_run:
        return

    key = open(args.key_file).read().strip()
    venice = "venice.ai" in args.base_url
    prices = m.get_prices(args.base_url, key) if venice else {}
    spent, rows = 0.0, []
    for model in args.models.split(","):
        price = prices.get(model, m.FALLBACK_PRICE)
        for question, place, _ in questions:
            for fmt, packet in (("full-json", full), ("gamectx-recall", recalled[question])):
                if fmt not in args.formats.split(","):
                    continue
                system = m.SYSTEM + packet
                user = question + m.ANSWER_RULE
                worst = (len((system + user).encode()) * price["input"] + m.MAX_OUTPUT_TOKENS * price["output"]) / 1e6
                if spent + worst > args.cap:
                    sys.exit(f"stopping: spending cap ${args.cap:.2f} (spent ${spent:.4f})")
                reply = m.chat(args.base_url, key, model, system, user, venice)
                usage = reply.get("usage", {})
                spent += (usage.get("prompt_tokens", 0) * price["input"] + usage.get("completion_tokens", 0) * price["output"]) / 1e6
                text = reply["choices"][0]["message"].get("content") or ""
                data = m.extract_json(text) or {}
                answer = m.norm(data.get("answer", ""))
                ok = answer.removeprefix("the ") == place  # "the forge" names the same place
                rows.append((model, fmt, ok, usage.get("prompt_tokens", 0), question, place, text[:200]))
    print(f"\n{'model':<34} {'format':<16} {'correct':>8} {'avg input tokens':>17}")
    for model in args.models.split(","):
        for fmt in args.formats.split(","):
            r = [x for x in rows if x[0] == model and x[1] == fmt]
            if not r:
                continue
            print(f"{model:<34} {fmt:<16} {sum(x[2] for x in r):>4}/{len(r):<3} {sum(x[3] for x in r) / len(r):>17.0f}")
    for x in rows:
        if not x[2]:
            print(f"  wrong [{x[0]} / {x[1]}] {x[4]} wanted {x[5]!r}; replied {x[6]!r}")
    print(f"\nspent ${spent:.4f}")
    json.dump({"spent_usd": spent, "rows": rows}, open(os.path.join(run_dir, "results.json"), "w"))


if __name__ == "__main__":
    main()
