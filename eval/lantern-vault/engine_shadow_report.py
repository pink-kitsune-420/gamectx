"""Summarize a Lantern Vault engine shadow log (GAMECTX_SHADOW=<file>): for every chat turn
the stage Lantern Vault used and the route a fresh RoleplayEngine chose. Offline, $0."""
import collections, json, sys

def lv_stage(r):
    lv = r['lv']
    if lv.get('error'): return 'error'
    if not r['active'] and 'status' in lv: return 'command'
    if 'out_of_character' in lv: return 'out_of_character'
    if 'roleplay_action' in lv: return 'stage_action'
    if lv.get('status') == 'blocked': return 'stage_action'
    if 'stage' in lv and 'greeting' not in lv: return 'direction'
    if lv.get('status') == 'clarify': return 'command'
    if 'status' in lv or 'id' in lv: return 'command'
    if 'reaction' in lv: return 'reaction'
    if 'greeting' in lv: return 'greeting'
    if 'disclosure_ack' in lv: return 'disclosure'
    if 'scoped_recall' in lv: return 'recall'
    if 'current_perception' in lv or 'perceived_or_public' in lv: return 'perception'
    if 'stock_query' in lv: return 'stock'
    if 'debt_query' in lv: return 'secret'
    if 'saved_viewpoint' in lv: return 'viewpoint'
    if 'expression' in lv: return 'expression'
    if 'social' in lv: return 'social'
    if lv.get('direct'): return 'authored'
    if lv.get('addressed') is False: return 'absent'
    return 'model'

SAME = {('greeting', 'greeting'), ('social', 'social'), ('disclosure', 'disclosure'), ('recall', 'recall'), ('perception', 'perception'),
        ('command', 'command'), ('command', 'stage_action'), ('command', 'props'), ('command', 'entrances'), ('stage_action', 'stage_action'), ('direction', 'direction'),
        ('out_of_character', 'out_of_character'), ('model', 'model'), ('expression', 'model'), ('secret', 'secrets'), ('viewpoint', 'opinions')}

def main(path):
    rows = [json.loads(l) for l in open(path)]
    table = collections.Counter(); text_same = collections.Counter(); examples = collections.defaultdict(list)
    for r in rows:
        a, b = lv_stage(r), r['engine']['route']
        table[(a, b)] += 1
        said = [x.get('text') for x in r['said'] if isinstance(x, dict)]
        if (a, b) in SAME and r['engine'].get('text') and r['engine']['text'] in said: text_same[a] += 1
        if (a, b) not in SAME and len(examples[(a, b)]) < 3: examples[(a, b)].append(r['text'][:80])
    by = collections.defaultdict(lambda: [0, 0])
    for (a, b), n in table.items(): by[a][0] += n; by[a][1] += n * ((a, b) in SAME)
    print(f'turns {len(rows)}; route agreement {sum(v[1] for v in by.values())}/{len(rows)}')
    for a, (n, ok) in sorted(by.items(), key=lambda kv: -kv[1][0]):
        print(f'  {a:17} {ok:5}/{n:<5} same text {text_same[a]:4}   engine: ' + ', '.join(f'{b}={m}' for (x, b), m in table.most_common() if x == a))
    print('disagreement examples:')
    for k, v in sorted(examples.items(), key=lambda kv: -table[kv[0]])[:24]: print(' ', table[k], k, v)

if __name__ == '__main__': main(sys.argv[1])
