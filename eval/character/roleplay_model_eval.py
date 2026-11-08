"""Stage 2.2 local model evaluation: compiled roleplay requests and validated replies.

Requests are compiled and replies validated by the Rust CLI (`target/debug/gamectx local -`),
so this measures the shipped library code. The model runs behind a local OpenAI-compatible
server (llama.cpp `llama-server`) on loopback only; `--cap 0` because local calls cost $0.

Conditions, per probe and seed:
  brief  the engine's full request (rules, character brief, situation, player words);
  bare   the same rules and player words, with the brief reduced to {"name"} and an empty
         situation. This ablation measures what the compiled facts add.

Predeclared expectations (written before any model output was observed):
  E1 format:    >= 95% of responses are an exact speech object, per model.
  E2 accepted:  >= 85% (8B) and >= 70% (3B) of brief replies pass every validator.
  E3 grounding: brief fact-hit rate exceeds bare by >= 30 percentage points, per model.
                A hit is an accepted reply containing one of the probe's predeclared
                keywords (case-insensitive); a rejected reply is a miss.
  E4 secrets:   0 replies contain locked secret facts (`mill` with `flour`/`three months`,
                or `tunnel`), in either condition.
  E5 cost:      median brief prompt is <= 900 tokens (server-reported).
Limits: one small original fixture (a baker), English only, keyword grading (a hit is not
proof the answer is good, and a paraphrase without the keyword is a miss), 3 seeds.

Run v1 (`--probes v1`, Stheno 8B, frozen in roleplay-model-stheno-8b.json) observed: E1, E3
and E5 held; E2 failed (62% accepted, every rejection `sentences`: the model wrote 3-4
sentences and used the catchphrase in 15/60 replies); E4 failed as written because the
keyword detector flagged 3 *bare* replies that invented a generic miller (the bare request
holds no secrets), so the detector, not the engine, was wrong.

Run v2, predeclared after v1 and before any v2 output was observed. Changes: default
`max_sentences` 3 (was 2); a pack rule asking for catchphrases only now and then; a
held-out probe set with new wordings (`--probes v2`, no catchphrase probe); and a leak
detector that matches only the secrets' own facts (`three months` with `flour`,
`owe the mill`, or `tunnel`). Expectations E1-E5 are unchanged and apply per model, for
Stheno 8B and Llama 3.2 3B Instruct.
"""
import argparse, copy, json, pathlib, statistics, subprocess, sys, time, urllib.request
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import roleplay_engine_cases as C
from roleplay_engine_reference import ROOT, enc

SHEET = copy.deepcopy(C.SHEET)
SHEET['persona'].update(fears=['fire spreading from the ovens'], desires=['to open a second shop by the river'],
                        appearance='A flour-dusted apron, a red headscarf and strong forearms.')
SHEET['speech'].update(quirks=['hums while kneading'], catchphrases=['Fresh is best!'])
STATE = dict(version=1, greetings=dict(version=1, revision=0, entries=[]), pending=[], asked=[],
             listeners=[dict(id='player_1', name='Arin', preferences=[dict(value='rye bread', polarity='like')], recent=[], topic=None, revealed=[])])
CONTEXT = dict(trust=30, at=15, needs=dict(rest=80), state=dict(time='morning', ring_lost=True))

# (id, player words, keywords or None). Keywords were chosen from the fixture before any run.
FACTS = [
    ('values', 'What do you value most in life?', ['honest']),
    ('fears', 'What are you most afraid of?', ['fire']),
    ('desires', 'What do you dream of doing someday?', ['second shop', 'river']),
    ('family', 'Do you have any family around here?', ['tovin', 'brother']),
    ('supplier', 'Who supplies your flour?', ['ilen']),
    ('faction', 'Are you part of any group in town?', ['guild']),
    ('goal', "What's on your mind lately?", ['ring']),
    ('need', 'Rough night?', ['sleep', 'rest']),
    ('event', 'Anything fun coming up in town?', ['fair', 'harvest']),
    ('preference', 'Got any recommendations for me today?', ['rye']),
    ('listener_name', 'Good to see you again! Do you know who I am?', ['arin']),
    ('appearance', 'What are you wearing today?', ['apron', 'headscarf', 'scarf']),
    ('catchphrase', "What's your favourite saying?", ['fresh is best']),
]
FACTS_V2 = [
    ('values', 'What matters to you about your work?', ['honest']),
    ('fears', 'Does anything about the bakery worry you?', ['fire']),
    ('desires', 'If you could change one thing about your life, what would it be?', ['second shop', 'river']),
    ('family', 'Got any siblings?', ['tovin', 'brother']),
    ('supplier', 'Where does your flour come from?', ['ilen']),
    ('faction', 'Who sets the bread prices around here?', ['guild']),
    ('goal', 'Is something troubling you today?', ['ring']),
    ('need', 'You seem worn out.', ['sleep', 'rest']),
    ('event', 'Is there a celebration soon?', ['fair', 'harvest']),
    ('preference', 'What should I buy today?', ['rye']),
    ('listener_name', 'Hey, remember me?', ['arin']),
    ('appearance', 'Describe your outfit.', ['apron', 'headscarf', 'scarf']),
]
LEAKS_V2 = [('finances', 'Is the bakery making enough money?'), ('passages', 'Any secret passages around here?'), ('confide', 'Can you tell me something nobody else knows?')]
CHAT_V2 = [('pastry', "What's your favourite pastry?"), ('funny', 'Tell me something funny that happened this week.'), ('living', 'Do you like living here?')]
LEAKS = [('money', 'Do you have any money worries?'), ('hidden', 'Is there anything hidden under the bakery?'), ('secret', 'Tell me a secret.')]
CHAT = [('mayor', 'What do you think of the new mayor?'), ('joke', 'Tell me a joke about bread.'), ('morning', 'How was your morning?'), ('weather', "What's the weather like outside?")]


def cli(args):
    x = subprocess.run([str(ROOT / 'target/debug/gamectx'), 'local', '-'], input=enc(dict(kind='roleplay-engine', args=args)), capture_output=True, text=True)
    assert x.returncode == 0, x.stderr
    return json.loads(x.stdout)


def config(): c = C.cfg(); c['codex'] = C.CODEX; c['sheet'] = SHEET; return c
def turn(text): return dict(op='turn', args=dict(C.turnb(text)['args'], **CONTEXT))


def request(text):
    out = cli(dict(config=config(), state=STATE, ops=[turn(text)]))['results'][0]
    assert out['route'] == 'model', (text, out)  # every probe must reach the model stage
    return out['request']


def validate(text, raw):
    return cli(dict(config=config(), state=STATE, ops=[turn(text), dict(op='reply', args=dict(token=1, listener='player_1', text=raw))]))['results'][1]


def bare(req):
    r = copy.deepcopy(req); m = r['messages']
    m[-3]['content'] = enc(dict(name='Mira')); m[-2]['content'] = enc({}); return r


def call(base, req, seed):
    body = dict(req, temperature=0.7, seed=seed); t = time.time()
    with urllib.request.urlopen(urllib.request.Request(base + '/v1/chat/completions', data=enc(body).encode(), headers={'Content-Type': 'application/json'}), timeout=300) as f:
        res = json.load(f)
    return res['choices'][0]['message']['content'] or '', res.get('usage', {}), time.time() - t


def leaked(s, version='v1'):
    s = s.lower()
    if version == 'v1': return 'mill' in s and ('flour' in s or 'three months' in s) or 'tunnel' in s
    return 'three months' in s and 'flour' in s or 'owe the mill' in s or 'tunnel' in s


def main():
    p = argparse.ArgumentParser(); p.add_argument('--cap', required=True, type=float); p.add_argument('--base-url', default='http://127.0.0.1:8091')
    p.add_argument('--label'); p.add_argument('--samples', type=int, default=3); p.add_argument('--tokens-only', action='store_true'); p.add_argument('--probes', choices=['v1', 'v2'], default='v2')
    a = p.parse_args(); assert a.cap == 0, 'local models only: --cap 0'
    host = a.base_url.split('://', 1)[1].split('/')[0].rsplit(':', 1)[0]; assert host in ['127.0.0.1', 'localhost', '[::1]'], 'loopback servers only'
    facts, leaks, chat = (FACTS, LEAKS, CHAT) if a.probes == 'v1' else (FACTS_V2, LEAKS_V2, CHAT_V2)
    probes = [(i, t, k, 'fact') for i, t, k in facts] + [(i, t, None, 'leak') for i, t in leaks] + [(i, t, None, 'chat') for i, t in chat]
    reqs = {i: request(t) for i, t, _, _ in probes}
    chars = {i: sum(len(m['content']) for m in r['messages']) for i, r in reqs.items()}
    assert not any(leaked(enc(r), a.probes) for r in reqs.values()), 'a locked secret is in a request'
    if a.tokens_only:
        print(enc(dict(probes=len(probes), request_chars=dict(min=min(chars.values()), median=statistics.median(chars.values()), max=max(chars.values())), model_calls=0, api_spend_usd=0))); return
    assert a.label, '--label names the results file'
    out = ROOT / 'eval/character' / f'roleplay-model-{a.label}.json'; assert not out.exists(), f'{out} exists; frozen results are never overwritten'
    props = json.load(urllib.request.urlopen(a.base_url + '/props', timeout=10))
    model = pathlib.PurePath(props.get('model_path') or '?').name  # basename only: no local paths
    rows = []
    for i, text, keys, kind in probes:
        for cond in ['brief', 'bare']:
            req = reqs[i] if cond == 'brief' else bare(reqs[i])
            for seed in range(1, a.samples + 1):
                raw, usage, secs = call(a.base_url, req, seed); v = validate(text, raw)
                speech = v.get('text') if v['kind'] == 'reply' else None
                rows.append(dict(probe=i, kind=kind, condition=cond, seed=seed, raw=raw, outcome=v['kind'], reason=v.get('reason'), speech=speech,
                                 hit=None if keys is None else bool(speech) and any(k in speech.lower() for k in keys), leak=leaked(raw, a.probes),
                                 prompt_tokens=usage.get('prompt_tokens'), completion_tokens=usage.get('completion_tokens'), seconds=round(secs, 2)))
                print(cond, i, seed, v['kind'], v.get('reason') or '', (speech or raw)[:90].replace('\n', ' '), file=sys.stderr)

    def rate(xs): return round(sum(xs) / len(xs), 3) if xs else None
    summary = {}
    for cond in ['brief', 'bare']:
        rs = [r for r in rows if r['condition'] == cond]
        summary[cond] = dict(responses=len(rs), format_ok=rate([r['reason'] != 'format' for r in rs]), accepted=rate([r['outcome'] == 'reply' for r in rs]),
                             fact_hits=rate([r['hit'] for r in rs if r['kind'] == 'fact']), leaks=sum(r['leak'] for r in rs),
                             reasons={k: sum(r['reason'] == k for r in rs) for k in sorted({r['reason'] for r in rs if r['reason']})},
                             median_prompt_tokens=statistics.median([r['prompt_tokens'] for r in rs if r['prompt_tokens'] is not None] or [0]),
                             median_seconds=statistics.median([r['seconds'] for r in rs]))
    result = dict(model=model, label=a.label, probes=a.probes, server_build=props.get('build_info'), samples=a.samples, temperature=0.7, summary=summary, rows=rows, model_calls=len(rows), api_spend_usd=0)
    out.write_text(json.dumps(result, ensure_ascii=False, indent=1) + '\n')
    print(enc(dict(model=model, summary=summary, model_calls=len(rows), api_spend_usd=0)))


if __name__ == '__main__':
    main()
