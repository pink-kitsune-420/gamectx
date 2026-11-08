"""Independent reference for the configurable roleplay engine (0.4.0a, stages 2.1 and 2.1b).

Zero model calls. Composes the existing independent oracles (message chunks, expressive
routing and claims, registered stage actions, the command grammar, social routing and
greeting memory) and adds the engine's own contracts: config defaults, the line book,
phrase matchers, props, stage directions, per-listener state and delivery receipts.

Predeclared expectations (written before the Rust implementation existed):
  1. Every stage answers only when one of its line rules matches; otherwise the turn falls
     through, so `pack: "none"` with no lines sends every conversational turn to the model.
  2. Disabling a feature never changes the outcome of an earlier stage in the cascade.
  3. Prop interactions and stage actions only propose host decisions; a decision that is not
     one of the turn's legal choices is blocked, never executed.
  4. Variety only counts fully heard replies; a partial delivery changes nothing but
     removes the pending reply.
  5. With the default English pack, Lantern Vault's greeting and social wording is
     reproduced line for line (checked against the frozen web host wording in stage 2.4).

Stage 2.1b (definitions; predeclared before the Rust implementation):
  6. With an empty character sheet and codex, every 2.1 outcome is unchanged; only the
     normalized config echo and the listener record (new `revealed` list) grow.
  7. A secret is spoken only when its `when` conditions hold for this turn; otherwise
     its cover story (or a withheld line) is spoken and the secret text never appears.
  8. Hidden entrances are never listed, described or traversed; traversal proposes the
     side's travel decision only when the entrance is open and the decision is legal.
  9. A norm reaction is attached only to action outcomes whose decision matches the
     current place's norm; it never blocks or changes the decision.

Stage 2.2 (model requests; predeclared before the Rust implementation and before any model
output was observed):
 10. A `model` outcome carries a complete chat request: pack rules, a stable character
     brief, the per-turn situation, then the player's words, constrained to a
     `{"speech": string}` JSON schema. A secret's text appears in it only after that
     secret was revealed to this listener.
 11. Compiling never changes which stage answers: every outcome other than `model` is
     byte-identical to stage 2.1b apart from the config echo and the checkpoint's `asked`.
 12. `reply` accepts only an exact speech object that passes every bounded check; an
     accepted reply is pending until delivered like any other, and a rejection keeps the
     request open and changes nothing else.
"""
import copy, json, pathlib, re
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import roleplay_definitions_reference as defs
import expression_cases, greeting_memory_cases, message_chunks_cases, overnight_input_cases, roleplay_actions_cases, social_routing_cases

ROOT = pathlib.Path(__file__).resolve().parents[2]
PACKS = {'en': json.loads((ROOT / 'src/character/roleplay/engine/en.json').read_text()), 'none': dict(version=1, language='none', phrases={}, lists=None, articles=[], lines=[], model_rules=[])}
FEATURES = ['chunks', 'out_of_character', 'addressing', 'stage_actions', 'props', 'directions', 'perception', 'commands', 'disclosures', 'recall', 'greetings', 'social', 'model',
            'content', 'entrances', 'scenery', 'persona', 'services', 'secrets', 'opinions', 'relationships', 'factions', 'events', 'quests', 'goals', 'bio',
            'timeline', 'places', 'things', 'recipes', 'communities', 'species', 'reactions', 'viewpoints']
PARAMS = dict(chunks=dict(max_chars=(512, 4096, 1024)), addressing=dict(action_turns=(0, 100, 3), conversational_distance=(1, 100, 5), max_distance=(1, 1000, 20)),
              disclosures=dict(max_preferences=(1, 16, 3)), greetings=dict(long_gap_turns=(1, 1000000, 12)), events=dict(soon_turns=(1, 1000000, 10)),
              model=dict(max_tokens=(16, 1024, 160), max_sentences=(1, 4, 3), max_chars=(40, 240, 240), brief_chars=(512, 16384, 4096)))
VARS = set('name scene place atmosphere trust relationship meeting needs_introduction sight sound has_sight has_sound props has_props prop description note has_note slot value old corrects polarity known reason has_reason '
           'exits status destination entrance through heard smell light temperature floor has_floor ceiling has_ceiling exterior role summary services label text need has_need person kind faction stance event events timing quest stage sentences facing spot '
           'material color texture clothing eyes hair skin posture activity awareness expression scent '
           'held has_held has_posture gaze has_gaze has_clothing has_activity has_material has_color has_texture has_matter state '
           'age has_age birthday has_birthday birth_month birth_day is_birthday profession has_profession health has_health conditions has_conditions skills has_skills skill skill_level knowledge knowledge_topic knowledge_level family has_family relation member family_status species '
           'news nearby things recipe inputs community currency resource temperament'.split())
MATCHERS = ['exact', 'contains', 'prefix', 'unless']
U32 = 4294967295


def enc(v): return json.dumps(v, ensure_ascii=False, separators=(',', ':'))
def control(s): return any(ord(c) < 32 or 127 <= ord(c) <= 159 for c in s)
def text(s, chars, nbytes=None):
    assert isinstance(s, str) and 1 <= len(s) <= chars and not control(s)
    if nbytes is not None: assert len(s.encode()) <= nbytes
    return s
def ident(s): assert isinstance(s, str) and re.fullmatch('[A-Za-z0-9_:-]{1,64}', s); return s
def uint(n, lo=0, hi=U32): assert type(n) is int and lo <= n <= hi; return n
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
    assert isinstance(v, dict)
    keys(v, ['width', 'length', 'height'])
    for k in ['width', 'length', 'height']:
        x = v[k]; assert (type(x) is int or type(x) is float) and 0.0 < x <= 100000.0
    return [float(v['width']), float(v['length']), float(v['height'])]
def dist_val(v):
    if v is None: return None
    if type(v) is int or type(v) is float:
        assert 0.0 <= v <= 1000.0; return float(v)
    assert isinstance(v, str) and v in ('near', 'conversational', 'far', 'distant'); return v
def audience_val(v):
    if isinstance(v, list):
        assert 1 <= len(v) <= 16
        ids = [ident(x) for x in v]; assert len(set(ids)) == len(ids)
        return v
    return ident(v)
def calc_dist(d, c1, c2):
    if d is not None:
        if type(d) is int or type(d) is float: return float(d)
        if d == 'near': return 1.0
        if d == 'conversational': return 3.0
        if d == 'far': return 10.0
        if d == 'distant': return 30.0
    if c1 is not None and c2 is not None and len(c1) == len(c2):
        return sum((float(a) - float(b)) ** 2 for a, b in zip(c1, c2)) ** 0.5
    return None
def boolean(b): assert type(b) is bool; return b
def keys(v, required, optional=()):
    assert isinstance(v, dict) and set(required) <= set(v) <= set(required) | set(optional)
def norm(s): return ' '.join(''.join(c if c.isalnum() else ' ' for c in s.lower()).split())
def decision(d):
    if d is not None:
        keys(d, ['action', 'target']); ident(d['action'])
        if d['target'] is not None: ident(d['target'])
    return d


def template(t):
    text(t, 240); names = []; rest = t
    while rest:
        i = min([x for x in (rest.find('{'), rest.find('}')) if x >= 0], default=-1)
        if i < 0: break
        assert rest[i] == '{'; j = rest.find('}', i); assert j > i
        name = rest[i + 1:j]; assert name in VARS; names.append(name); rest = rest[j + 1:]
    return names
def render(t, v):
    out = ''; rest = t
    while '{' in rest:
        i = rest.index('{'); j = rest.index('}', i); name = rest[i + 1:j]
        if name not in v: return None
        x = v[name]; out += rest[:i] + ('true' if x is True else 'false' if x is False else str(x)); rest = rest[j + 1:]
    return out + rest


def matcher(m):
    assert isinstance(m, dict) and m and set(m) <= set(MATCHERS); out = {}
    for k in MATCHERS:
        if k in m:
            assert isinstance(m[k], list) and 1 <= len(m[k]) <= 32; out[k] = []
            for p in m[k]: text(p, 128, 128); q = norm(p); assert q; out[k].append(q)
    return out
def match(m, q):
    """(matched, remainder): unless vetoes; then exact, prefix (word boundary), contains."""
    if m is None: return False, ''
    padded = ' ' + q + ' '
    if any(' ' + p + ' ' in padded for p in m.get('unless', [])): return False, ''
    if q in m.get('exact', []): return True, ''
    for p in m.get('prefix', []):
        if q == p or q.startswith(p + ' '): return True, q[len(p):].strip()
    if any(' ' + p + ' ' in padded for p in m.get('contains', [])): return True, ''
    return False, ''


def rule(r):
    keys(r, ['id', 'intent', 'variants'], ['when', 'introduces']); ident(r['id']); ident(r['intent'])
    assert isinstance(r['variants'], list) and 1 <= len(r['variants']) <= 8
    for t in r['variants']: template(t)
    boolean(r.get('introduces', False)); when = r.get('when', {}); assert isinstance(when, dict) and len(when) <= 8
    defs.conditions(when, VARS)
    return r
SPEECH = re.compile(r'\A[ \t\n\r]*\{[ \t\n\r]*"speech"[ \t\n\r]*:[ \t\n\r]*("(?:[^"\\\x00-\x1f]|\\["\\/bfnrt]|\\u[0-9A-Fa-f]{4})*")[ \t\n\r]*\}[ \t\n\r]*\Z')
FENCE = re.compile(r'\A[ \t\n\r]*```(?:json)?[ \t]*\n(.*)\n[ \t]*```[ \t\n\r]*\Z', re.S)
def check_speech(raw, e, who):
    """(reason, speech): the first failed check, or (None, speech). Guardrails, not proof of truth."""
    m = e.cfg['features']['model']
    if len(raw.encode()) > 16384: return 'length', None
    f = FENCE.match(raw); body = f.group(1) if f else raw
    g = SPEECH.match(body)
    if g is None: return 'format', None
    s = json.loads(g.group(1))
    if any(0xD800 <= ord(c) <= 0xDFFF for c in s): return 'format', None
    s = s.strip(' \t\n\r')
    if not s: return 'empty', None
    if len(s) > m['max_chars']: return 'length', None
    if control(s): return 'control', None
    if any(c in s for c in '*[]{}<>'): return 'stage', None
    runs = 0; prev = False
    for c in s:
        end = c in '.!?'; runs += end and not prev; prev = end
    tail = s.rstrip('"\')\u201d\u2019 ')
    if tail and tail[-1] not in '.!?': runs += 1
    if runs > m['max_sentences']: return 'sentences', None
    q = norm(s); padded = ' ' + q + ' '
    if match(e.phrases.get('model_reject'), q)[0]: return 'out_of_character', None
    if e.avoided(s) or e.on('content') and any(' ' + norm(p) + ' ' in padded for a in e.cfg['features']['content']['avoid'] for p in a['phrases']): return 'avoided', None
    l = e.listener(who)
    told = l['revealed'] if l else []
    if any(norm(x['reveal']) in q for x in e.sheet.get('secrets', []) if x['id'] not in told): return 'secret', None
    words = set(re.split(r'[^a-z0-9_:-]+', s.lower()))
    if any(i.lower() in words for i in [who, e.cfg['character']['id']] if any(c in i for c in '_:0123456789')): return 'identifier', None
    if l and any(norm(r) == q for r in l['recent']): return 'repeat', None
    return None, s


def when_ok(when, v):
    for k, c in when.items():
        if k not in v: return False
        x = v[k]
        if isinstance(c, dict):
            if type(x) is not int or x < c.get('min', x) or x > c.get('max', x): return False
        elif not any(type(y) is type(x) and y == x for y in c): return False
    return True


def props_registry(v):
    keys(v, ['version', 'props']); assert v['version'] == 1 and isinstance(v['props'], list) and len(v['props']) <= 128
    ids, names, phrases = [], [], []
    for p in v['props']:
        keys(p, ['id', 'label', 'aliases', 'description', 'interactions'], ['dimensions', 'mass', 'state', 'material', 'color', 'texture', 'tags', 'attributes']); ident(p['id']); assert p['id'] not in ids; ids.append(p['id'])
        if 'dimensions' in p and p['dimensions'] is not None: dimensions_val(p['dimensions'])
        if 'mass' in p and p['mass'] is not None:
            assert (type(p['mass']) is int or type(p['mass']) is float) and 0.0 < p['mass'] <= 1000000.0
        if 'state' in p and p['state'] is not None: assert p['state'] in ['solid', 'liquid', 'gas']
        for k in ['material', 'color', 'texture']:
            if k in p and p[k] is not None: text(p[k], 48)
        if 'tags' in p and p['tags'] is not None:
            for tg in lst(p['tags'], 0, 32): text(tg, 48)
        if 'attributes' in p and p['attributes'] is not None:
            assert isinstance(p['attributes'], dict) and len(p['attributes']) <= 32
            for k, val in p['attributes'].items():
                ident(k)
                if isinstance(val, str): text(val, 120)
                else: assert type(val) in (int, float, bool)
        text(p['label'], 48); text(p['description'], 240); assert isinstance(p['aliases'], list) and len(p['aliases']) <= 8
        for n in [p['label']] + p['aliases']:
            text(n, 48); q = norm(n); assert q and q not in names; names.append(q)
        assert isinstance(p['interactions'], list) and len(p['interactions']) <= 8; seen = []
        for i in p['interactions']:
            keys(i, ['id', 'phrases', 'decision', 'narration']); ident(i['id']); assert i['id'] not in seen; seen.append(i['id'])
            decision(i['decision']); text(i['narration'], 240); assert isinstance(i['phrases'], list) and 1 <= len(i['phrases']) <= 8
            for ph in i['phrases']: text(ph, 128, 128); q = norm(ph); assert q and q not in phrases; phrases.append(q)
    return v


def config(c):
    keys(c, ['version', 'character'], ['features', 'variety', 'pack', 'phrases', 'lines', 'lists', 'articles', 'sheet', 'codex', 'model_rules'])
    uint(c['version'], 1, 1); keys(c['character'], ['id', 'name']); ident(c['character']['id']); text(c['character']['name'], 32)
    fin = c.get('features', {}); assert isinstance(fin, dict) and set(fin) <= set(FEATURES); features = {}
    for f in FEATURES:
        v = fin.get(f, {})
        if f == 'content': features[f] = defs.content(v); continue
        extra = list(PARAMS.get(f, {})) + (['actions'] if f == 'stage_actions' else ['registry'] if f == 'props' else [])
        keys(v, [], ['enabled'] + extra); out = dict(enabled=boolean(v.get('enabled', True)))
        for k, (lo, hi, d) in PARAMS.get(f, {}).items(): out[k] = uint(v.get(k, d), lo, hi)
        if f == 'stage_actions':
            out['actions'] = v.get('actions', dict(version=1, entries=[])); roleplay_actions_cases.oracle(dict(registry=out['actions'], input='x', choices=[]))
        if f == 'props': out['registry'] = props_registry(v.get('registry', dict(version=1, props=[])))
        features[f] = out
    variety = c.get('variety', {}); keys(variety, [], ['recent']); variety = dict(recent=uint(variety.get('recent', 2), 0, 8))
    pack = c.get('pack', 'en'); assert pack in PACKS
    phrases = c.get('phrases', {}); assert isinstance(phrases, dict) and len(phrases) <= 64
    for k, m in phrases.items(): ident(k); matcher(m)
    lines = c.get('lines', []); assert isinstance(lines, list) and len(lines) <= 128; seen = []
    for r in lines: rule(r); assert r['id'] not in seen; seen.append(r['id'])
    lists = c.get('lists'); articles = c.get('articles')
    if lists is not None: keys(lists, ['separator', 'final']); [assert_sep(lists[k]) for k in ['separator', 'final']]
    if articles is not None:
        assert isinstance(articles, list) and len(articles) <= 8
        for a in articles: text(a, 16); assert norm(a) == a
    cdx = defs.codex(c.get('codex', dict(version=1)), VARS); sh = defs.sheet(c.get('sheet', dict(version=1)), cdx, VARS)
    rules = c.get('model_rules')
    if rules is not None:
        assert isinstance(rules, list) and 1 <= len(rules) <= 8
        for t in rules: template(t)
    return dict(version=1, character=dict(c['character']), features=features, variety=variety, pack=pack, phrases=phrases, lines=lines, lists=lists, articles=articles, sheet=sh, codex=cdx, model_rules=rules)
def assert_sep(s): assert isinstance(s, str) and len(s) <= 16 and not control(s)


class Engine:
    def __init__(self, cfg, state=None):
        self.cfg = config(cfg); p = PACKS[self.cfg['pack']]
        self.phrases = {k: matcher(m) for k, m in p['phrases'].items()}
        for k, m in self.cfg['phrases'].items(): self.phrases[k] = matcher(m)
        own = {r['id']: r for r in self.cfg['lines']}; merged = [own.pop(r['id']) if r['id'] in own else r for r in p['lines']]
        self.lines = [r for r in self.cfg['lines'] if r['id'] in own] + merged; assert len(self.lines) <= 256
        self.lists = self.cfg['lists'] if self.cfg['lists'] is not None else p['lists']
        self.articles = self.cfg['articles'] if self.cfg['articles'] is not None else p['articles']
        self.gcfg = dict(version=1, long_gap_turns=self.cfg['features']['greetings']['long_gap_turns'])
        self.codex = self.cfg['codex']; self.sheet = self.cfg['sheet']
        self.rules = self.cfg['model_rules'] if self.cfg['model_rules'] is not None else p['model_rules']
        self.avoid = [norm(w) for w in self.sheet.get('speech', {}).get('avoid_words', [])]
        self.state = self.restore(state) if state is not None else dict(version=1, greetings=dict(version=1, revision=0, entries=[]), listeners=[], pending=[], asked=[])

    def on(self, f): return self.cfg['features'][f]['enabled']

    def restore(self, s):
        keys(s, ['version', 'greetings', 'listeners', 'pending', 'asked']); uint(s['version'], 1, 1)
        greeting_memory_cases.recover(s['greetings'])
        for e in s['greetings']['entries']: assert e['actor'] == self.cfg['character']['id']
        assert isinstance(s['listeners'], list) and len(s['listeners']) <= 256; ids = []
        for l in s['listeners']:
            keys(l, ['id', 'name', 'preferences', 'recent', 'topic', 'revealed']); ident(l['id']); assert l['id'] not in ids and l['id'] != self.cfg['character']['id']; ids.append(l['id'])
            if l['name'] is not None: text(l['name'], 40, 160)
            assert isinstance(l['preferences'], list) and len(l['preferences']) <= self.cfg['features']['disclosures']['max_preferences']; seen = []
            for p in l['preferences']:
                keys(p, ['value', 'polarity']); text(p['value'], 120, 240); assert p['polarity'] in ['like', 'dislike', 'prefer'] and p['value'].lower() not in seen; seen.append(p['value'].lower())
            assert isinstance(l['recent'], list) and len(l['recent']) <= self.cfg['variety']['recent']
            for r in l['recent']: text(r, 240, 1024)
            if l['topic'] is not None: ident(l['topic'])
            assert isinstance(l['revealed'], list) and len(l['revealed']) <= 32 and len(set(l['revealed'])) == len(l['revealed'])
            for x in l['revealed']: ident(x)
        assert isinstance(s['pending'], list) and len(s['pending']) <= 16; tokens = []
        for p in s['pending']:
            keys(p, ['token', 'listener', 'text', 'scene', 'visit', 'at', 'greeting', 'introduced'])
            uint(p['token'], 1); assert p['token'] not in tokens; tokens.append(p['token']); ident(p['listener']); text(p['text'], 240, 1024); ident(p['scene'])
            uint(p['visit']); uint(p['at']); boolean(p['greeting']); boolean(p['introduced']); assert p['greeting'] or not p['introduced']
        assert isinstance(s['asked'], list) and len(s['asked']) <= 16
        for a in s['asked']:
            keys(a, ['token', 'listener', 'scene', 'visit', 'at']); uint(a['token'], 1); assert a['token'] not in tokens; tokens.append(a['token'])
            ident(a['listener']); ident(a['scene']); uint(a['visit']); uint(a['at'])
        return copy.deepcopy(s)

    def listener(self, who, create=False):
        l = next((l for l in self.state['listeners'] if l['id'] == who), None)
        if l is None and create:
            assert len(self.state['listeners']) < 256; l = dict(id=who, name=None, preferences=[], recent=[], topic=None, revealed=[]); self.state['listeners'].append(l)
        return l

    # Stage `bio` (sheet age, birthday, profession, skills, knowledge, health, family).
    def bio_health(self, turn_health):
        return turn_health if turn_health is not None else (self.sheet.get('bio') or {}).get('health')

    @staticmethod
    def bio_visible(h):
        return sorted([c for c in (h or {}).get('conditions', []) if c['visible'] is True], key=lambda c: (-c['severity'], c['id']))

    def bio_ranked(self):
        return sorted((self.sheet.get('bio') or {}).get('skills', []), key=lambda x: (-x['level'], x['id']))

    def bio_vars(self, base, turn_health, date):
        bio = self.sheet.get('bio') or {}
        if 'species' in bio: base['species'] = bio['species']
        base['has_age'] = 'age' in bio
        if 'age' in bio: base['age'] = bio['age']
        b = bio.get('birthday'); base['has_birthday'] = b is not None; today = False
        if b is not None:
            base['birth_month'] = b['month']; base['birth_day'] = b['day']
            got = self.say('birthday_date', base, [])
            if got is not None: base['birthday'] = got[1]
            today = date is not None and date['month'] == b['month'] and date['day'] == b['day']
        base['is_birthday'] = today
        base['has_profession'] = 'profession' in bio
        if 'profession' in bio: base['profession'] = bio['profession']
        h = self.bio_health(turn_health); base['has_health'] = h is not None
        if h is not None: base['health'] = h['status']
        for flag, var, labels in [('has_conditions', 'conditions', [c['label'] for c in self.bio_visible(h)]), ('has_skills', 'skills', [x['label'] for x in self.bio_ranked()[:3]])]:
            j = self.joined(labels) if labels else None
            j = j if j is not None and len(j) <= 240 else None
            base[flag] = j is not None
            if j is not None: base[var] = j
        members = []
        for m in bio.get('family', []):
            got = self.say('family_member', dict(relation=m['relation'], member=m['name'], family_status=m['status']), [])
            if got is not None: members.append(got[1])
        j = self.joined(members) if members else None
        j = j if j is not None and len(j) <= 240 else None
        base['has_family'] = j is not None
        if j is not None: base['family'] = j

    def bio_skill(self, want):
        for x in (self.sheet.get('bio') or {}).get('skills', []):
            if any(self.bare(norm(n)) == want for n in [x['label']] + x.get('phrases', [])): return x
        return None

    def bio_knowledge(self, want, base):
        x = next((k for k in (self.sheet.get('bio') or {}).get('knowledge', []) if any(self.bare(norm(n)) == want for n in [k['topic']] + k['phrases'])), None)
        return x if x is not None and when_ok(x.get('when', {}), base) else None

    def bio_brief(self, turn_health, base):
        bio = self.sheet.get('bio') or {}; out = {}
        for k in ['species', 'age']:
            if k in bio: out[k] = bio[k]
        if 'birthday' in base: out['birthday'] = base['birthday']
        for k in ['profession', 'anatomy']:
            if k in bio: out[k] = copy.deepcopy(bio[k])
        h = self.bio_health(turn_health)
        if h is not None: out['health'] = dict(status=h['status'], conditions=[c['label'] for c in self.bio_visible(h)])
        if bio.get('skills'): out['skills'] = [dict(name=x['label'], level=x['level']) for x in self.bio_ranked()]
        known = [dict(topic=k['topic'], level=k['level'], summary=k['summary']) for k in bio.get('knowledge', []) if when_ok(k.get('when', {}), base)]
        if known: out['knowledge'] = known
        if bio.get('family'): out['family'] = [dict(name=m['name'], relation=m['relation'], status=m['status']) for m in bio['family']]
        return out or None

    def say(self, intent, v, recent):
        for r in self.lines:
            if r['intent'] != intent or not when_ok(r.get('when', {}), v): continue
            cands = [x for t in r['variants'] if (x := render(t, v)) is not None and 1 <= len(x) <= 240 and not self.avoided(x)]
            if cands: return r, next((x for x in cands if x not in recent), cands[0])
        return None

    def avoided(self, s):
        padded = ' ' + norm(s) + ' '
        return any(' ' + w + ' ' in padded for w in self.avoid)

    def bare(self, q):
        w = q.split()
        return ' '.join(w[1:]) if len(w) > 1 and w[0] in self.articles else q

    def joined(self, labels):
        if self.lists is None: return None
        if len(labels) == 1: return labels[0]
        return self.lists['separator'].join(labels[:-1]) + self.lists['final'] + labels[-1]

    def parse_vocative(self, line, me, targets, sh):
        candidates = [(me['id'], {norm(me['name']), norm(me['id']), 'the ' + norm(me['id'])})]
        for g in targets:
            if g['kind'] == 'character':
                candidates.append((g['id'], {norm(g['id'])} | {norm(n) for n in g['names']}))
        for r in sh.get('relationships', []):
            if not any(c[0] == r['with'] for c in candidates):
                candidates.append((r['with'], {norm(r['with']), norm(r['name'])}))
        stripped = line.strip()
        if stripped.startswith('*') and '*' in stripped[1:]:
            end_ast = stripped.index('*', 1)
            act = norm(stripped[1:end_ast])
            for prefix in ['turns to', 'turn to', 'looks at', 'look at', 'to']:
                if act == prefix or act.startswith(prefix + ' '):
                    target_name = act[len(prefix):].strip()
                    for cid, names in candidates:
                        if target_name in names: return cid
        for sep in [',', ':']:
            if sep in line:
                prefix = norm(line.split(sep, 1)[0])
                for opener in ['hey', 'hi', 'hello', 'greetings']:
                    if prefix.startswith(opener + ' '):
                        prefix = prefix[len(opener) + 1:].strip()
                for cid, names in candidates:
                    if prefix in names: return cid
        if ',' in line:
            tail = norm(line.rsplit(',', 1)[1].rstrip('.!?'))
            for cid, names in candidates:
                if tail in names: return cid
        return None

    def eval_addressing(self, t, line, me, targets, sh, at):
        cfg = self.cfg['features']['addressing']
        my_dist = calc_dist(t.get('distance'), t.get('coords'), t.get('speaker_coords'))
        if my_dist is not None and my_dist > cfg['max_distance']:
            return dict(route='addressing', kind='silent', reason='out_of_earshot')
        audience = t.get('audience'); inferred = False
        if isinstance(audience, list):
            audience = me['id'] if me['id'] in audience else list(audience)
        elif audience is not None:
            ident(audience)
        else:
            voc = self.parse_vocative(line, me, targets, sh)
            if voc is not None:
                audience = voc
            else:
                inferred = True
                closest_id = me['id']
                closest_dist = my_dist if my_dist is not None else float(cfg['conversational_distance'])
                sp_facing = t.get('speaker_facing')
                if sp_facing is not None:
                    sp_norm = norm(sp_facing)
                    if sp_norm in (norm(me['name']), norm(me['id'])):
                        closest_id = me['id']; closest_dist = -1.0
                for g in targets:
                    if g['kind'] == 'character':
                        if sp_facing is not None:
                            sp_norm = norm(sp_facing)
                            if sp_norm in [norm(n) for n in g['names']] or sp_norm == norm(g['id']):
                                closest_id = g['id']; closest_dist = -1.0; continue
                        gd = calc_dist(g.get('distance'), g.get('coords'), t.get('speaker_coords'))
                        if gd is not None and gd < closest_dist:
                            closest_dist = gd; closest_id = g['id']
                audience = closest_id
        if audience == me['id']:
            if inferred and my_dist is not None and my_dist > cfg['conversational_distance']:
                return dict(route='addressing', kind='gesture', reason='too_far', audience=me['id'])
            stripped = line.strip()
            if stripped.endswith(('...', '…', '--', '—', '–', ',')):
                return dict(route='addressing', kind='pause', reason='unfinished')
            return None
        action_turns = cfg['action_turns']
        quirks = sh.get('speech', {}).get('quirks', [])
        if action_turns > 0 and at % action_turns == 0 and quirks:
            narration = quirks[(at // action_turns) % len(quirks)]
            return dict(route='addressing', kind='action', audience=audience, action='quirk', decision=None, narration=narration)
        return dict(route='addressing', kind='silent', audience=audience)

    def turn(self, t):
        cfg = self.cfg; me = cfg['character']
        keys(t, ['listener', 'text', 'at', 'token', 'scene'], ['trust', 'choices', 'labels', 'focus', 'reason', 'targets', 'entrances', 'needs', 'quests', 'state',
                                                               'audience', 'distance', 'facing', 'spot', 'coords', 'speaker_facing', 'speaker_spot', 'speaker_coords',
                                                               'line_of_sight', 'blocked_by', 'behind', 'dimensions',
                                                               'features', 'clothing', 'sensory', 'habits', 'tags', 'attributes',
                                                               'posture', 'activity', 'awareness', 'expression', 'held', 'gaze', 'scent', 'health', 'date',
                                                               'timeline', 'things', 'recipes', 'communities', 'species', 'reactions', 'viewpoints', 'loan'])
        if t.get('health') is not None: defs.health(t['health'])
        if t.get('date') is not None: defs.day(t['date'])
        who = ident(t['listener']); assert who != me['id']; at = uint(t['at']); token = uint(t['token'], 1)
        assert all(p['token'] != token for p in self.state['pending'] + self.state['asked'])
        if t.get('audience') is not None: audience_val(t['audience'])
        dist_val(t.get('distance'))
        if t.get('facing') is not None: text(t['facing'], 48)
        if t.get('spot') is not None: ident(t['spot'])
        if t.get('coords') is not None: coords_val(t['coords'])
        if t.get('speaker_facing') is not None: text(t['speaker_facing'], 48)
        if t.get('speaker_spot') is not None: ident(t['speaker_spot'])
        if t.get('speaker_coords') is not None: coords_val(t['speaker_coords'])
        if t.get('line_of_sight') is not None: boolean(t['line_of_sight'])
        if t.get('blocked_by') is not None: ident(t['blocked_by'])
        if t.get('behind') is not None: ident(t['behind'])
        if t.get('dimensions') is not None: dimensions_val(t['dimensions'])
        if t.get('features') is not None:
            f = t['features']; keys(f, [], ['eyes', 'hair', 'skin', 'scars', 'height', 'build'])
            for k in ['eyes', 'hair', 'skin', 'build']:
                if k in f and f[k] is not None: text(f[k], 48)
            for sc_item in lst(f.get('scars', []), 0, 8): text(sc_item, 48)
            if 'height' in f and f['height'] is not None:
                assert (type(f['height']) is int or type(f['height']) is float) and 0.0 < f['height'] <= 100.0
        if t.get('clothing') is not None:
            for item in (t['clothing'] if isinstance(t['clothing'], list) else [t['clothing']]):
                text(item, 48)
        if t.get('sensory') is not None:
            sn = t['sensory']; keys(sn, [], ['scent', 'voice_tone'])
            for k in ['scent', 'voice_tone']:
                if k in sn and sn[k] is not None: text(sn[k], 48)
        if t.get('habits') is not None:
            for h in lst(t['habits'], 0, 16): text(h, 80)
        if t.get('tags') is not None:
            for tg in lst(t['tags'], 0, 32): text(tg, 48)
        if t.get('attributes') is not None:
            att = t['attributes']; assert isinstance(att, dict) and len(att) <= 32
            for k, val in att.items():
                ident(k)
                if isinstance(val, str): text(val, 120)
                else: assert type(val) in (int, float, bool)
        for k in ['posture', 'activity', 'awareness', 'expression', 'scent']:
            if t.get(k) is not None: text(t[k], 80)
        if t.get('gaze') is not None: ident(t['gaze'])
        if t.get('held') is not None:
            if isinstance(t['held'], dict):
                assert len(t['held']) <= 8
                for k, val in t['held'].items(): text(k, 32); text(val, 64)
            elif isinstance(t['held'], list):
                assert len(t['held']) <= 8
                for x in t['held']: text(x, 64)
            else: assert False
        sc = t['scene']; keys(sc, ['id', 'label', 'place', 'atmosphere', 'visit'], ['sight', 'sound', 'props', 'floor', 'ceiling', 'exterior', 'things'])
        ident(sc['id']); text(sc['label'], 48); uint(sc['visit'])
        assert sc['place'] in ['workplace', 'social', 'private', 'other', 'hazard'] and sc['atmosphere'] in ['calm', 'busy', 'tense', 'emergency']
        for k in ['sight', 'sound', 'floor', 'ceiling']:
            if sc.get(k) is not None: text(sc[k], 160)
        if sc.get('exterior') is not None: boolean(sc['exterior'])
        present = sc.get('props', []); assert isinstance(present, list) and len(present) <= 64; seen = []
        for p in present:
            keys(p, ['id', 'note'], ['spot', 'facing', 'coords', 'occludes', 'line_of_sight', 'blocked_by', 'behind', 'dimensions',
                                     'mass', 'state', 'material', 'color', 'texture', 'tags', 'attributes']); ident(p['id']); assert p['id'] not in seen; seen.append(p['id'])
            if p['note'] is not None: text(p['note'], 160)
            if p.get('spot') is not None: ident(p['spot'])
            if p.get('facing') is not None: text(p['facing'], 48)
            if p.get('coords') is not None: coords_val(p['coords'])
            if p.get('occludes') is not None: boolean(p['occludes'])
            if p.get('line_of_sight') is not None: boolean(p['line_of_sight'])
            if p.get('blocked_by') is not None: ident(p['blocked_by'])
            if p.get('behind') is not None: ident(p['behind'])
            if p.get('dimensions') is not None: dimensions_val(p['dimensions'])
            defs.validate_matter(p)
            if p.get('tags') is not None:
                for tg in lst(p['tags'], 0, 32): text(tg, 48)
            if p.get('attributes') is not None:
                att = p['attributes']; assert isinstance(att, dict) and len(att) <= 32
                for k, val in att.items():
                    ident(k)
                    if isinstance(val, str): text(val, 120)
                    else: assert type(val) in (int, float, bool)
        trust = t.get('trust', 0); assert type(trust) is int and -100 <= trust <= 100
        choices = t.get('choices', []); assert isinstance(choices, list) and len(choices) <= 128
        for i, d in enumerate(choices): assert d is not None; decision(d); assert d not in choices[:i]
        labels = t.get('labels', []); assert isinstance(labels, list) and len(labels) <= 32; lids = []
        for l in labels:
            keys(l, ['id', 'aliases']); ident(l['id']); assert l['id'] not in lids; lids.append(l['id']); assert isinstance(l['aliases'], list) and len(l['aliases']) <= 4
            for a in l['aliases']: text(a, 48, 96)
        focus = t.get('focus');
        if focus is not None: ident(focus)
        reason = t.get('reason')
        if reason is not None: text(reason, 240)
        targets = t.get('targets', []); assert isinstance(targets, list) and len(targets) <= 64; tids = []
        for g in targets:
            keys(g, ['kind', 'id', 'names'], ['distance', 'spot', 'facing', 'coords', 'occludes', 'line_of_sight', 'blocked_by', 'behind', 'dimensions',
                                              'mass', 'state', 'material', 'color', 'texture', 'tags', 'attributes',
                                              'features', 'clothing', 'sensory', 'habits', 'posture', 'activity', 'awareness', 'expression', 'held', 'gaze', 'scent'])
            assert g['kind'] in ['character', 'place']; ident(g['id']); assert (g['kind'], g['id']) not in tids; tids.append((g['kind'], g['id']))
            assert isinstance(g['names'], list) and 1 <= len(g['names']) <= 8
            for n in g['names']: text(n, 48); assert norm(n)
            dist_val(g.get('distance'))
            if g.get('spot') is not None: ident(g['spot'])
            if g.get('facing') is not None: text(g['facing'], 48)
            if g.get('coords') is not None: coords_val(g['coords'])
            if g.get('occludes') is not None: boolean(g['occludes'])
            if g.get('line_of_sight') is not None: boolean(g['line_of_sight'])
            if g.get('blocked_by') is not None: ident(g['blocked_by'])
            if g.get('behind') is not None: ident(g['behind'])
            if g.get('dimensions') is not None: dimensions_val(g['dimensions'])
            defs.validate_matter(g)
            if g.get('features') is not None:
                f = g['features']; keys(f, [], ['eyes', 'hair', 'skin', 'scars', 'height', 'build'])
                for k in ['eyes', 'hair', 'skin', 'build']:
                    if k in f and f[k] is not None: text(f[k], 48)
                for sc_item in lst(f.get('scars', []), 0, 8): text(sc_item, 48)
                if 'height' in f and f['height'] is not None:
                    assert (type(f['height']) is int or type(f['height']) is float) and 0.0 < f['height'] <= 100.0
            if g.get('clothing') is not None:
                for item in (g['clothing'] if isinstance(g['clothing'], list) else [g['clothing']]):
                    text(item, 48)
            if g.get('sensory') is not None:
                sn = g['sensory']; keys(sn, [], ['scent', 'voice_tone'])
                for k in ['scent', 'voice_tone']:
                    if k in sn and sn[k] is not None: text(sn[k], 48)
            if g.get('habits') is not None:
                for h in lst(g['habits'], 0, 16): text(h, 80)
            if g.get('tags') is not None:
                for tg in lst(g['tags'], 0, 32): text(tg, 48)
            if g.get('attributes') is not None:
                att = g['attributes']; assert isinstance(att, dict) and len(att) <= 32
                for k, val in att.items():
                    ident(k)
                    if isinstance(val, str): text(val, 120)
                    else: assert type(val) in (int, float, bool)
            for k in ['posture', 'activity', 'awareness', 'expression', 'scent']:
                if g.get(k) is not None: text(g[k], 80)
            if g.get('gaze') is not None: ident(g['gaze'])
            if g.get('held') is not None:
                if isinstance(g['held'], dict):
                    assert len(g['held']) <= 8
                    for k, val in g['held'].items(): text(k, 32); text(val, 64)
                elif isinstance(g['held'], list):
                    assert len(g['held']) <= 8
                    for x in g['held']: text(x, 64)
                else: assert False
            if g.get('line_of_sight') is not None: boolean(g['line_of_sight'])
            if g.get('blocked_by') is not None: ident(g['blocked_by'])
            if g.get('behind') is not None: ident(g['behind'])
            if g.get('dimensions') is not None: dimensions_val(g['dimensions'])
        cdx = self.codex; sh = self.sheet
        place = next((x for x in cdx.get('places', []) if x['id'] == sc['id']), None)
        listed = defs.lst(t.get('entrances', []), 0, 64); eids = []; visible = []
        for x in listed:
            keys(x, ['id', 'status'], ['note']); assert x['id'] not in eids; eids.append(x['id']); assert x['status'] in defs.STATUSES
            if x.get('note') is not None: text(x['note'], 160)
            e = next(e for e in cdx.get('entrances', []) if e['id'] == x['id']); side = next(sd for sd in e['sides'] if sd['place'] == sc['id'])
            other = next((sd for sd in e['sides'] if sd is not side), None)
            dest = None if other is None else next(pl['label'] for pl in cdx['places'] if pl['id'] == other['place'])
            if x['status'] != 'hidden':
                visible.append(dict(d=e, status=x['status'], note=x.get('note'), side=side, other=other, dest=dest, names=[norm(n) for n in [e['label']] + e.get('aliases', [])]))
        needs = t.get('needs', {}); assert isinstance(needs, dict) and len(needs) <= 16
        for k, n in needs.items(): assert any(x['id'] == k for x in sh.get('needs', [])); uint(n, 0, 100)
        quests = t.get('quests', {}); assert isinstance(quests, dict) and len(quests) <= 64
        for k, st in quests.items():
            qd = next(x for x in cdx.get('quests', []) if x['id'] == k); assert isinstance(st, str) and any(x['id'] == st for x in qd['stages'])
        flags = t.get('state', {}); assert isinstance(flags, dict) and len(flags) <= 64
        for k, x in flags.items(): ident(k); assert type(x) is bool or type(x) is int and -1000000000 <= x <= 1000000000 or isinstance(x, str) and len(x) <= 64 and not control(x)
        registry = cfg['features']['props']['registry']['props']
        if self.on('props'):
            present = [dict(prop=next(r for r in registry if r['id'] == p['id']), note=p['note'], line_of_sight=p.get('line_of_sight', True),
                            dimensions=dimensions_val(p.get('dimensions')), material=p.get('material'), color=p.get('color'), texture=p.get('texture')) for p in present]
        else: present = []

        raw = t['text']
        if self.on('chunks'):
            chunks = message_chunks_cases.oracle(dict(op='player', args=dict(text=raw, max_chars=cfg['features']['chunks']['max_chars'])))['chunks']
            line = ''.join(chunks); long = len(chunks) > 1
        else:
            text(raw, 1024, 1024); line = raw; long = False
        q = norm(line); stripped = line.strip(); starred = len(stripped) >= 3 and stripped.startswith('*') and stripped.endswith('*')
        l = self.listener(who); recent = l['recent'] if l else []
        rel = 'guarded' if trust < 0 else 'warm' if trust >= 25 else 'reserved'
        is_exterior = sc.get('exterior') if sc.get('exterior') is not None else (place or {}).get('exterior', False)
        base = dict(name=me['name'], scene=sc['label'], place=sc['place'], atmosphere=sc['atmosphere'], trust=trust, relationship=rel, exterior=is_exterior)
        if t.get('facing') is not None: base['facing'] = t['facing']
        if t.get('spot') is not None: base['spot'] = t['spot']
        feat = t.get('features') if t.get('features') is not None else sh.get('features')
        if feat:
            if feat.get('eyes'): base['eyes'] = feat['eyes']
            if feat.get('hair'): base['hair'] = feat['hair']
            if feat.get('skin'): base['skin'] = feat['skin']
        clothing = t.get('clothing') if t.get('clothing') is not None else sh.get('clothing')
        base['has_clothing'] = bool(clothing)
        if clothing:
            cl_list = [clothing] if isinstance(clothing, str) else [x for x in clothing if isinstance(x, str)]
            if cl_list and (j := self.joined(cl_list)) is not None and len(j) <= 240:
                base['clothing'] = j
        posture = t.get('posture') if t.get('posture') is not None else (sh.get('state') or {}).get('posture')
        base['has_posture'] = bool(posture)
        if posture: base['posture'] = posture
        held = t.get('held') if t.get('held') is not None else (t.get('state') or {}).get('held') if (t.get('state') or {}).get('held') is not None else (sh.get('state') or {}).get('held')
        if held:
            h_list = [held] if isinstance(held, str) else list(held.values()) if isinstance(held, dict) else [x for x in held if isinstance(x, str)]
            if h_list and (j := self.joined(h_list)) is not None and len(j) <= 240:
                base['held'] = j
                base['has_held'] = True
            else:
                base['has_held'] = False
        else:
            base['has_held'] = False
        gaze = t.get('gaze') if t.get('gaze') is not None else (sh.get('state') or {}).get('gaze')
        base['has_gaze'] = bool(gaze)
        if gaze: base['gaze'] = gaze
        activity = t.get('activity') if t.get('activity') is not None else (sh.get('state') or {}).get('activity')
        base['has_activity'] = bool(activity)
        if activity: base['activity'] = activity
        awareness = t.get('awareness') if t.get('awareness') is not None else (sh.get('state') or {}).get('awareness')
        if awareness: base['awareness'] = awareness
        expr = t.get('expression') if t.get('expression') is not None else (sh.get('state') or {}).get('expression')
        if expr: base['expression'] = expr
        scent = t.get('scent') if t.get('scent') is not None else (t.get('sensory') or {}).get('scent') if (t.get('sensory') or {}).get('scent') is not None else (sh.get('sensory') or {}).get('scent')
        if scent: base['scent'] = scent
        for k, x in flags.items(): base['state.' + k] = x
        self.bio_vars(base, t.get('health'), t.get('date'))
        if self.on('goals'):
            urgent = [(needs[n['id']], -i, n) for i, n in enumerate(sh.get('needs', [])) if n['id'] in needs and needs[n['id']] >= n['threshold']]
            base['has_need'] = bool(urgent)
            if urgent: base['need'] = max(urgent, key=lambda u: (u[0], u[1]))[2]['label']
        soon = cfg['features']['events']['soon_turns']
        def timing(e): return ('soon' if e['start'] - at <= soon else 'later') if at < e['start'] else 'now' if at <= e['end'] else 'over'
        def contains(ps): return any(' ' + norm(p) + ' ' in ' ' + q + ' ' for p in ps)
        expressive = None
        def expression():
            nonlocal expressive
            if expressive is None: expressive = expression_cases.oracle(dict(op='route', args=dict(input=line, style='expressive')))
            return expressive
        def reply(route, intent, v, extra=None, greeting=False, kind='reply'):
            got = self.say(intent, v, recent)
            if got is None: return None
            r, s = got; out = dict(route=route, kind=kind, intent=intent, line=r['id'], text=s); out.update(extra or {})
            if kind == 'reply':
                introduced = greeting and r.get('introduces', False)
                if greeting: out['introduced'] = introduced
                if len(self.state['pending']) == 16: self.state['pending'].pop(0)
                self.state['pending'].append(dict(token=token, listener=who, text=s, scene=sc['id'], visit=sc['visit'], at=at, greeting=greeting, introduced=introduced))
            return out
        def authored(route, intent, line_id, texts, extra):
            s = next((x for x in texts if x not in recent), texts[0]); out = dict(route=route, kind='reply', intent=intent, line=line_id, text=s); out.update(extra)
            if len(self.state['pending']) == 16: self.state['pending'].pop(0)
            self.state['pending'].append(dict(token=token, listener=who, text=s, scene=sc['id'], visit=sc['visit'], at=at, greeting=False, introduced=False))
            return out
        def normed(out):
            d = out.get('decision')
            if self.on('scenery') and place is not None and d is not None:
                for n in place.get('norms', []):
                    if any(a['action'] == d['action'] and (a['target'] is None or a['target'] == d['target']) for a in n['actions']):
                        out['norm'] = dict(id=n['id'], reaction=n['reaction']); break
            return out
        def clarify(route, intent, extra=None, v=None):
            got = self.say(intent, base if v is None else v, [])
            out = dict(route=route, kind='clarify', intent=intent, line=got[0]['id'] if got else None, text=got[1] if got else None); out.update(extra or {})
            return out

        if self.on('out_of_character') and expression()['mode'] == 'ooc':
            return dict(route='out_of_character', kind='note', note=expression()['note'])
        if self.on('addressing'):
            act = self.eval_addressing(t, line, me, targets, sh, at)
            if act is not None: return act
        if self.on('content'):
            hit = next((a for a in cfg['features']['content']['avoid'] if contains(a['phrases'])), None)
            if hit is not None: return reply('content', 'content_redirect', base, dict(topic=hit['id'])) or clarify('content', 'content_redirect', dict(topic=hit['id']))
        if self.on('species'):
            sp = t.get('species') or sh.get('species') or {}
            if sp.get('kind') == 'animal' or sp.get('temperament'):
                temp = sp.get('temperament', 'calm')
                v = dict(base, temperament=temp)
                if out := reply('species', 'animal_behavior', v): return out
        key = norm(stripped[1:-1]) if starred else q
        if self.on('stage_actions') and starred and len(stripped.encode()) <= 1024:
            if t.get('line_of_sight') is False:
                return dict(route='stage_action', kind='silent', reason='occluded')
            s = roleplay_actions_cases.oracle(dict(registry=cfg['features']['stage_actions']['actions'], input=stripped, choices=choices))
            if s['status'] == 'proposed': return normed(dict(route='stage_action', kind='action', id=s['id'], decision=s['decision'], narration=s['narration']))
            if s['status'] == 'blocked': return clarify('stage_action', 'stage_action_blocked', dict(id=s['id']))
        if self.on('props'):
            for p in present:
                i = next((i for i in p['prop']['interactions'] if key in [norm(x) for x in i['phrases']]), None)
                if i is None: continue
                extra = dict(prop=p['prop']['id'], interaction=i['id'])
                if i['decision'] is not None and i['decision'] not in choices: return clarify('props', 'stage_action_blocked', extra)
                return normed(dict(route='props', kind='action', **extra, decision=i['decision'], narration=i['narration']))
        if self.on('entrances'):
            ok, rest = match(self.phrases.get('traverse'), key)
            hits = [e for e in visible if ok and rest and self.bare(rest) in [self.bare(n) for n in e['names']]]
            if len(hits) == 1:
                e = hits[0]; d = e['side'].get('travel'); extra = dict(entrance=e['d']['id'])
                if e['d'].get('passage', 'both') == 'one_way' and e['side'] is not e['d']['sides'][0]: d = None
                if e['status'] != 'open' or d is None or d not in choices:
                    v = dict(base, label=e['d']['label'], status=e['status'])
                    if e['dest'] is not None: v['destination'] = e['dest']
                    return clarify('entrances', 'entrance_blocked', extra, v)
                return normed(dict(route='entrances', kind='action', **extra, decision=d))
        if self.on('services'):
            for x in sh.get('services', []):
                if key in [norm(p) for p in x.get('phrases', [])] and when_ok(x.get('when', {}), base):
                    d = x.get('decision'); extra = dict(service=x['id'])
                    if d is not None and d not in choices: return clarify('services', 'service_unavailable', extra)
                    return normed(dict(route='services', kind='action', **extra, decision=d))
        if self.on('directions') and starred:
            body = norm(stripped[1:-1])
            for verb, kind in [('approach', 'character'), ('enter', 'place')]:
                ok, rest = match(self.phrases.get(verb), body)
                if not ok: continue
                hits = [g['id'] for g in targets if g['kind'] == kind and rest and self.bare(rest) in [self.bare(norm(n)) for n in g['names']]]
                spots = [x['id'] for x in (place or {}).get('spots', []) if verb == 'approach' and self.on('scenery') and rest and self.bare(rest) in [self.bare(norm(n)) for n in [x['label']] + x.get('aliases', [])]]
                if len(hits) + len(spots) == 1:
                    target_id = (hits + spots)[0]
                    d = None
                    if verb == 'enter':
                        d = next((c for c in choices if c.get('action') == 'go' and c.get('target') in [f"room:{target_id}", target_id]), None)
                    elif verb == 'approach':
                        d = next((c for c in choices if c.get('action') == 'approach' and c.get('target') in [f"npc:{target_id}", target_id]), None)
                    return normed(dict(route='direction', kind='direction', verb=verb, target=target_id, **(dict(decision=d) if d else {}), **(dict(spot=True) if spots else {})))
                break
            return clarify('direction', 'direction_unsupported')
        if self.on('reactions') and not long:
            if match(self.phrases.get('reaction_debt'), q)[0]:
                loan = (l or {}).get('loan') or t.get('loan') or (sh.get('state') or {}).get('loan')
                v = dict(base)
                if loan: v['resource'] = loan if isinstance(loan, str) else '1 coin'
                if out := reply('reactions', 'reaction_debt', v): return out
            if match(self.phrases.get('reaction_repay'), q)[0]:
                loan = (l or {}).get('loan') or t.get('loan') or (sh.get('state') or {}).get('loan')
                v = dict(base)
                if loan:
                    v['resource'] = loan if isinstance(loan, str) else '1 coin'
                    if l: l['loan'] = None
                if out := reply('reactions', 'reaction_repay', v): return out
            is_loan = match(self.phrases.get('reaction_loan'), q)[0]
            is_gift = match(self.phrases.get('reaction_gift'), q)[0]
            is_help = match(self.phrases.get('reaction_help'), q)[0]
            if is_loan or is_gift or is_help:
                hostile = any(w in line.lower() for w in ["kill", "hurt", "stab", "or else", "attack you"])
                if hostile:
                    if out := reply('reactions', 'reaction_hostile', base): return out
                elif trust >= 0:
                    res = '1 coin' if ('coin' in q or 'money' in q) else 'support' if is_help else 'item'
                    intent = 'reaction_loan' if is_loan else 'reaction_gift' if is_gift else 'reaction_help'
                    if out := reply('reactions', intent, dict(base, resource=res)): return out
                else:
                    if out := reply('reactions', 'reaction_refuse', base): return out
        def ambience(pl, k):
            return next((a[k] for a in (pl or {}).get('ambience', []) if k in a and when_ok(a.get('when', {}), base)), None)
        asks = ([('perception', i) for i in ['ask_location', 'ask_sight', 'ask_hearing', 'ask_name']] if self.on('perception') else []) + \
            ([('entrances', 'ask_exits')] if self.on('entrances') and cdx.get('entrances') else []) + \
            ([('scenery', i) for i in ['ask_smell', 'ask_light', 'ask_temperature', 'ask_floor', 'ask_ceiling']] if self.on('scenery') and place is not None else []) + \
            ([('persona', i) for i in ['ask_role', 'ask_self', 'ask_clothing', 'ask_appearance', 'ask_activity', 'ask_posture', 'ask_held']] if self.on('persona') else [])
        if asks and not long:
            sound = sc.get('sound') if sc.get('sound') is not None else ambience(place, 'sound') if self.on('scenery') else None
            floor = sc.get('floor') if sc.get('floor') is not None else ambience(place, 'floor') if self.on('scenery') else None
            ceiling = sc.get('ceiling') if sc.get('ceiling') is not None else ambience(place, 'ceiling') if self.on('scenery') else None
            visible_props = [p for p in present if p.get('line_of_sight') is not False]
            v = dict(base, has_sight=sc.get('sight') is not None, has_sound=sound is not None, has_props=bool(visible_props),
                     has_floor=floor is not None, has_ceiling=ceiling is not None)
            if sc.get('sight') is not None: v['sight'] = sc['sight']
            if sound is not None: v['sound'] = sound
            if floor is not None: v['floor'] = floor
            if ceiling is not None: v['ceiling'] = ceiling
            if visible_props and (j := self.joined([p['prop']['label'] for p in visible_props])) is not None and len(j) <= 240: v['props'] = j
            if self.on('scenery'):
                for k in ['smell', 'light', 'temperature']:
                    if (a := ambience(place, k)) is not None: v[k] = a
            if self.on('entrances'):
                if visible and (j := self.joined([e['d']['label'] for e in visible])) is not None and len(j) <= 240: v['exits'] = j
                for e in visible:
                    mode = e['d'].get('carries', {}).get('sound', 'when_open')
                    if e['other'] is None or not (mode == 'always' or mode == 'when_open' and e['status'] == 'open'): continue
                    heard = ambience(next(pl for pl in cdx['places'] if pl['id'] == e['other']['place']), 'sound')
                    if heard is not None: v.update(through=e['d']['label'], heard=heard); break
            if self.on('persona'):
                for k in ['role', 'summary']:
                    if k in sh.get('persona', {}): v[k] = sh['persona'][k]
            for route_name, intent in asks:
                if match(self.phrases.get(intent), q)[0] and (out := reply(route_name, intent, v)): return out
        if not long and self.on('places'):
            if match(self.phrases.get('ask_nearby'), q)[0]:
                import places_reference
                try:
                    res_nb = places_reference.nearby(cdx, {'from': sc['id']})
                    nb_labels = [p['label'] for p in res_nb.get('routes', []) + res_nb.get('around', []) if p.get('place') != sc['id']]
                except Exception: nb_labels = []
                v = dict(base)
                if nb_labels and (j := self.joined(nb_labels)): v['nearby'] = j
                if out := reply('places', 'ask_nearby', v): return out
            ok, rest = match(self.phrases.get('ask_route'), q)
            if ok and rest:
                want = self.bare(rest)
                target_p = next((p for p in cdx.get('places', []) if want in [self.bare(norm(n)) for n in [p['label']] + p.get('aliases', [])]), None)
                if target_p:
                    dest_label = target_p['label']
                    ent = next((e['d']['label'] for e in visible if e.get('dest') == dest_label), None)
                    v = dict(base, destination=dest_label)
                    if ent: v['entrance'] = ent
                    if out := reply('places', 'ask_route', v): return out
        if not long and self.on('things'):
            if match(self.phrases.get('ask_things'), q)[0]:
                th = sc.get('things') or t.get('things') or []
                labels = [x['label'] if isinstance(x, dict) else str(x) for x in th]
                v = dict(base)
                if labels and (j := self.joined(labels)): v['things'] = j
                if out := reply('things', 'ask_things', v): return out
        if self.on('bio') and not long and sh.get('bio') is not None:
            for intent in ['ask_age', 'ask_birthday', 'ask_profession', 'ask_skills', 'ask_health', 'ask_family']:
                if match(self.phrases.get(intent), q)[0] and (out := reply('bio', intent, base)): return out
            ok, rest = match(self.phrases.get('ask_skill'), q)
            if ok and rest and (k := self.bio_skill(self.bare(rest))) is not None:
                if out := reply('bio', 'ask_skill', dict(base, skill=k['label'], skill_level=k['level']), dict(skill=k['id'])): return out
            ok, rest = match(self.phrases.get('ask_knowledge'), q)
            if ok and rest and (k := self.bio_knowledge(self.bare(rest), base)) is not None:
                if out := reply('bio', 'ask_knowledge', dict(base, knowledge=k['summary'], knowledge_topic=k['topic'], knowledge_level=k['level']), dict(knowledge=k['id'])): return out
        if not long and (self.on('props') or self.on('entrances') or self.on('scenery')):
            ok, rest = match(self.phrases.get('describe_prop'), q)
            if ok and rest:
                hits = [p for p in present if p.get('line_of_sight') is not False and self.bare(rest) in [self.bare(norm(n)) for n in [p['prop']['label']] + p['prop']['aliases']]]
                if len(hits) == 1:
                    p = hits[0]; v = dict(base, prop=p['prop']['label'], description=p['prop']['description'], has_note=p['note'] is not None)
                    if p['note'] is not None: v['note'] = p['note']
                    mat = p.get('material') or p['prop'].get('material')
                    if mat: v['material'] = mat
                    col = p.get('color') or p['prop'].get('color')
                    if col: v['color'] = col
                    tex = p.get('texture') or p['prop'].get('texture')
                    if tex: v['texture'] = tex
                    v['has_material'] = mat is not None
                    v['has_color'] = col is not None
                    v['has_texture'] = tex is not None
                    v['has_matter'] = mat is not None or col is not None or tex is not None
                    if out := reply('props', 'describe_prop', v, dict(prop=p['prop']['id'])): return out
                hits = [e for e in visible if self.on('entrances') and self.bare(rest) in [self.bare(n) for n in e['names']]]
                if len(hits) == 1:
                    e = hits[0]; v = dict(base, label=e['d']['label'], description=e['side']['description'], status=e['status'], has_note=e['note'] is not None)
                    if e['note'] is not None: v['note'] = e['note']
                    if e['dest'] is not None: v['destination'] = e['dest']
                    if out := reply('entrances', 'describe_entrance', v, dict(entrance=e['d']['id'])): return out
                hits = [x for x in (place or {}).get('spots', []) if self.on('scenery') and self.bare(rest) in [self.bare(norm(n)) for n in [x['label']] + x.get('aliases', [])]]
                if len(hits) == 1:
                    v = dict(base, label=hits[0]['label'], description=hits[0]['description'])
                    for k in ['material', 'color', 'texture']:
                        if hits[0].get(k): v[k] = hits[0][k]
                    if out := reply('scenery', 'describe_spot', v, dict(spot=hits[0]['id'])): return out
            ok, rest = match(self.phrases.get('ask_matter'), q)
            if not (ok and rest):
                for pfx in ['what is ', 'what s ', 'what are ']:
                    if q.startswith(pfx):
                        for sfx in [' made of', ' made from']:
                            if q.endswith(sfx) and len(q) > len(pfx) + len(sfx):
                                rest = q[len(pfx):len(q) - len(sfx)].strip()
                                ok = bool(rest)
                                break
                        if ok and rest: break
            if ok and rest:
                hits = [p for p in present if p.get('line_of_sight') is not False and self.bare(rest) in [self.bare(norm(n)) for n in [p['prop']['label']] + p['prop']['aliases']]]
                if len(hits) == 1:
                    p = hits[0]; v = dict(base, prop=p['prop']['label'])
                    mat = p.get('material') or p['prop'].get('material')
                    if mat: v['material'] = mat
                    col = p.get('color') or p['prop'].get('color')
                    if col: v['color'] = col
                    tex = p.get('texture') or p['prop'].get('texture')
                    if tex: v['texture'] = tex
                    v['has_material'] = mat is not None
                    v['has_color'] = col is not None
                    v['has_texture'] = tex is not None
                    v['has_matter'] = mat is not None or col is not None or tex is not None
                    if out := reply('props', 'ask_matter', v, dict(prop=p['prop']['id'])): return out
                if self.on('things'):
                    th_list = sc.get('things') or t.get('things') or []
                    th_hits = [x for x in th_list if isinstance(x, dict) and self.bare(rest) in [self.bare(norm(n)) for n in [x.get('label', '')] + x.get('aliases', [])]]
                    if len(th_hits) == 1:
                        x = th_hits[0]; v = dict(base, prop=x.get('label', ''))
                        mat = x.get('material')
                        if mat: v['material'] = mat; v['has_material'] = True; v['has_matter'] = True
                        if out := reply('things', 'ask_matter', v, dict(thing=x.get('id', ''))): return out
        if self.on('commands') and not long:
            # A null focus never matches a null target (the Rust grammar checks focus only for targeted pairs).
            r = overnight_input_cases.resolve(line, choices, {l['id']: l['aliases'] for l in labels}, focus if focus is not None else '\0none')
            if r['status'] == 'resolved' and r['decision']['action'] != 'talk': return normed(dict(route='command', kind='action', decision=r['decision']))
            if r['status'] == 'clarify': return clarify('command', 'command_clarify')
        if self.on('disclosures') and len(line.encode()) <= 2048:
            c = expression_cases.claim(line)
            if c is not None and not any(x in line.rstrip('.!?') for x in '.!?'):
                v = dict(base, slot=c['slot'], value=c['value'], corrects=c['corrects'] is not None)
                if c['corrects'] is not None: v['old'] = c['corrects']
                if c['polarity'] is not None: v['polarity'] = c['polarity']
                if out := reply('disclosure', 'disclosure', v):
                    l = self.listener(who, True)
                    if c['slot'] == 'name': l['name'] = c['value']
                    else:
                        l['preferences'] = [dict(value=c['value'], polarity=c['polarity'])] + [p for p in l['preferences'] if p['value'].lower() != c['value'].lower()]
                        del l['preferences'][cfg['features']['disclosures']['max_preferences']:]
                    return out
        if self.on('recall'):
            l = self.listener(who)
            if match(self.phrases.get('recall_name'), q)[0]:
                v = dict(base, known=bool(l and l['name']))
                if v['known']: v['value'] = l['name']
                if out := reply('recall', 'recall_name', v): return out
            if match(self.phrases.get('recall_preference'), q)[0]:
                v = dict(base, known=bool(l and l['preferences']))
                if v['known']: v.update(value=l['preferences'][0]['value'], polarity=l['preferences'][0]['polarity'])
                if out := reply('recall', 'recall_preference', v): return out
        if self.on('secrets'):
            for x in sh.get('secrets', []):
                if not contains(x['phrases']): continue
                extra = dict(secret=x['id'])
                if when_ok(x['when'], base):
                    out = authored('secrets', 'reveal', 'secret:' + x['id'], [x['reveal']], extra); l = self.listener(who, True)
                    if x['id'] not in l['revealed']: l['revealed'].append(x['id'])
                    return out
                if x.get('cover') is not None: return authored('secrets', 'cover', 'cover:' + x['id'], [x['cover']], extra)
                return reply('secrets', 'secret_withheld', base, extra) or dict(route='secrets', kind='silent', secret=x['id'])
        if self.on('opinions'):
            for x in sh.get('opinions', []):
                if contains(x['phrases']) and when_ok(x.get('when', {}), base): return authored('opinions', 'opinion', 'opinion:' + x['id'], x['variants'], dict(opinion=x['id']))
        if self.on('viewpoints') and not long:
            topic = 'dreams' if any(w in q for w in ['dream', 'hope', 'aspiration']) else 'hobbies' if any(w in q for w in ['hobby', 'hobbies', 'for fun', 'spare time']) else None
            if topic:
                views = sh.get('viewpoints', []) + t.get('viewpoints', [])
                vp = next((x for x in views if x.get('topic') == topic), None)
                if vp:
                    return authored('viewpoints', 'opinion', 'viewpoint:' + topic, [vp['stance']], dict(viewpoint=topic))
        if not long and any(self.on(f) for f in ['relationships', 'factions', 'events', 'quests']):
            ok, rest = match(self.phrases.get('ask_about'), q)
            if ok and rest:
                want = self.bare(rest)
                def hit(items, label='label'):
                    hs = [x for x in items if want in [self.bare(norm(n)) for n in [x[label]] + x.get('aliases', [])]]
                    return hs[0] if len(hs) == 1 else None
                if self.on('relationships') and (r := hit(sh.get('relationships', []), 'name')):
                    v = dict(base, person=r['name'], kind=r['kind'])
                    if 'description' in r: v['text'] = r['description']
                    if out := reply('relationships', 'person', v, dict(person=r['with'])): return out
                if self.on('factions') and (f := hit(cdx.get('factions', []))):
                    if out := reply('factions', 'faction', dict(base, faction=f['label'], text=f['description'], stance=self.stance(f['id'])), dict(faction=f['id'])): return out
                if self.on('events') and (e := hit(cdx.get('events', []))):
                    if out := reply('events', 'event', dict(base, event=e['label'], text=e['description'], timing=timing(e)), dict(event=e['id'])): return out
                if self.on('quests') and (qd := hit([x for x in cdx.get('quests', []) if me['id'] in x.get('givers', []) and x['id'] in quests])):
                    st = next(x for x in qd['stages'] if x['id'] == quests[qd['id']])
                    if out := reply('quests', 'quest', dict(base, quest=qd['label'], text=st['description'], stage=st['id']), dict(quest=qd['id'])): return out
        if self.on('recipes') and not long:
            ok, rest = match(self.phrases.get('ask_recipe'), q)
            if ok and rest:
                want = self.bare(rest)
                all_recs = sh.get('recipes', []) + cdx.get('recipes', [])
                rec = next((r for r in all_recs if want in [self.bare(norm(n)) for n in [r.get('label', '')] + r.get('aliases', [])]), None)
                v = dict(base)
                if rec:
                    v['recipe'] = rec.get('label', rest)
                    in_labels = [x.get('label') or x.get('kind') or x.get('tag') or str(x) if isinstance(x, dict) else str(x) for x in rec.get('inputs', [])]
                    v['inputs'] = self.joined(in_labels) or 'materials'
                    if out := reply('recipes', 'ask_recipe', v, dict(recipe=rec.get('id', ''))): return out
                else:
                    v['recipe'] = rest
                    if out := reply('recipes', 'ask_recipe', v): return out
        if self.on('communities') and not long:
            if match(self.phrases.get('ask_trade'), q)[0]:
                comms = cdx.get('communities', [])
                v = dict(base)
                if comms or sh.get('factions') or any(c.get('action') in ['trade', 'buy'] for c in choices):
                    comm = comms[0] if comms else dict(label='market', currency='coins')
                    v.update(community=comm.get('label', 'here'), currency=comm.get('currency', 'coins'))
                    if out := reply('communities', 'ask_trade', v): return out
                else:
                    if out := reply('communities', 'ask_trade', base): return out
        if not long:
            if self.on('services') and sh.get('services') and match(self.phrases.get('ask_services'), q)[0]:
                v = dict(base); avail = [x['label'] for x in sh['services'] if when_ok(x.get('when', {}), base)]
                if avail and (j := self.joined(avail)) is not None and len(j) <= 240: v['services'] = j
                if out := reply('services', 'ask_services', v): return out
            if self.on('goals') and sh.get('goals') and match(self.phrases.get('ask_goals'), q)[0]:
                cands = [g for g in sh['goals'] if when_ok(g.get('when', {}), base)]
                if cands:
                    g = max(cands, key=lambda g: g['priority']); return authored('goals', 'goal', 'goal:' + g['id'], [g['description']], dict(goal=g['id']))
                if out := reply('goals', 'no_goal', base): return out
            if self.on('timeline') and match(self.phrases.get('ask_news'), q)[0]:
                tl = cdx.get('timeline', []) or t.get('timeline', [])
                known = [e for e in tl if e.get('visibility') == 'public' or me['id'] in e.get('actors', []) or me['id'] in e.get('witnesses', []) or me['id'] in e.get('known_to', [])]
                if known:
                    recent_e = max(known, key=lambda e: (e.get('at', 0), e.get('seq', 0)))
                    if out := reply('timeline', 'ask_news', dict(base, news=recent_e['summary'], at=recent_e.get('at', 0)), dict(news=recent_e.get('kind', 'news'))): return out
                elif not self.on('events') or not cdx.get('events'):
                    if out := reply('timeline', 'ask_news', base): return out
            if self.on('events') and cdx.get('events') and (match(self.phrases.get('ask_events'), q)[0] or match(self.phrases.get('ask_news'), q)[0]):
                v = dict(base); live = [e['label'] for e in cdx['events'] if timing(e) in ['now', 'soon']]
                if live and (j := self.joined(live)) is not None and len(j) <= 240: v['events'] = j
                if out := reply('events', 'ask_events', v): return out
        previous = (self.listener(who) or {}).get('topic')
        if long: route = dict(kind='other', topic=None)
        else:
            said = line
            for opener in ['hello', 'hi', 'hey', 'greetings']:
                if q in [opener + ' ' + w for w in [norm(me['name']), norm(me['id']), 'the ' + norm(me['id'])]]: said = opener
            route = social_routing_cases.route(dict(input=said, previous=previous))
        if route['topic'] is not None or previous is not None: self.listener(who, True)['topic'] = route['topic']
        kind = route['kind']
        if kind == 'greeting':
            if self.on('greetings'):
                res = greeting_memory_cases.oracle(dict(config=self.gcfg, state=self.state['greetings'], ops=[dict(op='plan', args=dict(actor=me['id'], listener=who, scene=sc['id'], visit=sc['visit'], at=at, trust=trust, atmosphere=sc['atmosphere'], place=sc['place']))]))
                plan = res['results'][0]
                if out := reply('greeting', 'greeting', dict(base, meeting=plan['meeting'], needs_introduction=plan['needs_introduction']), greeting=True): return out
            if self.on('social') and (out := reply('social', 'social_greeting', base, greeting=True)): return out
        if self.on('social'):
            if kind in ['thanks', 'farewell', 'wellbeing'] and (out := reply('social', kind, base)): return out
            if kind == 'elaborate':
                intent = 'clarify_topic' if previous is None else 'mood_reason' if previous == 'wellbeing' else 'elaborate'
                v = dict(base, has_reason=reason is not None)
                if reason is not None: v['reason'] = reason
                if out := reply('social', intent, v): return out
        if self.on('model'):
            req = self.compile(line, who, base, sc, present, visible, place, timing, ambience, recent, t.get('health'))
            if len(self.state['asked']) == 16: self.state['asked'].pop(0)
            self.state['asked'].append(dict(token=token, listener=who, scene=sc['id'], visit=sc['visit'], at=at))
            return dict(route='model', kind='model', input=line, long=long, expression=expression(), topic=route['topic'], request=req)
        if self.on('social') and (out := reply('fallback', 'unknown', base)): return out
        return dict(route='none', kind='silent')

    def compile(self, line, who, base, sc, present, visible, place, timing, ambience, recent, health=None):
        """Expectation 10: rules, brief, situation, player words; locked secrets never appear."""
        cfg = self.cfg; m = cfg['features']['model']; sh = self.sheet; cdx = self.codex; me = cfg['character']
        v = dict(base, sentences=m['max_sentences'])
        rules = [x for t in self.rules if (x := render(t, v)) is not None and x]
        brief = dict(name=me['name'])
        if self.on('persona'):
            per = sh.get('persona', {})
            for k in ['pronouns', 'role', 'summary', 'appearance', 'values', 'fears', 'desires']:
                if per.get(k) not in (None, []): brief[k] = per[k]
            sp = {k: sh['speech'][k] for k in ['formality', 'verbosity', 'quirks', 'catchphrases', 'avoid_words'] if sh.get('speech', {}).get(k) not in (None, [])}
            if sp: brief['speech'] = sp
            if sh.get('features'): brief['features'] = dict(sh['features'])
            if sh.get('clothing'): brief['clothing'] = list(sh['clothing'])
            if sh.get('state'): brief['state'] = dict(sh['state'])
        if self.on('bio') and (b := self.bio_brief(health, base)) is not None: brief['bio'] = b
        if self.on('species') and sh.get('species'): brief['species'] = dict(sh['species'])
        if self.on('relationships') and sh.get('relationships'): brief['relationships'] = [dict(name=r['name'], kind=r['kind']) for r in sh['relationships']]
        if self.on('factions') and sh.get('factions'):
            brief['factions'] = []
            for f in sh['factions']:
                x = dict(name=next(d['label'] for d in cdx['factions'] if d['id'] == f['faction']))
                if f.get('role') is not None: x['role'] = f['role']
                brief['factions'].append(x)
        if self.on('content'): brief['content'] = dict(rating=cfg['features']['content']['rating'])
        scene = dict(name=sc['label'], atmosphere=sc['atmosphere'])
        if sc.get('sight') is not None: scene['sight'] = sc['sight']
        sound = sc.get('sound') if sc.get('sound') is not None else ambience(place, 'sound') if self.on('scenery') else None
        if sound is not None: scene['sound'] = sound
        if self.on('scenery'):
            for k in ['smell', 'light', 'temperature']:
                if (a := ambience(place, k)) is not None: scene[k] = a
        floor = sc.get('floor') if sc.get('floor') is not None else ambience(place, 'floor') if self.on('scenery') else None
        if floor is not None: scene['floor'] = floor
        ceiling = sc.get('ceiling') if sc.get('ceiling') is not None else ambience(place, 'ceiling') if self.on('scenery') else None
        if ceiling is not None: scene['ceiling'] = ceiling
        if base.get('exterior'): scene['exterior'] = True
        visible_props = [p for p in present if p.get('line_of_sight') is not False]
        if visible_props: scene['props'] = [p['prop']['label'] for p in visible_props]
        if self.on('entrances') and visible: scene['exits'] = [e['d']['label'] for e in visible]
        l = self.listener(who); listener = dict(relationship=base['relationship'])
        if l and l['name']: listener['name'] = l['name']
        if l and l['preferences']: listener['preferences'] = [dict(p) for p in l['preferences']]
        if self.on('secrets') and l and l['revealed']:
            told = [x['reveal'] for x in sh.get('secrets', []) if x['id'] in l['revealed']]
            if told: listener['already_told'] = told
        sit = dict(scene=scene, listener=listener)
        if self.on('goals'):
            cands = [g for g in sh.get('goals', []) if when_ok(g.get('when', {}), base)]
            if cands: sit['goal'] = max(cands, key=lambda g: g['priority'])['description']
            if base.get('has_need'): sit['need'] = base['need']
        if self.on('bio') and base.get('is_birthday') is True: sit['birthday'] = True
        if self.on('events'):
            ev = [dict(name=e['label'], timing=timing(e)) for e in cdx.get('events', []) if timing(e) in ['now', 'soon']]
            if ev: sit['events'] = ev
        if recent: sit['recent_replies'] = list(recent)
        drops = [(sit, 'recent_replies'), (brief, 'appearance'), (brief, 'desires'), (brief, 'fears'), (brief, 'bio'), (brief, 'factions'), (brief, 'relationships'), (sit, 'events'),
                 (brief, 'speech'), (brief, 'clothing'), (brief, 'features'), (brief, 'state'), (sit, 'need'), (sit, 'goal'), (brief, 'values'), (brief, 'summary'), (listener, 'already_told'), (listener, 'preferences'), (scene, 'exits'), (scene, 'props')]
        for d, k in drops:
            if len(enc(brief)) + len(enc(sit)) <= m['brief_chars']: break
            d.pop(k, None)
        msgs = ([dict(role='system', content=' '.join(rules))] if rules else []) + [dict(role='system', content=enc(brief)), dict(role='system', content=enc(sit)), dict(role='user', content=line)]
        schema = dict(type='object', properties=dict(speech=dict(type='string', minLength=1, maxLength=m['max_chars'])), required=['speech'], additionalProperties=False)
        return dict(messages=msgs, response_format=dict(type='json_schema', json_schema=dict(name='speech', strict=True, schema=schema)), max_tokens=m['max_tokens'])

    def reply(self, a):
        """Expectation 12: validate untrusted model text against the open request `token`."""
        keys(a, ['token', 'listener', 'text']); uint(a['token'], 1); ident(a['listener']); assert isinstance(a['text'], str)
        tk = next(x for x in self.state['asked'] if x['token'] == a['token']); assert tk['listener'] == a['listener']
        reason, speech = check_speech(a['text'], self, a['listener'])
        if reason is not None: return dict(route='model', kind='rejected', reason=reason)
        self.state['asked'].remove(tk)
        if len(self.state['pending']) == 16: self.state['pending'].pop(0)
        self.state['pending'].append(dict(token=tk['token'], listener=tk['listener'], text=speech, scene=tk['scene'], visit=tk['visit'], at=tk['at'], greeting=False, introduced=False))
        return dict(route='model', kind='reply', intent='model', line=None, text=speech)

    def stance(self, fid):
        member = [m['faction'] for m in self.sheet.get('factions', [])]
        if fid in member: return 'member'
        for m in member:
            f = next(f for f in self.codex['factions'] if f['id'] == m)
            r = next((r for r in f.get('relations', []) if r['faction'] == fid), None)
            if r is not None: return r['stance']
        return 'neutral'

    def delivered(self, d):
        keys(d, ['token', 'listener', 'text', 'heard']); uint(d['token'], 1); ident(d['listener']); text(d['text'], 240, 1024); uint(d['heard'], 0, len(d['text']))
        p = next((p for p in self.state['pending'] if p['token'] == d['token']), None)
        if p is None: return dict(recorded=False, reason='unknown')
        assert p['listener'] == d['listener'] and p['text'] == d['text']; self.state['pending'].remove(p)
        if d['heard'] != len(d['text']): return dict(recorded=False, reason='partial')
        n = self.cfg['variety']['recent']
        if n:
            l = self.listener(d['listener'], True); l['recent'] = (l['recent'] + [d['text']])[-n:]
        greeting = None
        if self.on('greetings'):
            me = self.cfg['character']['id']; e = next((e for e in self.state['greetings']['entries'] if e['actor'] == me and e['listener'] == d['listener']), None)
            receipt = dict(actor=me, listener=d['listener'], scene=p['scene'], visit=p['visit'], at=p['at'], token=p['token'], greeting=p['greeting'], introduced=p['introduced'], text=d['text'], heard=d['heard'], expected=None if e is None else e['token'])
            res = greeting_memory_cases.oracle(dict(config=self.gcfg, state=self.state['greetings'], ops=[dict(op='confirm', args=receipt)]))
            self.state['greetings'] = res['state']; greeting = res['results'][0]
        return dict(recorded=True, greeting=greeting)


def oracle(a):
    keys(a, ['config', 'state', 'ops']); e = Engine(a['config'], a['state']); assert isinstance(a['ops'], list) and len(a['ops']) <= 64; out = []
    for op in a['ops']:
        keys(op, ['op', 'args']); assert op['op'] in ['turn', 'delivered', 'reply']
        out.append(e.turn(op['args']) if op['op'] == 'turn' else e.delivered(op['args']) if op['op'] == 'delivered' else e.reply(op['args']))
    return dict(config=e.cfg, results=out, state=e.state)
