"""Place kinds, nesting and nearby-place cases over an original codex (Ember Vale).
Expectations were written before running; the `places` local kind must match
places_reference.py byte for byte. Offline only (--cap 0).

  python3 -I eval/knowledge/places_cases.py --cap 0 [--write-conformance] [--random N --seed S]
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse, copy, random, subprocess
import places_reference as pr
from roleplay_engine_reference import enc

ROOT = _pl.Path(__file__).resolve().parents[2]


def place(id_, label, kind=None, within=None, **kw):
    out = dict(id=id_, label=label)
    if kind is not None: out['kind'] = kind
    if within is not None: out['within'] = within
    out.update(kw); return out


def door(id_, label, a, b, **kw): return dict(id=id_, label=label, sides=[dict(place=a, description='A door.'), dict(place=b, description='A door.')], **kw)


VALE = dict(version=1, place_kinds=[dict(id='sky_dock', label='sky dock', scale='site', indoor=False)], places=[
    place('ember_vale', 'Ember Vale', 'valley'),
    place('emberford', 'Emberford', 'village', 'ember_vale', links=[dict(place='whisper_woods', distance=5, direction='north'), dict(place='keep', distance=3, direction='east')]),
    place('bakery', 'the bakery', 'bakery', 'emberford', tags=['food', 'warm']),
    place('smithy', 'the smithy', 'smithy', 'emberford'),
    place('square', 'the square', 'square', 'emberford'),
    place('well', 'the old well', 'well', 'emberford'),
    place('cellar', 'the bakery cellar', 'cellar', 'bakery'),
    place('whisper_woods', 'Whisper Woods', 'forest', 'ember_vale'),
    place('clearing', 'the clearing', 'clearing', 'whisper_woods', links=[dict(place='wolf_cave', distance=2, direction='west')]),
    place('wolf_cave', 'the wolf cave', 'cave', 'whisper_woods'),
    place('keep', 'Ember Keep', 'castle', 'ember_vale'),
    place('great_hall', 'the great hall', 'great_hall', 'keep'),
    place('vault', 'the vault', 'vault', 'keep'),
    place('dock', 'the sky dock', 'sky_dock'),
], entrances=[
    door('bakery_door', 'the bakery door', 'bakery', 'square'),
    door('cellar_hatch', 'the cellar hatch', 'bakery', 'cellar', distance=0),
    door('smithy_door', 'the smithy door', 'smithy', 'square'),
    door('vault_door', 'the vault door', 'great_hall', 'vault', passage='one_way'),
])


def q(**kw): return kw
def run(*queries, codex=VALE): return dict(codex=codex, queries=list(queries))
def res(out, i=0): r = out['results'][i]; assert r['ok'], r; return r['result']
def ids(rows): return [r['place'] for r in rows]
def routes(want, i=0): return lambda out: [(r['place'], r['distance']) for r in res(out, i)['routes']] == want
def around(want, i=0): return lambda out: [(r['place'], r['within']) for r in res(out, i)['around']] == want
def failed(i=0): return lambda out: not out['results'][i]['ok']
def both(*fs): return lambda out: all(f(out) for f in fs)
def with_place(p, **kw): return dict(VALE, places=[dict(x, **kw) if x['id'] == p else x for x in VALE['places']])


CASES = [
    ('from_bakery', run(q(**{'from': 'bakery'})), both(
        lambda o: [(h['place'], h['scale']) for h in res(o)['here']] == [('bakery', 'building'), ('emberford', 'settlement'), ('ember_vale', 'region')],
        routes([('cellar', 0), ('square', 1), ('smithy', 2), ('keep', 3), ('whisper_woods', 5)]), around([('well', 'emberford')]),
        lambda o: res(o)['routes'][3]['from'] == 'emberford' and res(o)['routes'][3]['steps'] == [dict(to='keep', entrance=None, direction='east')],
        lambda o: res(o)['routes'][2]['steps'] == [dict(to='square', entrance='bakery_door', direction=None), dict(to='smithy', entrance='smithy_door', direction=None)]), False),
    ('from_woods', run(q(**{'from': 'whisper_woods'})), both(
        routes([('emberford', 5), ('keep', 8)]), around([('clearing', 'whisper_woods'), ('wolf_cave', 'whisper_woods')]),
        lambda o: [s['direction'] for s in res(o)['routes'][1]['steps']] == ['south', 'east']), False),
    ('from_clearing', run(q(**{'from': 'clearing'})), both(routes([('wolf_cave', 2), ('emberford', 5), ('keep', 8)]), around([])), False),
    ('closed_door', run(q(**{'from': 'bakery', 'closed': ['bakery_door']})),
     both(routes([('cellar', 0), ('keep', 3), ('whisper_woods', 5)]), around([('smithy', 'emberford'), ('square', 'emberford'), ('well', 'emberford')])), False),
    ('max_distance', run(q(**{'from': 'bakery', 'max_distance': 2})),
     both(routes([('cellar', 0), ('square', 1), ('smithy', 2)]), around([('well', 'emberford'), ('whisper_woods', 'ember_vale'), ('keep', 'ember_vale')])), False),
    ('max_distance_zero', run(q(**{'from': 'bakery', 'max_distance': 0})), routes([('cellar', 0)]), False),
    ('one_way_forward', run(q(**{'from': 'great_hall'})), lambda o: ('vault', 1) in [(r['place'], r['distance']) for r in res(o)['routes']], False),
    ('one_way_back_blocked', run(q(**{'from': 'vault'})), both(routes([('emberford', 3), ('whisper_woods', 8)]), around([('great_hall', 'keep')])), False),
    ('limit_and_totals', run(q(**{'from': 'bakery', 'limit': 1})), both(routes([('cellar', 0)]), lambda o: res(o)['total_routes'] == 5 and res(o)['total_around'] == 1), False),
    ('custom_kind_scale', run(q(**{'from': 'dock'})), lambda o: res(o)['here'] == [dict(place='dock', label='the sky dock', kind='sky_dock', scale='site')] and res(o)['routes'] == [], False),
    ('no_kind', run(q(**{'from': 'x'}), codex=dict(version=1, places=[dict(id='x', label='X')])), lambda o: res(o)['here'] == [dict(place='x', label='X', kind=None, scale=None)], False),
    ('tie_codex_order', run(q(**{'from': 'a'}), codex=dict(version=1, places=[place('a', 'A'), place('c', 'C', links=[dict(place='a', distance=1)]), place('b', 'B', links=[dict(place='a', distance=1)])])),
     routes([('c', 1), ('b', 1)]), False),
    ('several_queries', run(q(**{'from': 'bakery'}), q(**{'from': 'nowhere'}), q(**{'from': 'keep'})), both(failed(1), lambda o: res(o, 2)['here'][0]['place'] == 'keep'), False),
    ('bad_query_limit', run(q(**{'from': 'bakery', 'limit': 0})), failed(), False),
    ('bad_query_closed_dup', run(q(**{'from': 'bakery', 'closed': ['a', 'a']})), failed(), False),
    ('bad_query_key', run(q(**{'from': 'bakery', 'near': True})), failed(), False),
    ('bad_query_distance', run(q(**{'from': 'bakery', 'max_distance': 1000000001})), failed(), False),
    ('error_within_cycle', run(codex=with_place('ember_vale', within='keep')), None, True),
    ('error_within_self', run(codex=with_place('keep', within='keep')), None, True),
    ('error_within_unknown', run(codex=with_place('keep', within='moon')), None, True),
    ('error_too_deep', run(codex=dict(version=1, places=[place('p0', 'P0')] + [place(f'p{i}', f'P{i}', within=f'p{i - 1}') for i in range(1, 10)])), None, True),
    ('deep_eight_ok', run(q(**{'from': 'p8'}), codex=dict(version=1, places=[place('p0', 'P0')] + [place(f'p{i}', f'P{i}', within=f'p{i - 1}') for i in range(1, 9)])),
     lambda o: len(res(o)['here']) == 9, False),
    ('error_unknown_kind', run(codex=with_place('keep', kind='spaceship')), None, True),
    ('error_custom_kind_builtin', run(codex=dict(VALE, place_kinds=[dict(id='cave', label='cave', scale='site', indoor=True)])), None, True),
    ('error_custom_scale', run(codex=dict(VALE, place_kinds=[dict(id='sky_dock', label='sky dock', scale='planet', indoor=False)])), None, True),
    ('error_link_both_ends', run(codex=with_place('whisper_woods', links=[dict(place='emberford', distance=5)])), None, True),
    ('error_link_self', run(codex=with_place('keep', links=[dict(place='keep', distance=1)])), None, True),
    ('error_link_direction', run(codex=with_place('keep', links=[dict(place='dock', distance=1, direction='left')])), None, True),
    ('error_tags_dup', run(codex=with_place('keep', tags=['a', 'a'])), None, True),
    ('error_entrance_distance', run(codex=dict(VALE, entrances=[dict(VALE['entrances'][0], distance=-1)])), None, True),
]


def random_codex(rng):
    n = rng.randint(1, 12); ids = [f'p{i}' for i in range(n)]; places = []
    for i, x in enumerate(ids):
        p = place(x, f'Place {i}', rng.choice([None, 'cave', 'house', 'forest', 'room', 'village', 'sky_dock']))
        if i and rng.random() < 0.6: p['within'] = rng.choice(ids[:i] + ([x] if rng.random() < 0.03 else []))
        links = []
        for _ in range(rng.randint(0, 2)):
            o = rng.choice(ids)
            if o != x and all(l['place'] != o for l in links): links.append(dict(place=o, distance=rng.choice([0, 1, 2, 5]), **({'direction': rng.choice(['north', 'up', 'west'])} if rng.random() < 0.5 else {})))
        if links: p['links'] = links
        places.append(p)
    pairs = []  # drop reverse duplicates most of the time so most codexes are valid
    for p in places:
        keep = []
        for l in p.get('links', []):
            pair = sorted([p['id'], l['place']])
            if pair not in pairs or rng.random() < 0.05: keep.append(l); pairs.append(pair)
        if 'links' in p: p['links'] = keep
    ents = []
    for j in range(rng.randint(0, 6)):
        a, b = rng.choice(ids), rng.choice(ids)
        if a != b: ents.append(dict(door(f'd{j}', f'Door {j}', a, b), **({'distance': rng.choice([0, 1, 3])} if rng.random() < 0.5 else {}), **({'passage': 'one_way'} if rng.random() < 0.3 else {})))
    queries = [dict({'from': rng.choice(ids + ['zz'])}, **({'closed': rng.sample([e['id'] for e in ents], rng.randint(0, len(ents)))} if ents and rng.random() < 0.4 else {}),
                    **({'max_distance': rng.choice([0, 1, 3])} if rng.random() < 0.4 else {}), **({'limit': rng.choice([1, 2, 64])} if rng.random() < 0.3 else {}))
               for _ in range(rng.randint(1, 4))]
    return run(*queries, codex=dict(version=1, place_kinds=[dict(id='sky_dock', label='sky dock', scale='site', indoor=False)], places=places, entrances=ents))


def call(req): return subprocess.run([str(ROOT / 'target/debug/gamectx'), 'local', '-'], input=enc(req), capture_output=True, text=True)


def main():
    p = argparse.ArgumentParser(); p.add_argument('--cap', type=float, required=True); p.add_argument('--prototype-only', action='store_true')
    p.add_argument('--write-conformance', action='store_true'); p.add_argument('--random', type=int, default=0); p.add_argument('--seed', type=int, default=1)
    a = p.parse_args(); assert a.cap == 0, 'place cases are offline only'
    if a.random:
        rng = random.Random(a.seed); mismatches = valid = 0
        for _ in range(a.random):
            args = random_case = random_codex(rng); req = dict(kind='places', args=args)
            try: got = pr.workflow(copy.deepcopy(args)); valid += 1
            except (AssertionError, KeyError, TypeError, ValueError): got = None
            x = call(req)
            if (got is None) != (x.returncode != 0) or got is not None and x.stdout.strip() != enc(got): mismatches += 1; print('MISMATCH', enc(random_case)[:800])
        print(enc(dict(workflows=a.random, valid_codexes=valid, mismatches=mismatches, actual_model_calls=0, api_spend_usd=0))); return
    for n, args, check, error in CASES:
        try: got = pr.workflow(copy.deepcopy(args)); assert not error, n
        except (AssertionError, KeyError, TypeError, ValueError):
            assert error, n; got = None
        if check is not None: assert check(got), (n, enc(got)[:1500])
        req = dict(kind='places', args=args)
        if not a.prototype_only:
            x = call(req); assert (x.returncode != 0) == error, (n, x.stderr)
            if not error: assert x.stdout.strip() == enc(got), (n, x.stdout[:1500], enc(got)[:1500])
        if a.write_conformance:
            dd = ROOT / 'conformance/places' / n; new = not dd.exists(); dd.mkdir(parents=True, exist_ok=True)
            for k, v in [('case', dict(op='local', **(dict(error=True) if error else {}))), ('input', req)] + ([] if error else [('expected', got)]):
                f = dd / (k + '.json')
                if new: f.write_text(enc(v) + '\n')
                else: assert f.read_text() == enc(v) + '\n', f
    print(enc(dict(workflows=len(CASES), malformed=sum(e for *_, e in CASES), actual_model_calls=0, api_spend_usd=0)))


if __name__ == '__main__':
    main()
