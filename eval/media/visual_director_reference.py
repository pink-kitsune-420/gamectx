"""Independent original visual director: tiered art bibles, format profiles, host fact
selectors and an approval/repair/fallback workflow over the visual registry reference.
Python standard library only; no image model, URI fetch or pixel check."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy, re
import visual_reference as r
from archetype_reference import fields, integer, name, scope

FORMATS = ['sprite', 'portrait', 'scene']
KEY = re.compile('visual:[0-9a-f]{64}')
SOURCE_TEXT = 'Host-selected visual fact.'


def value(v):
    if type(v) is bool: return v
    if type(v) is int: assert -10**9 <= v <= 10**9; return v
    assert isinstance(v, str) and len(v) <= 64; return scope(v)


def when(w):
    assert isinstance(w, dict) and len(w) <= 8
    for k, vs in w.items():
        scope(k); assert isinstance(vs, list) and 1 <= len(vs) <= 16
        for x in vs: value(x)
    return w


def holds(w, vars):
    return all(k in vars and any(type(x) is type(vars[k]) and x == vars[k] for x in vs) for k, vs in w.items())


def profile(p):
    fields(p, 'id format framing camera_angle size'); name(p['id']); assert p['format'] in FORMATS
    for k in ['framing', 'camera_angle']:
        if p[k] is not None: r.token(p[k])
    if p['size'] is not None: fields(p['size'], 'width height aspect')
    return p


def applied(bible, p):
    b = copy.deepcopy(bible); b['format'] = p['format']
    for k in ['framing', 'camera_angle']:
        if p[k] is not None: b[k] = p[k]
    if p['size'] is not None: b.update(copy.deepcopy(p['size']))
    r.bible(b); return b


def config(c):
    fields(c, 'version tiers profiles selectors'); assert c['version'] == 1
    assert isinstance(c['tiers'], list) and 1 <= len(c['tiers']) <= 4
    assert isinstance(c['profiles'], list) and 1 <= len(c['profiles']) <= 8
    assert isinstance(c['selectors'], list) and len(c['selectors']) <= 32
    ids = []
    for t in c['tiers']:
        fields(t, 'id bible'); name(t['id']); assert t['id'] not in ids; ids.append(t['id'])
    ids = []
    for p in c['profiles']:
        profile(p); assert p['id'] not in ids; ids.append(p['id'])
        for t in c['tiers']: applied(t['bible'], p)
    ids = []
    for s in c['selectors']:
        fields(s, 'fact when'); scope(s['fact']); assert s['fact'] not in ids; ids.append(s['fact']); when(s['when'])
    return copy.deepcopy(c)


def key(k): assert isinstance(k, str) and KEY.fullmatch(k); return k


def patch_shape(p): fields(p, 'replace_tokens add_tokens negative_tokens forbid_props seed_delta asset'); return p


def approval(a): fields(a, 'key asset_uri digest'); key(a['key']); r.asset(a['asset_uri'], a['digest']); return a


def repair(x):
    fields(x, 'base patches'); key(x['base']); assert isinstance(x['patches'], list) and 1 <= len(x['patches']) <= 8
    for p in x['patches']: patch_shape(p)
    return x


class Director:
    def __init__(self, cfg, registry, checkpoint=None):
        self.cfg = config(cfg); self.registry = r.Registry(registry); self.approvals = {}; self.repairs = {}; self.applied = []
        if checkpoint is not None:
            fields(checkpoint, 'version approvals repairs'); assert checkpoint['version'] == 1
            assert isinstance(checkpoint['approvals'], list) and len(checkpoint['approvals']) <= 128
            assert isinstance(checkpoint['repairs'], list) and len(checkpoint['repairs']) <= 64
            for a in checkpoint['approvals']:
                approval(a); assert a['key'] not in self.approvals; self.approvals[a['key']] = copy.deepcopy(a)
            for x in checkpoint['repairs']:
                repair(x); assert x['base'] not in self.repairs; self.repairs[x['base']] = copy.deepcopy(x)

    def checkpoint(self):
        return dict(version=1, approvals=[self.approvals[k] for k in sorted(self.approvals)], repairs=[self.repairs[k] for k in sorted(self.repairs)])

    def direct(self, q, missing):
        fields(q, 'tier profile entity scope observer listener audience scene vars life_revision activity_revision generate')
        assert type(q['generate']) is bool and q['life_revision'] is None and q['activity_revision'] is None
        s = q['scene']; fields(s, 'id revision epoch at participants')
        assert isinstance(q['vars'], dict) and len(q['vars']) <= 64
        for k, v in q['vars'].items(): scope(k); value(v)
        tiers = self.cfg['tiers']; requested = tiers[-1]['id'] if q['tier'] is None else name(q['tier'])
        top = next(i for i, t in enumerate(tiers) if t['id'] == requested)
        p = next(x for x in self.cfg['profiles'] if x['id'] == name(q['profile']))
        kind = 'scene' if p['format'] == 'scene' else 'character'
        chosen = None
        for i in range(top, -1, -1):
            b = applied(tiers[i]['bible'], p)
            if any(a['entity'] == q['entity'] and a['kind'] == kind for a in b['anchors']): chosen = (tiers[i]['id'], b); break
        assert chosen is not None
        emitted = [x['fact'] for x in self.cfg['selectors'] if holds(x['when'], q['vars'])]
        facts = [dict(id=f, revision=s['revision'], observed_at=s['at'], source=dict(id='director', speaker=q['observer'], text=SOURCE_TEXT),
                      kind='perceived', audience=copy.deepcopy(s['participants']), text=SOURCE_TEXT) for f in emitted]
        frame = dict(scope=q['scope'], observer=q['observer'], entity=q['entity'], listener=q['listener'], scene=dict(s, facts=facts, affordances=[]),
                     audience=q['audience'], life_revision=None, activity_revision=None)
        spec = r.compile(dict(bible=chosen[1], frame=frame, life=None, activity=None))
        assert isinstance(missing, list) and len(missing) <= 128
        for u in missing: r.asset(u, '0' * 64)
        # Work on copies so a failure leaves the director unchanged.
        reg = copy.deepcopy(self.registry); approvals = copy.deepcopy(self.approvals); applied_ = list(self.applied)
        sel = reg.request(spec, enqueue=False); current = sel['key']; repaired = False
        x = self.repairs.get(spec['key'])
        if x is not None and spec['key'] not in applied_ and current in reg.rows:
            for patch in x['patches']: current = reg.repair(current, patch)['key']
            applied_.append(spec['key']); repaired = True
        state = None
        a = approvals.get(current)
        if a is not None and current in reg.rows and reg.rows[current]['state'] not in ['approved', 'pinned']:
            if a['asset_uri'] in missing: del approvals[current]; state = 'unverified'
            else:
                try: reg.approve(current, a['asset_uri'], a['digest']); state = 'verified'
                except (AssertionError, KeyError): del approvals[current]; state = 'stale'
        sel = reg.request(spec, enqueue=q['generate'])
        self.registry, self.approvals, self.applied = reg, approvals, applied_
        status = 'exact' if sel['exact'] else 'fallback' if sel['asset'] is not None else 'placeholder'
        return dict(key=sel['key'], tier=chosen[0], requested_tier=requested, profile=p['id'], facts=emitted, selected_facts=spec['inspection']['state']['selected_facts'],
                    status=status, asset=sel['asset'], exact=sel['exact'], pending=sel['pending'], generation=sel['generation'], reason=sel['reason'], repaired=repaired, approval=state)

    def approve(self, k, uri, sha):
        out = self.registry.approve(key(k), uri, sha); self.approvals[k] = dict(key=k, asset_uri=uri, digest=sha); return out

    def reject(self, k, reason):
        out = self.registry.reject(key(k), reason); self.approvals.pop(k, None); return out

    def submit(self, k, ticket, uri, sha): return self.registry.submit(key(k), ticket, uri, sha)

    def queue_repair(self, base, patches):
        x = repair(dict(base=base, patches=patches))
        assert base in self.repairs or len(self.repairs) < 64
        self.repairs[base] = copy.deepcopy(x)
        if base in self.applied: self.applied.remove(base)
        return dict(base=base, patches=len(patches))


def workflow(a):
    fields(a, 'config registry checkpoint operations'); assert isinstance(a['operations'], list) and len(a['operations']) <= 256
    d = Director(a['config'], a['registry'], a['checkpoint']); keys = []; tickets = []; results = []
    for op in a['operations']:
        try:
            fields(op, 'op args'); v = op['args']; k = op['op']
            if k == 'direct':
                fields(v, 'request missing'); result = d.direct(v['request'], v['missing']); keys.append(result['key'])
                tickets.append(None if result['generation'] is None else result['generation']['ticket'])
            elif k == 'approve': fields(v, 'key asset_uri digest'); result = d.approve(keys[integer(v['key'], 0, 255)], v['asset_uri'], v['digest'])
            elif k == 'submit': fields(v, 'key ticket asset_uri digest'); result = d.submit(keys[integer(v['key'], 0, 255)], tickets[integer(v['ticket'], 0, 255)], v['asset_uri'], v['digest'])
            elif k == 'reject': fields(v, 'key reason'); result = d.reject(keys[integer(v['key'], 0, 255)], v['reason'])
            elif k == 'repair': fields(v, 'base patches'); result = d.queue_repair(keys[integer(v['base'], 0, 255)], v['patches'])
            elif k == 'checkpoint': fields(v, ''); result = d.checkpoint()
            else: raise ValueError(k)
            results.append(dict(ok=True, result=copy.deepcopy(result)))
        except (AssertionError, ValueError, KeyError, IndexError, TypeError, StopIteration): results.append(dict(ok=False))
    return dict(results=results, checkpoint=d.checkpoint(), actual_model_calls=0)
