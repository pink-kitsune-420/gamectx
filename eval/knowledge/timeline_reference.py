"""Independent original world timeline: the host's authoritative, append-only history of
what happened in the world. Python standard library only; nothing here calls a model.

A timeline is `{version:1, next, entries}`. The host records entries (it alone writes
history), retracts mistakes (kept, hidden from queries by default), compacts old entries
into one summary when it chooses (never automatically) and queries what a character
knows: public entries, entries they acted in, and witnessed entries they witnessed.
"""
import copy, json, re

MAX_ENTRIES, MAX_SAFE = 4096, 9007199254740991
VISIBILITY = ['public', 'witnessed', 'private']
ORDERS = ['newest', 'oldest']
ENTRY = ['seq', 'at', 'kind', 'summary', 'actors', 'place', 'things', 'tags', 'visibility', 'witnesses', 'phrases', 'facts', 'retracted', 'covers']
RECORD_REQUIRED, RECORD_OPTIONAL = ['at', 'kind', 'summary'], ['actors', 'place', 'things', 'tags', 'visibility', 'witnesses', 'phrases', 'facts']


def enc(v): return json.dumps(v, ensure_ascii=False, separators=(',', ':'))
def control(s): return any(ord(c) < 32 or 127 <= ord(c) <= 159 for c in s)
def text(s, chars):
    assert isinstance(s, str) and 1 <= len(s) <= chars and not control(s); return s
def ident(s): assert isinstance(s, str) and re.fullmatch('[A-Za-z0-9_:-]{1,64}', s); return s
def uint(n, lo=0, hi=MAX_SAFE): assert type(n) is int and lo <= n <= hi; return n
def boolean(b): assert type(b) is bool; return b
def keys(v, required, optional=()): assert isinstance(v, dict) and set(required) <= set(v) <= set(required) | set(optional)
def norm(s): return ' '.join(''.join(c if c.isalnum() else ' ' for c in s.lower()).split())


def ids(v, hi):
    assert isinstance(v, list) and len(v) <= hi
    for x in v: ident(x)
    assert len(set(v)) == len(v); return list(v)


def phrases(v):
    assert isinstance(v, list) and len(v) <= 8; out = []
    for p in v:
        text(p, 128); assert len(p.encode()) <= 128; q = norm(p); assert q and q not in out; out.append(q)
    return out


def facts(v):
    assert isinstance(v, dict) and len(v) <= 16; out = {}
    for k, x in v.items():
        ident(k)
        if type(x) is bool or type(x) is int and -10**9 <= x <= 10**9: out[k] = x
        else: out[k] = text(x, 64)
    return out


def recorded(seq, e):
    """A canonical entry from a record request."""
    keys(e, RECORD_REQUIRED, RECORD_OPTIONAL)
    vis = e.get('visibility', 'public'); assert vis in VISIBILITY
    place = e.get('place'); place = None if place is None else ident(place)
    return dict(seq=seq, at=uint(e['at']), kind=ident(e['kind']), summary=text(e['summary'], 240), actors=ids(e.get('actors', []), 16), place=place,
                things=ids(e.get('things', []), 16), tags=ids(e.get('tags', []), 8), visibility=vis, witnesses=ids(e.get('witnesses', []), 64),
                phrases=phrases(e.get('phrases', [])), facts=facts(e.get('facts', {})), retracted=None, covers=None)


def stored(e):
    """Validate one stored entry (from a loaded timeline) and return it canonical."""
    assert isinstance(e, dict) and list(e) == ENTRY
    out = recorded(uint(e['seq'], 1), {k: e[k] for k in RECORD_REQUIRED + RECORD_OPTIONAL})
    assert out['phrases'] == e['phrases']  # stored phrases are already normalized
    if e['retracted'] is not None: out['retracted'] = text(e['retracted'], 120)
    if e['covers'] is not None:
        c = e['covers']; assert isinstance(c, dict) and list(c) == ['from', 'through', 'count']
        assert 1 <= uint(c['from'], 1) < uint(c['through'], 1) == out['seq'] and uint(c['count'], 2) <= MAX_SAFE
        out['covers'] = dict(c)
    return out


class Timeline:
    def __init__(self, v=None):
        if v is None: self.next, self.entries = 1, []; return
        keys(v, ['version', 'next', 'entries']); assert v['version'] == 1 and type(v['version']) is int
        self.next = uint(v['next'], 1); assert isinstance(v['entries'], list) and len(v['entries']) <= MAX_ENTRIES
        self.entries = [stored(e) for e in v['entries']]
        seqs = [e['seq'] for e in self.entries]; assert seqs == sorted(set(seqs)) and all(s < self.next for s in seqs)

    def value(self): return dict(version=1, next=self.next, entries=copy.deepcopy(self.entries))

    def find(self, seq): return next(e for e in self.entries if e['seq'] == uint(seq, 1))

    def record(self, e):
        assert len(self.entries) < MAX_ENTRIES and self.next < MAX_SAFE
        out = recorded(self.next, e); self.entries.append(out); self.next += 1
        return dict(seq=out['seq'])

    def retract(self, seq, reason):
        e = self.find(seq); assert e['retracted'] is None; e['retracted'] = text(reason, 120)
        return dict(seq=seq)

    def compact(self, through, summary, kind):
        uint(through, 1); text(summary, 240); ident(kind)
        covered = [e for e in self.entries if e['seq'] <= through]
        assert len(covered) >= 2 and covered[-1]['seq'] == through
        count = sum(1 if e['covers'] is None else e['covers']['count'] for e in covered)
        first = covered[0]['seq'] if covered[0]['covers'] is None else covered[0]['covers']['from']
        out = dict(seq=through, at=max(e['at'] for e in covered), kind=kind, summary=summary, actors=[], place=None, things=[], tags=[], visibility='public',
                   witnesses=[], phrases=[], facts={}, retracted=None, covers={'from': first, 'through': through, 'count': count})
        self.entries = [out] + [e for e in self.entries if e['seq'] > through]
        return dict(seq=through, count=count)

    def query(self, f):
        keys(f, [], ['known_to', 'actor', 'place', 'thing', 'kind', 'tag', 'since', 'until', 'about', 'limit', 'order', 'include_retracted'])
        for k in ['known_to', 'actor', 'place', 'thing', 'kind', 'tag']:
            if k in f: ident(f[k])
        since, until = uint(f.get('since', 0)), uint(f.get('until', MAX_SAFE)); assert since <= until
        about = norm(text(f['about'], 128)) if 'about' in f else None; assert about != ''
        limit = uint(f.get('limit', 16), 1, 64); order = f.get('order', 'newest'); assert order in ORDERS
        retracted = boolean(f.get('include_retracted', False))

        def ok(e):
            if e['retracted'] is not None and not retracted: return False
            if not since <= e['at'] <= until: return False
            if 'known_to' in f:
                w = f['known_to']
                if not (e['visibility'] == 'public' or w in e['actors'] or e['visibility'] == 'witnessed' and w in e['witnesses']): return False
            if 'actor' in f and f['actor'] not in e['actors']: return False
            if 'place' in f and e['place'] != f['place']: return False
            if 'thing' in f and f['thing'] not in e['things']: return False
            if 'kind' in f and e['kind'] != f['kind']: return False
            if 'tag' in f and f['tag'] not in e['tags']: return False
            if about is not None and not any(words_in(p, about) or words_in(about, p) for p in e['phrases']): return False
            return True
        hits = sorted((e for e in self.entries if ok(e)), key=lambda e: (e['at'], e['seq']), reverse=order == 'newest')
        return dict(total=len(hits), entries=copy.deepcopy(hits[:limit]))


def words_in(needle, hay): return (' ' + needle + ' ') in (' ' + hay + ' ')


def workflow(a):
    keys(a, ['timeline', 'ops']); assert isinstance(a['ops'], list) and len(a['ops']) <= 64
    t = Timeline(a['timeline']); results = []
    for op in a['ops']:
        before = copy.deepcopy((t.next, t.entries))
        try:
            keys(op, ['op', 'args']); k, v = op['op'], op['args']
            if k == 'record': result = t.record(v)
            elif k == 'retract': keys(v, ['seq', 'reason']); result = t.retract(v['seq'], v['reason'])
            elif k == 'compact': keys(v, ['through', 'summary'], ['kind']); result = t.compact(v['through'], v['summary'], v.get('kind', 'summary'))
            elif k == 'query': result = t.query(v)
            else: raise ValueError(k)
            results.append(dict(ok=True, result=result))
        except (AssertionError, ValueError, KeyError, TypeError, StopIteration):
            t.next, t.entries = before; results.append(dict(ok=False))
    return dict(results=results, timeline=t.value())
