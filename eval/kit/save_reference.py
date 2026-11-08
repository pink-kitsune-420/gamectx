"""Independent reference for packed game data files and multi-state save files (0.4.0a).

Zero model calls. Two documents share one container shape:

  gamectx-data  authored definitions (codex, props, lore, cast packs, character sheets,
                roleplay engine configs, host data), one section per path.
  gamectx-save  any number of named states (slots, autosaves, forks); each state is a
                complete snapshot of runtime sections (engine memory, long-term memory,
                journals, facts, brains, inventories, host data) and records its parent,
                the host's `saved_at` value and its own digest. The save links to the
                digest of the game data file it was made against.

Section paths are 2-4 identifier segments: `world/<...>`, `characters/<id>/<...>` or
`host/<...>`. Per-character kinds (`sheet`, `engine-config`, `engine`) live at exactly
`characters/<id>/<name>`. Sections are kept sorted by path, so packing is canonical.

Digests are 64-bit FNV-1a (16 hex digits) of compact JSON, used to notice changed or
mismatched data, never for security:
  section  fingerprint(enc(data))
  data     fingerprint(enc({format, version, title, sections:[{path, kind, digest}]}))
  state    fingerprint(enc({name, label, parent, saved_at, sections:[{path, kind, digest}]}))
  save     fingerprint(enc({format, version, game, states:[{name, digest}]}))

Deep validation: this reference fully validates codex, sheet, props, engine-config and
engine sections (with the roleplay engine references). Rust additionally re-validates
lore, cast-pack, memory, journal, facts, brain and inventory sections with their own tested
parsers; this reference only checks that those are JSON objects, so conformance cases never
contain deep-invalid data of those kinds.

Predeclared expectations (written before the Rust implementation existed):
  1. Packing is canonical: the same sections inserted in any order give byte-identical
     files and digests.
  2. Changing any section's data changes that section's digest and the state/file digest;
     a file whose recorded digests do not match its contents is rejected.
  3. Forking copies every section byte for byte (equal section digests), records the
     parent, and changes the state digest only through name, parent, saved_at and label.
  4. A save loaded against a different game data file reports `game_matches: false`
     (never silently); engine states are re-validated against the data file's engine
     config at the same path, and a state that does not fit is rejected.
  5. Rejected operations (bad path, unknown kind, invalid codex, a sheet naming an unknown
     faction, an engine config whose character id differs from its path, removing a codex
     that a sheet still needs) leave the working documents unchanged.
"""
import copy, re
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import roleplay_definitions_reference as defs
import roleplay_engine_reference as eng
import timeline_reference
import things_reference
from roleplay_engine_reference import ROOT as ROOT_DIR, enc, keys, ident, text, uint

DATA_KINDS = ['codex', 'props', 'sheet', 'engine-config', 'lore', 'cast-pack', 'host']
SAVE_KINDS = ['engine', 'memory', 'journal', 'facts', 'brain', 'inventory', 'timeline', 'things', 'host']
PER_CHARACTER = ['sheet', 'engine-config', 'engine']
MAX_SECTIONS, MAX_STATES, MAX_SAFE = 4096, 256, 9007199254740991


def fingerprint(s):
    h = 0xcbf29ce484222325
    for b in s.encode():
        h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return '%016x' % h


def path_ok(p, kind):
    assert isinstance(p, str) and len(p) <= 256; parts = p.split('/')
    assert 2 <= len(parts) <= 4 and all(re.fullmatch('[A-Za-z0-9_:-]{1,64}', x) for x in parts) and parts[0] in ['world', 'characters', 'host']
    if parts[0] == 'characters': assert len(parts) >= 3
    if kind in PER_CHARACTER: assert parts[0] == 'characters' and len(parts) == 3
    return parts


def codex_of(sections):
    s = next((s for s in sections if s['path'] == 'world/codex' and s['kind'] == 'codex'), None)
    return defs.codex(s['data'] if s else dict(version=1), eng.VARS)


def check_data(path, kind, data, sections):
    assert kind in DATA_KINDS; parts = path_ok(path, kind)
    if kind == 'codex': defs.codex(data, eng.VARS)
    elif kind == 'props': eng.props_registry(data)
    elif kind == 'sheet': defs.sheet(data, codex_of(sections), eng.VARS)
    elif kind == 'engine-config': assert eng.config(copy.deepcopy(data))['character']['id'] == parts[1]
    elif kind != 'host': assert isinstance(data, dict)


def check_save(path, kind, data):
    assert kind in SAVE_KINDS; path_ok(path, kind)
    if kind == 'engine': keys(data, ['version', 'greetings', 'listeners', 'pending', 'asked']); uint(data['version'], 1, 1)
    elif kind == 'timeline': timeline_reference.Timeline(data)
    elif kind == 'things': things_reference.Things(data)
    elif kind != 'host': assert isinstance(data, dict)


def section(path, kind, data): return dict(path=path, kind=kind, digest=fingerprint(enc(data)), data=copy.deepcopy(data))
def listing(sections): return [dict(path=s['path'], kind=s['kind'], digest=s['digest']) for s in sections]


def put(sections, path, kind, data):
    out = [s for s in sections if s['path'] != path] + [section(path, kind, data)]
    out.sort(key=lambda s: s['path'].encode()); assert len(out) <= MAX_SECTIONS; return out


def revalidate_sheets(sections):
    for s in sections:
        if s['kind'] == 'sheet': check_data(s['path'], 'sheet', s['data'], sections)


def data_digest(title, sections): return fingerprint(enc(dict(format='gamectx-data', version=1, title=title, sections=listing(sections))))
def data_file(title, sections): return dict(format='gamectx-data', version=1, title=title, sections=copy.deepcopy(sections), digest=data_digest(title, sections))
def state_digest(st): return fingerprint(enc(dict(name=st['name'], label=st['label'], parent=st['parent'], saved_at=st['saved_at'], sections=listing(st['sections']))))
def state_value(st): return dict(name=st['name'], label=st['label'], parent=st['parent'], saved_at=st['saved_at'], sections=copy.deepcopy(st['sections']), digest=state_digest(st))
def save_digest(game, states): return fingerprint(enc(dict(format='gamectx-save', version=1, game=game, states=[dict(name=s['name'], digest=state_digest(s)) for s in states])))
def save_file(game, states): return dict(format='gamectx-save', version=1, game=copy.deepcopy(game), states=[state_value(s) for s in states], digest=save_digest(game, states))


def load_sections(v, check):
    assert isinstance(v, list) and len(v) <= MAX_SECTIONS; out = []
    for s in v:
        keys(s, ['path', 'kind', 'digest', 'data']); assert s['digest'] == fingerprint(enc(s['data']))
        assert not out or out[-1]['path'].encode() < s['path'].encode(); out.append(copy.deepcopy(s))
    # codex first, then everything else, so sheets see the file's codex
    for s in sorted(out, key=lambda s: s['kind'] == 'sheet'): check(s, out)
    return out


def load_data(v):
    keys(v, ['format', 'version', 'title', 'sections', 'digest']); assert v['format'] == 'gamectx-data'; uint(v['version'], 1, 1); text(v['title'], 120)
    sections = load_sections(v['sections'], lambda s, all_: check_data(s['path'], s['kind'], s['data'], all_))
    assert v['digest'] == data_digest(v['title'], sections); return v['title'], sections


def state_meta(name, label, parent, saved_at):
    ident(name); uint(saved_at, 0, MAX_SAFE)
    if label is not None: text(label, 120)
    if parent is not None: ident(parent); assert parent != name


def load_save(v):
    keys(v, ['format', 'version', 'game', 'states', 'digest']); assert v['format'] == 'gamectx-save'; uint(v['version'], 1, 1)
    if v['game'] is not None: keys(v['game'], ['title', 'digest']); text(v['game']['title'], 120); assert re.fullmatch('[0-9a-f]{16}', v['game']['digest'])
    assert isinstance(v['states'], list) and len(v['states']) <= MAX_STATES; states = []
    for s in v['states']:
        keys(s, ['name', 'label', 'parent', 'saved_at', 'sections', 'digest']); state_meta(s['name'], s['label'], s['parent'], s['saved_at'])
        assert not states or states[-1]['name'].encode() < s['name'].encode()
        st = dict(name=s['name'], label=s['label'], parent=s['parent'], saved_at=s['saved_at'], sections=load_sections(s['sections'], lambda x, _: check_save(x['path'], x['kind'], x['data'])))
        assert s['digest'] == state_digest(st); states.append(st)
    assert v['digest'] == save_digest(v['game'], states); return copy.deepcopy(v['game']), states


def check_against(states, sections):
    """Expectation 4: engine states must fit the data file's engine config at the same path."""
    for st in states:
        for s in st['sections']:
            if s['kind'] != 'engine': continue
            cfg = next((d for d in sections if d['path'] == s['path'] and d['kind'] == 'engine-config'), None); assert cfg is not None
            eng.Engine(copy.deepcopy(cfg['data']), copy.deepcopy(s['data']))


def manifest(st): return dict(name=st['name'], parent=st['parent'], saved_at=st['saved_at'], digest=state_digest(st), sections=listing(st['sections']))


class Workspace:
    def __init__(self): self.data = None; self.game = None; self.states = []

    def op(self, op, a):
        if op == 'pack_data':
            keys(a, ['title', 'sections']); text(a['title'], 120); assert isinstance(a['sections'], list) and len(a['sections']) <= MAX_SECTIONS; sections = []
            for s in a['sections']:
                keys(s, ['path', 'kind', 'data']); assert all(x['path'] != s['path'] for x in sections); sections = put(sections, s['path'], s['kind'], s['data'])
            for s in sorted(sections, key=lambda s: s['kind'] == 'sheet'): check_data(s['path'], s['kind'], s['data'], sections)
            self.data = (a['title'], sections); return data_file(*self.data)
        if op == 'put_data':
            keys(a, ['path', 'kind', 'data']); assert self.data is not None; title, sections = self.data
            check_data(a['path'], a['kind'], a['data'], sections); new = put(sections, a['path'], a['kind'], a['data']); revalidate_sheets(new)
            self.data = (title, new); return dict(digest=data_digest(title, new))
        if op == 'remove_data':
            keys(a, ['path']); assert self.data is not None; title, sections = self.data; new = [s for s in sections if s['path'] != a['path']]
            assert len(new) < len(sections); revalidate_sheets(new); self.data = (title, new); return dict(digest=data_digest(title, new))
        if op == 'load_data':
            self.data = load_data(a); return dict(title=self.data[0], digest=data_digest(*self.data), sections=listing(self.data[1]))
        if op == 'link':
            keys(a, []); assert self.data is not None; self.game = dict(title=self.data[0], digest=data_digest(*self.data)); return dict(game=self.game)
        if op == 'save':
            keys(a, ['name', 'saved_at', 'sections'], ['label', 'parent']); state_meta(a['name'], a.get('label'), a.get('parent'), a['saved_at'])
            assert isinstance(a['sections'], list); sections = []
            for s in a['sections']:
                keys(s, ['path', 'kind', 'data']); assert all(x['path'] != s['path'] for x in sections); check_save(s['path'], s['kind'], s['data']); sections = put(sections, s['path'], s['kind'], s['data'])
            st = dict(name=a['name'], label=a.get('label'), parent=a.get('parent'), saved_at=a['saved_at'], sections=sections)
            if self.data is not None: check_against([st], self.data[1])
            states = sorted([s for s in self.states if s['name'] != a['name']] + [st], key=lambda s: s['name'].encode()); assert len(states) <= MAX_STATES
            self.states = states; return manifest(st)
        if op == 'fork':
            keys(a, ['from', 'name', 'saved_at'], ['label']); src = next(s for s in self.states if s['name'] == a['from'])
            state_meta(a['name'], a.get('label'), a['from'], a['saved_at']); assert all(s['name'] != a['name'] for s in self.states) and len(self.states) < MAX_STATES
            st = dict(name=a['name'], label=a.get('label'), parent=a['from'], saved_at=a['saved_at'], sections=copy.deepcopy(src['sections']))
            self.states = sorted(self.states + [st], key=lambda s: s['name'].encode()); return manifest(st)
        if op == 'remove_state':
            keys(a, ['name']); n = len(self.states); self.states = [s for s in self.states if s['name'] != a['name']]; assert len(self.states) < n; return dict(states=[s['name'] for s in self.states])
        if op == 'export_save':
            keys(a, []); return save_file(self.game, self.states)
        if op == 'load_save':
            game, states = load_save(a)
            if self.data is not None: check_against(states, self.data[1])
            self.game, self.states = game, states
            matches = None if self.data is None or game is None else game['digest'] == data_digest(*self.data)
            return dict(game_matches=matches, states=[manifest(s) for s in states])
        raise AssertionError(op)


def oracle(a):
    keys(a, ['ops']); assert isinstance(a['ops'], list) and 1 <= len(a['ops']) <= 32; w = Workspace(); out = []
    for o in a['ops']:
        keys(o, ['op', 'args']); assert isinstance(o['op'], str)
        before = copy.deepcopy((w.data, w.game, w.states))
        try:
            out.append(dict(ok=w.op(o['op'], o['args'])))
        except (AssertionError, KeyError, TypeError, ValueError, StopIteration, AttributeError, IndexError):
            assert copy.deepcopy((w.data, w.game, w.states)) == before; out.append(dict(error=o['op']))  # expectation 5
    return dict(results=out)
