#!/usr/bin/env python3
"""Compare plain JSON with gamectx-packed context on real models.

Every question is asked twice with identical wording, once with the game data as compact
JSON and once packed by gamectx, at temperature 0. The script reports answer accuracy
and the input tokens the provider actually billed for each format.

Standard library only. Works with any OpenAI-compatible chat API. Run with `--dry-run` to
build the packets and questions and estimate the cost without calling anything.

    cargo build --release
    python3 -I eval/model_eval.py --dry-run
    python3 -I eval/model_eval.py --key-file ~/.config/gamectx-eval/venice.key
"""

import argparse
import json
import os
import random
import subprocess
import sys
import time
import urllib.error
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_MODELS = "llama-3.2-3b,mistral-small-3-2-24b-instruct"
MAX_OUTPUT_TOKENS = 200
REASON_OUTPUT_TOKENS = 700
# Used for the spending guard when the provider does not publish a price.
FALLBACK_PRICE = {"input": 5.0, "output": 15.0}

SYSTEM = (
    "You are the game logic assistant for Warden Ilse in the keep. Use only the GAME DATA below. "
    "Reply with only a JSON object and nothing else.\n\nGAME DATA:\n"
)
ANSWER_RULE = ' Reply as {"answer": ...}.'
REASON_RULE = (
    " First list the rows that could match, one short line each, then give the final JSON "
    'object {"answer": ...} as the last line.'
)
ACTION_RULE = (
    ' Choose exactly one action from the game data and reply as {"action": "<name>", "args": {...}}, '
    "following every argument rule."
)


# ---------------------------------------------------------------- data

def build_data(seed=7):
    rng = random.Random(seed)
    tools = json.load(open(os.path.join(ROOT, "examples/ember-keep/tools.json")))
    nouns = {
        "tool": ["lantern", "flint", "bell", "shears", "hook", "chisel"],
        "reagent": ["pouch", "flask", "moss", "chip", "dust", "root"],
        "food": ["bread", "trout", "stew", "cake", "pear"],
        "key": ["key"],
        "scroll": ["scroll", "map", "letter"],
        "weapon": ["knife", "staff", "sling"],
    }
    adjectives = ["ember", "salt", "ash", "tide", "moss", "iron", "glass", "quiet",
                  "bone", "moon", "rust", "tallow", "hearth", "ward", "smoke", "frost"]
    rarities = ["common"] * 6 + ["uncommon"] * 3 + ["rare"] * 2 + ["unique"]
    inventory, seen = [], set()
    while len(inventory) < 48:
        kind = rng.choice(list(nouns))
        item_id = f"{rng.choice(adjectives)}_{rng.choice(nouns[kind])}"
        if item_id in seen:
            continue
        seen.add(item_id)
        inventory.append({
            "id": item_id, "kind": kind, "rarity": rng.choice(rarities),
            "count": rng.randint(1, 20), "weight": round(rng.uniform(0.1, 6.0), 2),
        })
    syllables = ["ka", "lo", "mi", "ren", "tas", "vel", "dor", "isa", "bek", "nu", "sha", "orn"]
    roster, names = [], set()
    while len(roster) < 20:
        name = (rng.choice(syllables) + rng.choice(syllables)).capitalize()
        if name in names:
            continue
        names.add(name)
        roster.append({
            "name": name,
            "faction": rng.choice(["keepers", "ashfolk", "tidewardens", "wanderers"]),
            "role": rng.choice(["guard", "smith", "healer", "scout", "cook"]),
            "location": rng.choice(["gate", "hall", "forge", "well", "tower", "cellar", "yard", "chapel"]),
            "mood": rng.choice(["calm", "wary", "stern", "warm"]),
            "trust": round(rng.random(), 2),
        })
    rules = ("Ilse guards the keep. The player has handle P1. Entity handles are given in each request. "
             "The game validates every action; invalid actions are refused, not repaired.")
    return tools, inventory, roster, rules


def build_packets(workdir, gamectx_bin):
    tools, inventory, roster, rules = build_data()
    os.makedirs(workdir, exist_ok=True)
    manifest = {"sections": [
        {"name": "actions", "pack": "schema", "value": tools, "priority": 10, "stable": True, "required": True},
        {"name": "rules", "pack": "raw", "value": rules, "priority": 9, "stable": True, "required": True},
        {"name": "inventory", "pack": "table", "value": inventory, "priority": 6},
        {"name": "npcs", "pack": "table", "value": roster, "priority": 5},
    ]}
    packets = {"json": json.dumps({"actions": tools, "rules": rules, "inventory": inventory, "npcs": roster},
                                  separators=(",", ":"), ensure_ascii=False)}
    packets["gamectx"] = pack(manifest, os.path.join(workdir, "manifest.json"), gamectx_bin)
    # Grouped by the columns the questions filter on most: item kind and NPC faction.
    manifest["sections"][2]["group_by"] = "kind"
    manifest["sections"][3]["group_by"] = "faction"
    packets["gamectx-grouped"] = pack(manifest, os.path.join(workdir, "manifest-grouped.json"), gamectx_bin)
    # Same grouping with rows sorted inside each group (gamectx views).
    manifest["sections"][2]["sort_by"] = "-count"
    manifest["sections"][3]["sort_by"] = "role"
    packets["grouped-sorted"] = pack(manifest, os.path.join(workdir, "manifest-sorted.json"), gamectx_bin)
    # Grouped tables plus action signatures instead of packed JSON Schema.
    manifest["sections"][0]["pack"] = "signatures"
    manifest["sections"][2].pop("sort_by")
    manifest["sections"][3].pop("sort_by")
    packets["gamectx-signatures"] = pack(manifest, os.path.join(workdir, "manifest-signatures.json"), gamectx_bin)
    # Inventory grouped by kind, then rarity: the two columns its filter questions use.
    manifest["sections"][2]["group_by"] = ["kind", "rarity"]
    packets["gamectx-nested"] = pack(manifest, os.path.join(workdir, "manifest-nested.json"), gamectx_bin)
    return packets, (tools, inventory, roster)


def pack(manifest, path, gamectx_bin):
    json.dump(manifest, open(path, "w"))
    return subprocess.run([gamectx_bin, "pack", path], check=True, capture_output=True, text=True).stdout.strip()


# ---------------------------------------------------------------- questions

def build_questions(tools, inventory, roster, seed=11):
    rng = random.Random(seed)
    q = []
    for item in rng.sample(inventory, 5):
        q.append(("inv_count", f"What is the count of item `{item['id']}` in the inventory?", "int", item["count"]))
    for item in rng.sample(inventory, 4):
        q.append(("inv_kind", f"What kind of item is `{item['id']}`?", "str", item["kind"]))
    for item in rng.sample(inventory, 4):
        q.append(("inv_rarity", f"What is the rarity of `{item['id']}`?", "str", item["rarity"]))
    combos = {}
    for item in inventory:
        combos.setdefault((item["rarity"], item["kind"]), []).append(item["id"])
    good = sorted(k for k, v in combos.items() if 2 <= len(v) <= 5)
    for rarity, kind in rng.sample(good, min(6, len(good))):
        q.append(("inv_filter", f"List the ids of every {rarity} item of kind {kind}, as an array.", "set", combos[(rarity, kind)]))
    for threshold in (18, 15, 3):
        if threshold == 3:
            want = [i["id"] for i in inventory if i["count"] <= threshold]
            text = f"List the ids of every item whose count is at most {threshold}, as an array."
        else:
            want = [i["id"] for i in inventory if i["count"] >= threshold]
            text = f"List the ids of every item whose count is at least {threshold}, as an array."
        q.append(("inv_numeric", text, "set", want))
    for npc in rng.sample(roster, 4):
        q.append(("npc_where", f"Where is {npc['name']}? Give the location value.", "str", npc["location"]))
    groups = {}
    for npc in roster:
        groups.setdefault((npc["faction"], npc["role"]), []).append(npc["name"])
    good = sorted(k for k, v in groups.items() if len(v) >= 1)
    for faction, role in rng.sample(good, min(6, len(good))):
        q.append(("npc_filter", f"List the names of every NPC whose faction is {faction} and role is {role}, as an array.", "set", groups[(faction, role)]))
    places = {}
    for npc in roster:
        places.setdefault((npc["role"], npc["location"]), []).append(npc["name"])
    for role, location in rng.sample(sorted(places), min(3, len(places))):
        q.append(("npc_cross", f"List the names of every NPC whose role is {role} and location is {location}, as an array.", "set", places[(role, location)]))
    by_name = {t["function"]["name"]: t["function"]["parameters"]["properties"] for t in tools}
    for tool, param in [("move_to", "spot"), ("speak", "tone"), ("cast_spell", "spell")]:
        q.append(("schema_enum", f"List every allowed value of the `{param}` argument of action `{tool}`, as an array.", "set", by_name[tool][param]["enum"]))
    for tool, param in [("use_item", "power"), ("guard", "seconds"), ("light_lantern", "power")]:
        q.append(("schema_range", f"What is the maximum allowed value of `{param}` for action `{tool}`?", "int", by_name[tool][param]["maximum"]))
    for tool in ["give_item", "speak"]:
        q.append(("schema_params", f"List the argument names of action `{tool}`, as an array.", "set", list(by_name[tool])))
    food = next(i for i in inventory if i["kind"] == "food")
    reagent = next(i for i in inventory if i["kind"] == "reagent")
    q += [
        ("action", "The player (P1) is badly hurt and asks Ilse to heal them as strongly as she can.", "action",
         {"action": "cast_spell", "args": {"spell": "mend", "target": "P1", "power": 5}}),
        ("action", "An ash hound (handle H7) is creeping toward the player. Ilse holds position and watches it for 20 seconds.", "action",
         {"action": "guard", "args": {"target": "H7", "seconds": 20}}),
        ("action", "Ilse decides to sneak down to the cellar.", "action",
         {"action": "move_to", "args": {"spot": "cellar", "pace": "sneak"}}),
        ("action", f"The player (P1) is hungry. Ilse hands them one item from her inventory: {food['id'].replace('_', ' ')}.", "action",
         {"action": "give_item", "args": {"item": food["id"], "target": "P1"}}),
        ("action", "Ilse wants to wait for a full minute, as long as one action allows.", "action",
         {"action": "wait", "args": {"seconds": 30}}),
        ("action", "Ilse lights the lantern with handle L2 as brightly as she can.", "action",
         {"action": "light_lantern", "args": {"lantern": "L2", "power": 5}}),
        ("action", "Ilse sternly tells the player (P1) to leave the forge.", "action",
         {"action": "speak", "args": {"target": "P1", "tone": "stern"}}),
        ("action", f"Ilse uses her {reagent['id'].replace('_', ' ')} on the ash hound (handle H7) at power 2.", "action",
         {"action": "use_item", "args": {"item": reagent["id"], "target": "H7", "power": 2}}),
        ("action", "Ilse runs to the gate.", "action",
         {"action": "move_to", "args": {"spot": "gate", "pace": "run"}}),
        ("action", "Ilse casts a hush spell on the crow sentinel (handle C3) at the weakest power.", "action",
         {"action": "cast_spell", "args": {"spell": "hush", "target": "C3", "power": 1}}),
    ]
    return q, by_name


# ---------------------------------------------------------------- scoring

def extract_json(text):
    """Returns the last JSON object in the reply, so brief reasoning before it is allowed."""
    text = text.strip()
    if "</think>" in text:
        text = text.split("</think>")[-1]
    decoder = json.JSONDecoder()
    found, i = None, text.find("{")
    while i >= 0:
        try:
            value, end = decoder.raw_decode(text, i)
        except json.JSONDecodeError:
            i = text.find("{", i + 1)
            continue
        if isinstance(value, dict):
            found = value
        i = text.find("{", end)  # skip objects nested inside this one
    return found


def norm(value):
    return str(value).strip().strip("`").lower()


def valid_args(args, schema):
    """Checks args against the tool's strict property rules; returns a problem or None."""
    if not isinstance(args, dict) or set(args) != set(schema):
        return f"argument names {sorted(args) if isinstance(args, dict) else args} != {sorted(schema)}"
    for name, rule in schema.items():
        value = args[name]
        if rule.get("type") == "integer":
            if not isinstance(value, int) or isinstance(value, bool):
                return f"{name} is not an integer"
            if not rule.get("minimum", value) <= value <= rule.get("maximum", value):
                return f"{name}={value} outside {rule.get('minimum')}..{rule.get('maximum')}"
        elif rule.get("type") == "string":
            if not isinstance(value, str):
                return f"{name} is not a string"
            if "enum" in rule and value not in rule["enum"]:
                return f"{name}={value!r} not in enum"
            if "maxLength" in rule and len(value) > rule["maxLength"]:
                return f"{name} too long"
    return None


def score(kind, expected, reply, schemas):
    data = extract_json(reply)
    if data is None:
        return False, "no JSON object in reply"
    if kind == "action":
        if data.get("action") != expected["action"]:
            return False, f"action {data.get('action')!r}"
        problem = valid_args(data.get("args"), schemas[expected["action"]])
        if problem:
            return False, problem
        for key, want in expected["args"].items():
            if norm(data["args"].get(key)) != norm(want):
                return False, f"{key}={data['args'].get(key)!r}, wanted {want!r}"
        return True, ""
    answer = data.get("answer")
    if kind == "int":
        try:
            ok = int(answer) == expected
        except (TypeError, ValueError):
            ok = False
    elif kind == "str":
        ok = norm(answer) == norm(expected)
    else:
        ok = isinstance(answer, list) and {norm(a) for a in answer} == {norm(e) for e in expected}
    return ok, "" if ok else f"answered {answer!r}"


# ---------------------------------------------------------------- API

def get_prices(base_url, key):
    request = urllib.request.Request(base_url + "/models", headers={"Authorization": f"Bearer {key}"})
    with urllib.request.urlopen(request, timeout=30) as response:
        models = json.load(response).get("data", [])
    prices = {}
    for m in models:
        pricing = m.get("model_spec", {}).get("pricing", {})
        if "input" in pricing and "output" in pricing:
            prices[m["id"]] = {"input": float(pricing["input"]["usd"]), "output": float(pricing["output"]["usd"])}
    return prices


def chat(base_url, key, model, system, user, venice, max_tokens=MAX_OUTPUT_TOKENS):
    body = {
        "model": model,
        "messages": [{"role": "system", "content": system}, {"role": "user", "content": user}],
        "temperature": 0,
        "max_completion_tokens": max_tokens,
    }
    if venice:
        body["venice_parameters"] = {"include_venice_system_prompt": False, "enable_web_search": "off"}
    request = urllib.request.Request(
        base_url + "/chat/completions", data=json.dumps(body).encode(), method="POST",
        headers={"Authorization": f"Bearer {key}", "Content-Type": "application/json"})
    for attempt in range(4):
        try:
            with urllib.request.urlopen(request, timeout=90) as response:
                return json.load(response)
        except urllib.error.HTTPError as e:
            if e.code in (429, 500, 502, 503, 504) and attempt < 3:
                time.sleep(2 ** attempt * 3)
                continue
            raise RuntimeError(f"HTTP {e.code}: {e.read()[:300]!r}") from None
    raise RuntimeError("unreachable")


# ---------------------------------------------------------------- main

def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--base-url", default="https://api.venice.ai/api/v1")
    ap.add_argument("--key-file", default=os.path.expanduser("~/.config/gamectx-eval/venice.key"))
    ap.add_argument("--models", default=DEFAULT_MODELS)
    ap.add_argument("--cap", type=float, default=1.0, help="stop before spending more than this many USD")
    ap.add_argument("--out", default=os.path.join(ROOT, "eval", "results"))
    ap.add_argument("--gamectx", default=os.path.join(ROOT, "target", "release", "gamectx"))
    ap.add_argument("--formats", default="json,gamectx-grouped,gamectx-signatures,gamectx-nested")
    ap.add_argument("--kinds", default="", help="only these question kinds, comma-separated")
    ap.add_argument("--tokens-only", action="store_true", help="measure billed input tokens per format, 1-token replies")
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    run_dir = os.path.join(args.out, time.strftime("%Y%m%d-%H%M%S"))
    packets, (tools, inventory, roster) = build_packets(run_dir, args.gamectx)
    questions, schemas = build_questions(tools, inventory, roster)
    if args.kinds:
        questions = [q for q in questions if q[0] in args.kinds.split(",")]
    models = [m for m in args.models.split(",") if m]
    formats = [f for f in args.formats.split(",") if f]
    unknown = {f.removesuffix("+reason") for f in formats} - set(packets)
    if unknown:
        sys.exit(f"unknown formats: {sorted(unknown)}; choose from {sorted(packets)}")
    calls = len(questions) * len(formats) * len(models)
    print("packets: " + ", ".join(f"{f} {len(packets[f.removesuffix('+reason')].encode())} bytes" for f in formats))
    print(f"{len(questions)} questions x {len(formats)} formats x {len(models)} models = {calls} calls; cap ${args.cap:.2f}")
    json.dump({"packets": packets, "questions": questions}, open(os.path.join(run_dir, "inputs.json"), "w"), indent=1)
    if args.dry_run:
        print(f"dry run: inputs written to {run_dir}")
        return

    key = open(args.key_file).read().strip()
    if not key:
        sys.exit("key file is empty")
    venice = "venice.ai" in args.base_url
    prices = get_prices(args.base_url, key) if venice else {}
    if args.tokens_only:
        print(f"\n{'model':<34} " + " ".join(f"{f:>16}" for f in formats))
        for model in models:
            counts = []
            for fmt in formats:
                reply = chat(args.base_url, key, model, SYSTEM + packets[fmt.removesuffix("+reason")], "Reply with {}.", venice, 1)
                counts.append(reply.get("usage", {}).get("prompt_tokens", 0))
            base = counts[0] or 1
            print(f"{model:<34} " + " ".join(f"{c:>9} ({100 * (c - base) / base:+4.0f}%)" for c in counts))
        return
    spent, results, stopped = 0.0, [], None
    for model in models:
        price = prices.get(model, FALLBACK_PRICE)
        for n, (kind, question, answer_type, expected) in enumerate(questions):
            for fmt in (formats if n % 2 == 0 else formats[::-1]):
                reason = fmt.endswith("+reason") and answer_type != "action"
                system = SYSTEM + packets[fmt.removesuffix("+reason")]
                user = question + (ACTION_RULE if answer_type == "action" else REASON_RULE if reason else ANSWER_RULE)
                out_tokens = REASON_OUTPUT_TOKENS if reason else MAX_OUTPUT_TOKENS
                # Worst case: one token per byte in, full output allowance out.
                worst = (len((system + user).encode()) * price["input"] + out_tokens * price["output"]) / 1e6
                if spent + worst > args.cap:
                    stopped = f"stopped before {model}/{kind}/{fmt}: spending cap ${args.cap:.2f} (spent ${spent:.4f})"
                    break
                try:
                    reply = chat(args.base_url, key, model, system, user, venice, out_tokens)
                    text = reply["choices"][0]["message"].get("content") or ""
                    usage = reply.get("usage", {})
                except Exception as e:  # noqa: BLE001 - record and continue
                    text, usage = "", {}
                    print(f"  {model} {kind} {fmt}: request failed: {e}")
                cost = (usage.get("prompt_tokens", 0) * price["input"] + usage.get("completion_tokens", 0) * price["output"]) / 1e6
                spent += cost
                ok, why = score(answer_type, expected, text, schemas) if text else (False, "no reply")
                cached = (usage.get("prompt_tokens_details") or {}).get("cached_tokens", 0) or 0
                results.append({"model": model, "kind": kind, "format": fmt, "question": question, "ok": ok, "why": why,
                                "reply": text[:400], "prompt_tokens": usage.get("prompt_tokens"), "cached_tokens": cached,
                                "completion_tokens": usage.get("completion_tokens"), "cost": cost})
            if stopped:
                break
            print(f"  {model}: {n + 1}/{len(questions)} questions, spent ${spent:.4f}", end="\r")
        print()
        if stopped:
            break

    json.dump({"models": models, "formats": formats, "spent_usd": spent, "stopped": stopped, "results": results},
              open(os.path.join(run_dir, "results.json"), "w"), indent=1)
    report(results, models, formats, spent, stopped, run_dir)


def report(results, models, formats, spent, stopped, run_dir):
    print(f"\n{'model':<34} {'format':<28} {'correct':>9} {'avg in':>7} {'avg out':>8} {'cached':>7}")
    for model in models:
        for fmt in formats:
            rows = [r for r in results if r["model"] == model and r["format"] == fmt]
            if not rows:
                continue
            ok = sum(r["ok"] for r in rows)
            tokens = [r["prompt_tokens"] for r in rows if r["prompt_tokens"]]
            avg = sum(tokens) / len(tokens) if tokens else 0
            cached = sum(r["cached_tokens"] for r in rows)
            outs = [r["completion_tokens"] for r in rows if r["completion_tokens"]]
            avg_out = sum(outs) / len(outs) if outs else 0
            print(f"{model:<34} {fmt:<28} {ok:>4}/{len(rows):<4} {avg:>7.0f} {avg_out:>8.0f} {cached:>7}")
    print(f"\naccuracy by question type, all models ({' / '.join(formats)}):")
    for kind in dict.fromkeys(r["kind"] for r in results):
        line = []
        for fmt in formats:
            rows = [r for r in results if r["kind"] == kind and r["format"] == fmt]
            line.append(f"{sum(r['ok'] for r in rows)}/{len(rows)}")
        print(f"  {kind:<14} " + "  ".join(f"{x:>6}" for x in line))
    print("\nwrong answers:")
    for r in results:
        if not r["ok"]:
            print(f"  [{r['model']} / {r['format']}] {r['kind']}: {r['why']}")
    print(f"\nspent ${spent:.4f}" + (f"; {stopped}" if stopped else ""))
    print(f"full results: {run_dir}/results.json")


if __name__ == "__main__":
    main()
