"""Independent reference validators for world and character definitions (0.4.0a, stage 2.1b).

Codex (shared world content): places with spots, ambience and norms; entrances with a
description and travel decision per side; factions; events; quests.
Character sheet (one per character): persona, speech style, services, goals, needs,
relationships, faction memberships, secrets and opinions.
Content settings: a rating and topics to steer away from.

Definitions are fixed authored content. Live state (entrance status, need levels, quest
stages, host flags) arrives with each turn. Nothing here executes or calls a model.
"""
import json, pathlib, re

U32 = 4294967295
LIMIT = 1000000000
STATUSES = ['open', 'closed', 'locked', 'blocked', 'hidden']
STANCES = ['allied', 'friendly', 'neutral', 'wary', 'hostile']
CARRIES = ['none', 'when_open', 'always']
RATINGS = ['all', 'teen', 'mature']
DIRECTIONS = ['north', 'northeast', 'east', 'southeast', 'south', 'southwest', 'west', 'northwest', 'up', 'down']
PLACE_KINDS = json.loads((pathlib.Path(__file__).resolve().parents[2] / 'src/knowledge/place_kinds.json').read_text())


def boolean(b): assert type(b) is bool; return b
def coords_val(v):
    assert isinstance(v, list) and len(v) in (2, 3)
    for x in v: assert (type(x) is int or type(x) is float) and -1000000.0 <= x <= 1000000.0
    return [float(x) for x in v]
def dimensions_val(v):
    if v is None: return None
    if isinstance(v, list):
        assert len(v) == 3
        for x in v: assert (type(x) is int or type(x) is float) and 0.0 < x <= 100000.0
        return [float(x) for x in v]
    if isinstance(v, dict):
        assert set(v.keys()) == {'width', 'length', 'height'}
        for k in ['width', 'length', 'height']: assert (type(v[k]) is int or type(v[k]) is float) and 0.0 < v[k] <= 100000.0
        return [float(v['width']), float(v['length']), float(v['height'])]
    assert False
def validate_matter(v):
    if 'mass' in v and v['mass'] is not None:
        assert (type(v['mass']) is int or type(v['mass']) is float) and 0.0 < v['mass'] <= 1000000.0
    if 'state' in v and v['state'] is not None: assert v['state'] in ['solid', 'liquid', 'gas']
    for k in ['material', 'color', 'texture']:
        if k in v and v[k] is not None: text(v[k], 48)

def control(s): return any(ord(c) < 32 or 127 <= ord(c) <= 159 for c in s)
def text(s, chars, nbytes=None):
    assert isinstance(s, str) and 1 <= len(s) <= chars and not control(s)
    if nbytes is not None: assert len(s.encode()) <= nbytes
    return s
def ident(s): assert isinstance(s, str) and re.fullmatch('[A-Za-z0-9_:-]{1,64}', s); return s
def uint(n, lo=0, hi=U32): assert type(n) is int and lo <= n <= hi; return n
def keys(v, required, optional=()):
    assert isinstance(v, dict) and set(required) <= set(v) <= set(required) | set(optional)
def norm(s): return ' '.join(''.join(c if c.isalnum() else ' ' for c in s.lower()).split())
def lst(v, lo, hi): assert isinstance(v, list) and lo <= len(v) <= hi; return v
def decision(d):
    if d is not None:
        keys(d, ['action', 'target']); ident(d['action'])
        if d['target'] is not None: ident(d['target'])
    return d


def condition_key(k, names):
    assert k in names or re.fullmatch(r'state\.[A-Za-z0-9_:-]{1,64}', k), k
def conditions(when, names):
    assert isinstance(when, dict) and len(when) <= 8
    for k, c in when.items():
        condition_key(k, names)
        if isinstance(c, dict):
            assert c and set(c) <= {'min', 'max'}
            for x in c.values(): assert type(x) is int and -LIMIT <= x <= LIMIT
        else:
            lst(c, 1, 16)
            for x in c: assert type(x) is bool or type(x) is int and -LIMIT <= x <= LIMIT or isinstance(x, str) and len(x) <= 64
    return when


def names(item, seen, n=8):
    """Label and aliases: each ≤48 chars, normalized non-empty and unique within `seen`."""
    out = []
    for x in [item['label']] + lst(item.get('aliases', []), 0, n):
        text(x, 48); q = norm(x); assert q and q not in seen; seen.append(q); out.append(q)
    return out
def phrases(v, seen, hi=16):
    out = []
    for p in lst(v, 1, hi):
        text(p, 128, 128); q = norm(p); assert q and q not in seen; seen.append(q); out.append(q)
    return out
def unique_ids(items):
    ids = []
    for x in items: ident(x['id']); assert x['id'] not in ids; ids.append(x['id'])


def place_graph(v, places):
    """Place kinds, nesting (`within`), tags and links between places."""
    builtin = [k['id'] for k in PLACE_KINDS['kinds']]; custom = lst(v.get('place_kinds', []), 0, 64)
    for k in custom:
        keys(k, ['id', 'label', 'scale', 'indoor']); ident(k['id']); assert k['id'] not in builtin
        text(k['label'], 48); assert k['scale'] in PLACE_KINDS['scales']; boolean(k['indoor'])
    unique_ids(custom); kinds = builtin + [k['id'] for k in custom]; ids = [p['id'] for p in places]; pairs = []
    for p in places:
        if 'kind' in p: assert p['kind'] in kinds
        if 'within' in p: assert p['within'] in ids and p['within'] != p['id']
        tags = lst(p.get('tags', []), 0, 8)
        for t in tags: ident(t)
        assert len(set(tags)) == len(tags)
        for l in lst(p.get('links', []), 0, 16):
            keys(l, ['place', 'distance'], ['direction']); assert l['place'] in ids and l['place'] != p['id']
            uint(l['distance'], 0, 1000000000)
            if 'direction' in l: assert l['direction'] in DIRECTIONS
            pair = sorted([p['id'], l['place']]); assert pair not in pairs; pairs.append(pair)
    parent = {p['id']: p.get('within') for p in places}
    for p in places:
        x, depth = p['id'], 0
        while parent[x] is not None: x = parent[x]; depth += 1; assert depth <= 8


def codex(v, vars_):
    keys(v, ['version'], ['places', 'entrances', 'factions', 'events', 'quests', 'place_kinds']); uint(v['version'], 1, 1)
    places = lst(v.get('places', []), 0, 128)
    for p in places:
        keys(p, ['id', 'label'], ['aliases', 'spots', 'ambience', 'norms', 'exterior', 'kind', 'within', 'tags', 'links'])
        if 'exterior' in p and p['exterior'] is not None: boolean(p['exterior'])
    unique_ids(places); seen = []
    for p in places:
        names(p, seen); spots = lst(p.get('spots', []), 0, 16)
        for s in spots:
            keys(s, ['id', 'label', 'description'], ['aliases', 'dimensions', 'coords', 'mass', 'state', 'material', 'color', 'texture', 'tags', 'attributes'])
            text(s['description'], 240)
            if 'dimensions' in s and s['dimensions'] is not None: dimensions_val(s['dimensions'])
            if 'coords' in s and s['coords'] is not None: coords_val(s['coords'])
            validate_matter(s)
            if 'tags' in s and s['tags'] is not None:
                for tg in lst(s['tags'], 0, 32): text(tg, 48)
            if 'attributes' in s and s['attributes'] is not None:
                assert isinstance(s['attributes'], dict) and len(s['attributes']) <= 32
                for k, val in s['attributes'].items():
                    ident(k)
                    if isinstance(val, str): text(val, 120)
                    else: assert type(val) in (int, float, bool)
        unique_ids(spots); spot_names = []
        for s in spots: names(s, spot_names)
        for a in lst(p.get('ambience', []), 0, 8):
            keys(a, [], ['when', 'smell', 'light', 'sound', 'temperature', 'floor', 'ceiling']); conditions(a.get('when', {}), vars_)
            assert any(k in a for k in ['smell', 'light', 'sound', 'temperature', 'floor', 'ceiling'])
            for k in ['smell', 'light', 'sound', 'temperature', 'floor', 'ceiling']:
                if k in a: text(a[k], 160)
        norms = lst(p.get('norms', []), 0, 16)
        for n in norms:
            keys(n, ['id', 'actions', 'reaction']); text(n['reaction'], 240)
            for d in lst(n['actions'], 1, 8): assert d is not None; decision(d)
        unique_ids(norms)
    place_ids = [p['id'] for p in places]
    entrances = lst(v.get('entrances', []), 0, 256)
    for e in entrances: keys(e, ['id', 'label', 'sides'], ['kind', 'aliases', 'passage', 'carries', 'distance'])
    unique_ids(entrances); seen = []
    for e in entrances:
        names(e, seen)
        if 'kind' in e: ident(e['kind'])
        assert e.get('passage', 'both') in ['both', 'one_way']
        carries = e.get('carries', {}); keys(carries, [], ['sound', 'sight'])
        for k in carries: assert carries[k] in CARRIES
        sides = lst(e['sides'], 1, 2)
        for s in sides:
            keys(s, ['place', 'description'], ['travel']); assert s['place'] in place_ids; text(s['description'], 240); decision(s.get('travel'))
        assert len({s['place'] for s in sides}) == len(sides)
        if 'distance' in e: uint(e['distance'], 0, 1000000000)
    place_graph(v, places)
    factions = lst(v.get('factions', []), 0, 64)
    for f in factions: keys(f, ['id', 'label', 'description'], ['aliases', 'relations']); text(f['description'], 240)
    unique_ids(factions); seen = []; fids = [f['id'] for f in factions]
    for f in factions:
        names(f, seen); others = []
        for r in lst(f.get('relations', []), 0, 32):
            keys(r, ['faction', 'stance']); assert r['faction'] in fids and r['faction'] != f['id'] and r['faction'] not in others and r['stance'] in STANCES; others.append(r['faction'])
    events = lst(v.get('events', []), 0, 128)
    for e in events:
        keys(e, ['id', 'label', 'description', 'start', 'end'], ['aliases', 'place']); text(e['description'], 240); uint(e['start']); uint(e['end']); assert e['start'] <= e['end']
        if 'place' in e: assert e['place'] in place_ids
    unique_ids(events); seen = []
    for e in events: names(e, seen)
    quests = lst(v.get('quests', []), 0, 128)
    for q in quests:
        keys(q, ['id', 'label', 'stages'], ['aliases', 'givers'])
        for g in lst(q.get('givers', []), 0, 16): ident(g)
        stages = lst(q['stages'], 1, 32)
        for s in stages: keys(s, ['id', 'description']); text(s['description'], 240)
        unique_ids(stages)
    unique_ids(quests); seen = []
    for q in quests: names(q, seen)
    return v


HEALTH = ['healthy', 'tired', 'hurt', 'ill', 'wounded', 'critical']
FAMILY_STATUS = ['alive', 'deceased', 'estranged', 'unknown']
def day(v):
    keys(v, ['month', 'day']); m = uint(v['month'], 1, 12); uint(v['day'], 1, [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][m - 1]); return v
def health(v):
    keys(v, ['status'], ['conditions']); assert v['status'] in HEALTH
    cs = lst(v.get('conditions', []), 0, 16)
    for x in cs: keys(x, ['id', 'label', 'severity', 'visible']); text(x['label'], 48); uint(x['severity'], 0, 100); assert type(x['visible']) is bool
    unique_ids(cs); return v
def bio(v, vars_):
    keys(v, [], ['species', 'age', 'birthday', 'profession', 'anatomy', 'health', 'skills', 'knowledge', 'family'])
    if 'species' in v: text(v['species'], 48)
    if 'profession' in v: text(v['profession'], 80)
    if 'age' in v: uint(v['age'], 0, 100000)
    if 'birthday' in v: day(v['birthday'])
    if 'anatomy' in v:
        a = v['anatomy']; keys(a, [], ['body', 'parts', 'notes'])
        if 'body' in a: text(a['body'], 48)
        if 'notes' in a: text(a['notes'], 240)
        for p in lst(a.get('parts', []), 0, 16): text(p, 48)
    if 'health' in v: health(v['health'])
    seen = []; skills = lst(v.get('skills', []), 0, 32)
    for x in skills:
        keys(x, ['id', 'label', 'level'], ['phrases']); text(x['label'], 48); uint(x['level'], 0, 100)
        if 'phrases' in x: phrases(x['phrases'], seen, 8)
    unique_ids(skills)
    seen = []; know = lst(v.get('knowledge', []), 0, 32)
    for x in know:
        keys(x, ['id', 'topic', 'phrases', 'level', 'summary'], ['when']); text(x['topic'], 48); phrases(x['phrases'], seen, 8); uint(x['level'], 0, 100); text(x['summary'], 240)
        conditions(x.get('when', {}), vars_)
    unique_ids(know)
    for x in lst(v.get('family', []), 0, 16):
        keys(x, ['name', 'relation', 'status'], ['with']); text(x['name'], 32); text(x['relation'], 32); assert x['status'] in FAMILY_STATUS
        if 'with' in x: ident(x['with'])


def sheet(v, cdx, vars_):
    keys(v, ['version'], ['persona', 'speech', 'services', 'goals', 'needs', 'relationships', 'factions', 'secrets', 'opinions',
                          'features', 'clothing', 'state', 'sensory', 'habits', 'tags', 'attributes', 'bio']); uint(v['version'], 1, 1)
    p = v.get('persona', {}); keys(p, [], ['pronouns', 'role', 'summary', 'appearance', 'values', 'fears', 'desires'])
    for k, n in [('pronouns', 32), ('role', 80), ('summary', 240), ('appearance', 480)]:
        if k in p: text(p[k], n)
    for k in ['values', 'fears', 'desires']:
        for x in lst(p.get(k, []), 0, 8): text(x, 80)
    s = v.get('speech', {}); keys(s, [], ['formality', 'verbosity', 'quirks', 'catchphrases', 'avoid_words'])
    assert s.get('formality', 'neutral') in ['casual', 'neutral', 'formal'] and s.get('verbosity', 'normal') in ['terse', 'normal', 'chatty']
    for k, n in [('quirks', 8), ('catchphrases', 8)]:
        for x in lst(s.get(k, []), 0, n): text(x, 120)
    for x in lst(s.get('avoid_words', []), 0, 32): text(x, 40); assert norm(x)
    services = lst(v.get('services', []), 0, 32); seen = []
    for x in services:
        keys(x, ['id', 'label', 'description'], ['phrases', 'decision', 'when']); text(x['label'], 48); text(x['description'], 240)
        if 'phrases' in x: phrases(x['phrases'], seen, 8)
        decision(x.get('decision')); conditions(x.get('when', {}), vars_)
    unique_ids(services)
    goals = lst(v.get('goals', []), 0, 16)
    for g in goals: keys(g, ['id', 'description', 'priority'], ['when']); text(g['description'], 240); uint(g['priority'], 0, 100); conditions(g.get('when', {}), vars_)
    unique_ids(goals)
    needs = lst(v.get('needs', []), 0, 16)
    for n in needs: keys(n, ['id', 'label', 'threshold']); text(n['label'], 48); uint(n['threshold'], 0, 100)
    unique_ids(needs)
    rels = lst(v.get('relationships', []), 0, 64); seen = []; withs = []
    for r in rels:
        keys(r, ['with', 'name', 'kind'], ['aliases', 'description']); ident(r['with']); assert r['with'] not in withs; withs.append(r['with'])
        text(r['kind'], 32); names(dict(label=r['name'], aliases=r.get('aliases', [])), seen)
        if 'description' in r: text(r['description'], 240)
    fids = [f['id'] for f in cdx.get('factions', [])]; member = []
    for m in lst(v.get('factions', []), 0, 16):
        keys(m, ['faction'], ['role']); assert m['faction'] in fids and m['faction'] not in member; member.append(m['faction'])
        if 'role' in m: text(m['role'], 48)
    secrets = lst(v.get('secrets', []), 0, 32); seen = []
    for x in secrets:
        keys(x, ['id', 'phrases', 'reveal', 'when'], ['cover']); phrases(x['phrases'], seen); text(x['reveal'], 240); conditions(x['when'], vars_)
        if x.get('cover') is not None: text(x['cover'], 240)
    unique_ids(secrets)
    opinions = lst(v.get('opinions', []), 0, 64); seen = []
    for x in opinions:
        keys(x, ['id', 'phrases', 'variants'], ['when']); phrases(x['phrases'], seen)
        for t in lst(x['variants'], 1, 8): text(t, 240)
        conditions(x.get('when', {}), vars_)
    unique_ids(opinions)
    if 'features' in v and v['features'] is not None:
        f = v['features']; keys(f, [], ['eyes', 'hair', 'skin', 'scars', 'height', 'build'])
        for k in ['eyes', 'hair', 'skin', 'build']:
            if k in f and f[k] is not None: text(f[k], 48)
        for sc in lst(f.get('scars', []), 0, 8): text(sc, 48)
        if 'height' in f and f['height'] is not None:
            assert (type(f['height']) is int or type(f['height']) is float) and 0.0 < f['height'] <= 100.0
    if 'clothing' in v and v['clothing'] is not None:
        for item in (v['clothing'] if isinstance(v['clothing'], list) else [v['clothing']]):
            text(item, 48)
    if 'state' in v and v['state'] is not None:
        st = v['state']; keys(st, [], ['posture', 'held', 'gaze', 'activity', 'awareness', 'exposure', 'expression'])
        for k in ['posture', 'activity', 'awareness', 'exposure', 'expression']:
            if k in st and st[k] is not None: text(st[k], 80)
        if 'gaze' in st and st['gaze'] is not None: ident(st['gaze'])
        if 'held' in st and st['held'] is not None:
            if isinstance(st['held'], dict):
                assert len(st['held']) <= 8
                for k, val in st['held'].items(): text(k, 32); text(val, 64)
            elif isinstance(st['held'], list):
                assert len(st['held']) <= 8
                for x in st['held']: text(x, 64)
            else: assert False
    if 'sensory' in v and v['sensory'] is not None:
        sn = v['sensory']; keys(sn, [], ['scent', 'voice_tone'])
        for k in ['scent', 'voice_tone']:
            if k in sn and sn[k] is not None: text(sn[k], 48)
    if v.get('bio') is not None: bio(v['bio'], vars_)
    if 'habits' in v and v['habits'] is not None:
        for h in lst(v['habits'], 0, 16): text(h, 80)
    if 'tags' in v and v['tags'] is not None:
        for tg in lst(v['tags'], 0, 32): text(tg, 48)
    if 'attributes' in v and v['attributes'] is not None:
        att = v['attributes']; assert isinstance(att, dict) and len(att) <= 32
        for k, val in att.items():
            ident(k)
            if isinstance(val, str): text(val, 120)
            else: assert type(val) in (int, float, bool)
    return v


def content(v):
    keys(v, [], ['enabled', 'rating', 'avoid'])
    assert type(v.get('enabled', True)) is bool and v.get('rating', 'teen') in RATINGS
    avoid = lst(v.get('avoid', []), 0, 32); seen = []
    for a in avoid: keys(a, ['id', 'phrases']); phrases(a['phrases'], seen)
    unique_ids(avoid)
    return dict(enabled=v.get('enabled', True), rating=v.get('rating', 'teen'), avoid=avoid)
