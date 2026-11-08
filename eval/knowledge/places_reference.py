"""Independent original nearby-places reference over a codex: where am I (the chain of
places I am inside), which places can I reach (shortest routes through entrances and
links, from my place and every place containing it) and what else is around (places in
the same containers without an authored route). Python standard library only.
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy
import roleplay_definitions_reference as defs
import roleplay_engine_reference as eng
from roleplay_definitions_reference import ident, keys, lst, uint

OPPOSITE = dict(north='south', northeast='southwest', east='west', southeast='northwest', south='north', southwest='northeast', west='east',
                northwest='southeast', up='down', down='up')


def kinds(codex):
    out = {k['id']: k for k in defs.PLACE_KINDS['kinds']}
    out.update({k['id']: k for k in codex.get('place_kinds', [])}); return out


def nearby(codex, q):
    keys(q, ['from'], ['closed', 'max_distance', 'limit'])
    places = codex.get('places', []); idx = {p['id']: i for i, p in enumerate(places)}; by = {p['id']: p for p in places}
    ident(q['from']); assert q['from'] in idx
    closed = lst(q.get('closed', []), 0, 256)
    for c in closed: ident(c)
    assert len(set(closed)) == len(closed)
    maxd = uint(q.get('max_distance', 1000000000), 0, 1000000000); limit = uint(q.get('limit', 16), 1, 64)
    kind_of = kinds(codex)
    chain = [q['from']]
    while by[chain[-1]].get('within') is not None: chain.append(by[chain[-1]]['within'])
    adj = {p['id']: [] for p in places}
    for e in codex.get('entrances', []):
        if e['id'] in closed or len(e['sides']) < 2: continue
        a, b = e['sides'][0]['place'], e['sides'][1]['place']; d = e.get('distance', 1)
        adj[a].append((b, d, dict(to=b, entrance=e['id'], direction=None)))
        if e.get('passage', 'both') != 'one_way': adj[b].append((a, d, dict(to=a, entrance=e['id'], direction=None)))
    for p in places:
        for l in p.get('links', []):
            d, way = l['distance'], l.get('direction')
            adj[p['id']].append((l['place'], d, dict(to=l['place'], entrance=None, direction=way)))
            adj[l['place']].append((p['id'], d, dict(to=p['id'], entrance=None, direction=None if way is None else OPPOSITE[way])))
    best = {c: (0, c, []) for c in chain}; done = set()
    while len(done) < len(best):
        u = min((x for x in best if x not in done), key=lambda x: (best[x][0], idx[x])); done.add(u)
        du, src, steps = best[u]
        for v, w, step in adj[u]:
            nd = du + w
            if nd <= maxd and (v not in best or nd < best[v][0]): best[v] = (nd, src, steps + [step])
    routes = sorted((x for x in best if x not in chain), key=lambda x: (best[x][0], idx[x]))
    around = [p['id'] for a in chain for p in places if p.get('within') == a and p['id'] not in chain and p['id'] not in best]
    label = lambda x: by[x]['label']; kind = lambda x: by[x].get('kind')
    return dict(
        here=[dict(place=x, label=label(x), kind=kind(x), scale=None if kind(x) is None else kind_of[kind(x)]['scale']) for x in chain],
        routes=[dict(place=x, label=label(x), kind=kind(x), distance=best[x][0], **{'from': best[x][1]}, steps=copy.deepcopy(best[x][2])) for x in routes[:limit]],
        total_routes=len(routes),
        around=[dict(place=x, label=label(x), kind=kind(x), within=by[x]['within']) for x in around[:limit]],
        total_around=len(around))


def workflow(a):
    keys(a, ['codex', 'queries']); codex = a['codex']; defs.codex(codex, eng.VARS)
    assert isinstance(a['queries'], list) and len(a['queries']) <= 64; results = []
    for q in a['queries']:
        try: results.append(dict(ok=True, result=nearby(codex, q)))
        except (AssertionError, KeyError, TypeError, ValueError): results.append(dict(ok=False))
    return dict(results=results)
