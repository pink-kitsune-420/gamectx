#!/usr/bin/env python3
"""Offline integration-kit reference. No network calls or provider credentials."""
import copy
import json
import math
import pathlib
import random
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'eval'))

def encode(v):
    return json.dumps(v, ensure_ascii=False, separators=(',', ':'), allow_nan=False)

def need(ok):
    if not ok:
        raise ValueError('invalid kit value')

def fields(v, keys):
    need(isinstance(v, dict) and set(v) == set(keys))

def uint(v, high=4294967295):
    need(type(v) is int and 0 <= v <= high)
    return v

def name(v):
    need(isinstance(v, str) and 0 < len(v) <= 32 and 'a' <= v[0] <= 'z'
         and all(c in 'abcdefghijklmnopqrstuvwxyz_0123456789' for c in v))
    return v

def prose(v, limit=280):
    need(isinstance(v, str) and 0 < len(v) <= limit and all(ord(c) >= 32 and not 127 <= ord(c) <= 159 for c in v))

def scalar(v, kind, nullable=False):
    if v is None:
        return nullable or kind == 'null'
    return {'string': isinstance(v, str), 'number': type(v) in (int, float) and math.isfinite(v),
            'integer': type(v) in (int, float) and int(v) == v if type(v) in (int, float) else False,
            'boolean': type(v) is bool, 'object': isinstance(v, dict), 'array': isinstance(v, list)}.get(kind, False)

def equivalent(a, b):
    if type(a) is bool or type(b) is bool:
        return type(a) is type(b) and a == b
    if isinstance(a, dict) and isinstance(b, dict):
        return set(a) == set(b) and all(equivalent(a[k], b[k]) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(equivalent(x,y) for x,y in zip(a,b))
    return a == b

COMMON = {'type', 'description', 'enum'}
def schema(s, depth=0, count=None):
    if count is None:
        count = [0]
    count[0] += 1
    need(depth <= 8 and count[0] <= 256 and isinstance(s, dict))
    t = s.get('type')
    if isinstance(t, list):
        need(len(t) == 2 and t[1] == 'null' and t[0] != 'null')
        t = t[0]
    need(t in ('string', 'number', 'integer', 'boolean', 'null', 'object', 'array'))
    if 'description' in s:
        prose(s['description'], 1024)
    allowed = set(COMMON)
    if t == 'object':
        allowed |= {'properties', 'required', 'additionalProperties'}
        need(s.get('additionalProperties') is False and isinstance(s.get('properties'), dict)
             and len(s['properties']) <= 32 and isinstance(s.get('required'), list))
        need(len(set(s['required'])) == len(s['required']) and all(k in s['properties'] for k in s['required']))
        for k, spec in s['properties'].items():
            name(k)
            schema(spec, depth + 1, count)
    if t == 'array':
        allowed |= {'items', 'minItems', 'maxItems'}
        uint(s.get('maxItems'), 128)
        uint(s.get('minItems', 0), s['maxItems'])
        schema(s.get('items'), depth + 1, count)
    if t == 'string':
        allowed |= {'minLength', 'maxLength'}
        uint(s.get('maxLength'), 4096)
        uint(s.get('minLength', 0), s['maxLength'])
    if t in ('integer', 'number'):
        allowed |= {'minimum', 'maximum'}
        for k in ('minimum', 'maximum'):
            if k in s:
                need(type(s[k]) in (int, float))
        need(s.get('minimum', -float('inf')) <= s.get('maximum', float('inf')))
    need(set(s) <= allowed)
    if 'enum' in s:
        need(isinstance(s['enum'], list) and 0 < len(s['enum']) <= 32)
        without = {k: v for k, v in s.items() if k != 'enum'}
        for i, v in enumerate(s['enum']):
            need(not any(equivalent(v, old) for old in s['enum'][:i]))
            validate(without, v)

def validate(s, v):
    t = s['type']
    nullable = isinstance(t, list)
    t = t[0] if nullable else t
    need(scalar(v, t, nullable))
    if 'enum' in s:
        need(any(equivalent(v, e) for e in s['enum']))
    if v is None:
        return
    if t == 'object':
        need(set(v) <= set(s['properties']) and set(s['required']) <= set(v))
        for k, value in v.items():
            validate(s['properties'][k], value)
    elif t == 'array':
        need(s.get('minItems', 0) <= len(v) <= s['maxItems'])
        for value in v:
            validate(s['items'], value)
    elif t == 'string':
        need(s.get('minLength', 0) <= len(v) <= s['maxLength'])
    elif t in ('integer', 'number'):
        need(s.get('minimum', -float('inf')) <= v <= s.get('maximum', float('inf')))

def definition(p):
    fields(p, ['version', 'datasets', 'tools', 'recipes'])
    need(type(p['version']) is int and p['version'] == 1 and len(encode(p).encode()) <= 262144)
    for key, cap in [('datasets', 16), ('tools', 32), ('recipes', 32)]:
        need(isinstance(p[key], list) and len(p[key]) <= cap)
        names = [name(x['name']) for x in p[key]]
        need(len(set(names)) == len(names))
    for d in p['datasets']:
        fields(d, ['name', 'description', 'id', 'max_age', 'fields'])
        prose(d['description'])
        if d['max_age'] is not None:
            uint(d['max_age'])
        need(0 < len(d['fields']) <= 64)
        names = [name(f['name']) for f in d['fields']]
        need(len(set(names)) == len(names))
        for f in d['fields']:
            fields(f, ['name', 'type', 'description', 'unit', 'nullable', 'visibility'])
            prose(f['description'])
            if f['unit'] is not None:
                prose(f['unit'], 32)
            need(f['type'] in ('string', 'number', 'integer', 'boolean') and type(f['nullable']) is bool
                 and f['visibility'] in ('model', 'host'))
        need(any(f['name'] == d['id'] and f['type'] == 'string' and not f['nullable'] and f['visibility'] == 'model' for f in d['fields']))
        need(d['name'] not in ('schemas', 'tools'))
    for t in p['tools']:
        fields(t, ['name', 'description', 'parameters'])
        prose(t['description'], 1024)
        schema(t['parameters'])
        need(t['parameters']['type'] == 'object')
    for r in p['recipes']:
        fields(r, ['name', 'budget_bytes', 'datasets', 'tools', 'sections'])
        uint(r['budget_bytes'], 1048576)
        need(len(r['datasets']) <= 16 and len(r['tools']) <= 32 and len(r['sections']) <= 32)
        need(len(set(r['tools'])) == len(r['tools']) and all(any(t['name'] == n for t in p['tools']) for n in r['tools']))
        names = ['schemas', 'tools']
        for x in r['datasets']:
            fields(x, ['name', 'mode', 'view', 'priority', 'required'])
            d = next(d for d in p['datasets'] if d['name'] == x['name'])
            need(x['mode'] in ('rows', 'schema') and type(x['required']) is bool and type(x['priority']) is int and abs(x['priority']) <= 1000000)
            v = x['view']
            need(isinstance(v, dict) and set(v) <= {'fields', 'where', 'sort_by', 'limit'})
            visible = [f['name'] for f in d['fields'] if f['visibility'] == 'model']
            if 'fields' in v:
                need(isinstance(v['fields'], list) and len(set(v['fields'])) == len(v['fields']) and set(v['fields']) <= set(visible) and len(v['fields']) > 0)
            if 'where' in v:
                need(isinstance(v['where'], dict) and set(v['where']) <= set(visible))
                for c in v['where'].values():
                    if isinstance(c, dict):
                        need(len(c) == 1 and next(iter(c)) in ('=', '!=', '<', '<=', '>', '>=', 'in'))
                        if 'in' in c:
                            need(isinstance(c['in'], list) and len(c['in']) <= 128)
                    else:
                        need(not isinstance(c, list))
            if 'sort_by' in v:
                need(isinstance(v['sort_by'], str) and v['sort_by'].removeprefix('-') in visible)
            if 'limit' in v:
                uint(v['limit'], 4096)
            names.append(x['name'])
        for s in r['sections']:
            fields(s, ['name', 'priority', 'required'])
            name(s['name'])
            need(type(s['priority']) is int and abs(s['priority']) <= 1000000 and type(s['required']) is bool)
            names.append(s['name'])
        need(len(set(names)) == len(names))
    return p

def snapshot(p, s):
    fields(s, ['now', 'tables', 'sections'])
    uint(s['now'])
    need(isinstance(s['tables'], dict) and isinstance(s['sections'], dict) and len(encode(s).encode()) <= 1048576 and len(s['sections']) <= 32)
    for key in s['sections']:
        name(key)
    out = {}
    for n, t in s['tables'].items():
        d = next(d for d in p['datasets'] if d['name'] == n)
        fields(t, ['updated_at', 'rows'])
        uint(t['updated_at'], s['now'])
        need(isinstance(t['rows'], list) and len(t['rows']) <= 4096)
        ids = set()
        rows = []
        for row in t['rows']:
            fields(row, [f['name'] for f in d['fields']])
            for f in d['fields']:
                need(scalar(row[f['name']], f['type'], f['nullable']))
            ident = row[d['id']]
            need(0 < len(ident) <= 256 and ident not in ids)
            ids.add(ident)
            rows.append({f['name']: row[f['name']] for f in d['fields'] if f['visibility'] == 'model'})
        out[n] = {'updated_at': t['updated_at'], 'rows': rows}
    return {'now': s['now'], 'tables': out, 'sections': s['sections']}

def selected(p, s, r):
    out = []
    for x in r['datasets']:
        d = next(d for d in p['datasets'] if d['name'] == x['name'])
        t = s['tables'][d['name']]
        age = s['now'] - t['updated_at']
        need(d['max_age'] is None or age <= d['max_age'])
        indices = list(range(len(t['rows'])))
        v = x['view']
        def match(row):
            for k, c in v.get('where', {}).items():
                op, val = next(iter(c.items())) if isinstance(c, dict) else ('=', c)
                a = row[k]
                if op == 'in':
                    yes = a in val
                elif op == '=':
                    yes = type(a) is type(val) and a == val
                elif op == '!=':
                    yes = not (type(a) is type(val) and a == val)
                elif a is None or val is None:
                    yes = False
                else:
                    yes = {'<': lambda: a < val, '<=': lambda: a <= val, '>': lambda: a > val, '>=': lambda: a >= val}[op]()
                if not yes:
                    return False
            return True
        indices = [i for i in indices if match(t['rows'][i])]
        if 'sort_by' in v:
            k = v['sort_by'].removeprefix('-')
            populated = [i for i in indices if t['rows'][i][k] is not None]
            empty = [i for i in indices if t['rows'][i][k] is None]
            populated.sort(key=lambda i: t['rows'][i][k], reverse=v['sort_by'].startswith('-'))
            indices = populated + empty
        indices = indices[:v.get('limit', len(indices))]
        columns = v.get('fields', [f['name'] for f in d['fields'] if f['visibility'] == 'model'])
        rows = [{c: t['rows'][i][c] for c in columns} for i in indices]
        desc = {'description': d['description'], 'columns': [{k: f[k] for k in ('name', 'type', 'description', 'unit', 'nullable')} for c in columns for f in d['fields'] if f['name'] == c]}
        diag = {'name': d['name'], 'source_rows': len(t['rows']), 'selected_rows': len(rows), 'source_indices': indices,
                'fields': columns, 'omitted_fields': [f['name'] for f in d['fields'] if f['name'] not in columns],
                'updated_at': t['updated_at'], 'age': age, 'mode': x['mode'], 'view': v}
        out.append((x, rows, desc, diag))
    return out

def build(p, s, rname):
    r = next(r for r in p['recipes'] if r['name'] == rname)
    selections = selected(p, s, r)
    sections = []
    if r['tools']:
        tools = [{'type': 'function', 'function': {**next(t for t in p['tools'] if t['name'] == n), 'strict': False}} for n in r['tools']]
        sections.append(('tools', tools, 1000001, True, True))
    if selections:
        sections.append(('schemas', {x['name']: desc for x, rows, desc, diag in selections}, 1000001, True, True))
    for x, rows, desc, diag in selections:
        if x['mode'] == 'rows':
            columns = list(rows[0]) if rows else []
            data = {'columns': columns, 'rows': [[row[c] for c in columns] for row in rows]}
            sections.append((x['name'], data, x['priority'], False, x['required']))
    for x in r['sections']:
        if x['name'] in s['sections']:
            sections.append((x['name'], s['sections'][x['name']], x['priority'], False, x['required']))
        else:
            need(not x['required'])
    kept = list(enumerate(sections))
    dropped = []
    while len(encode({x[0]: x[1] for i, x in kept}).encode()) > r['budget_bytes']:
        optional = [(i, x) for i, x in kept if not x[4]]
        need(optional)
        victim = min(optional, key=lambda ix: (ix[1][2], -ix[0]))
        kept.remove(victim)
        dropped.append(victim)
    packet = {x[0]: x[1] for i, x in kept}
    def line(ix):
        i, x = ix
        return [x[0], len(encode(x[1]).encode()), x[2], x[3]]
    receipt = {'columns': ['name', 'bytes', 'priority', 'stable'], 'included': [line(x) for x in kept], 'dropped': [line(x) for x in dropped], 'total_bytes': len(encode(packet).encode())}
    for x, rows, desc, diag in selections:
        diag['included'] = x['mode'] == 'schema' or x['name'] in packet
    return {'packet': packet, 'inspection': {'recipe': rname, 'receipt': receipt, 'datasets': [d for x, rows, desc, d in selections],
            'missing_sections': [x['name'] for x in r['sections'] if x['name'] not in s['sections']]}}

def same(a, b):
    import query_eval
    if isinstance(a, dict) and isinstance(b, dict):
        return set(a) == set(b) and all(same(a[k], b[k]) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(same(x, y) for x, y in zip(a, b))
    return query_eval.dumps(a) == query_eval.dumps(b)

def evaluate(p, suite):
    fields(suite, ['version', 'cases', 'runs'])
    need(type(suite['version']) is int and suite['version'] == 1 and 0 < len(suite['cases']) <= 128 and 0 < len(suite['runs']) <= 8)
    ids = []
    prepared = {}
    for c in suite['cases']:
        fields(c, ['id', 'recipe', 'snapshot', 'question', 'expected'])
        name(c['id'])
        prose(c['question'], 4096)
        need(c['id'] not in ids)
        ids.append(c['id'])
        prepared[c['id']] = snapshot(p, c['snapshot'])
        build(p, prepared[c['id']], c['recipe'])
    names = []
    runs = []
    for run in suite['runs']:
        fields(run, ['name', 'recipe', 'guidance', 'observations'])
        name(run['name'])
        need(run['name'] not in names and run['guidance'] in ('standard', 'explicit-selection'))
        names.append(run['name'])
        need(len(run['observations']) == len(ids))
        seen = set()
        details = []
        correct = queries = invalid_queries = invalid_calls = packet_bytes = 0
        totals = {'input_tokens': 0, 'output_tokens': 0, 'latency_micros': 0}
        for o in run['observations']:
            fields(o, ['case_id', 'sql', 'answer', 'input_tokens', 'output_tokens', 'latency_micros', 'tool_calls'])
            need(o['case_id'] in ids and o['case_id'] not in seen)
            seen.add(o['case_id'])
            case = next(c for c in suite['cases'] if c['id'] == o['case_id'])
            recipe = run['recipe'] if run['recipe'] is not None else case['recipe']
            built = build(p, prepared[case['id']], recipe)
            packet_bytes += len(encode(built['packet']).encode())
            answer_ok = same(o['answer'], case['expected'])
            correct += answer_ok
            query_ok = False
            if o['sql'] is not None:
                # Use the independent, existing SQL prototype over exactly the recipe view.
                need(isinstance(o['sql'], str) and len(o['sql'].encode()) <= 4096)
                import query_eval
                r = next(r for r in p['recipes'] if r['name'] == recipe)
                tables = {x['name']: rows for x, rows, desc, diag in selected(p, prepared[case['id']], r)}
                try:
                    result = query_eval.run_sql(o['sql'], tables)
                    query_ok = same(result, case['expected'])
                except (ValueError, KeyError, TypeError, query_eval.QueryError):
                    invalid_queries += 1
                queries += query_ok
            r = next(r for r in p['recipes'] if r['name'] == recipe)
            need(isinstance(o['tool_calls'], list) and len(o['tool_calls']) <= 32)
            bad_calls = 0
            for call in o['tool_calls']:
                fields(call, ['name', 'arguments'])
                try:
                    need(call['name'] in r['tools'])
                    t = next(t for t in p['tools'] if t['name'] == call['name'])
                    validate(t['parameters'], call['arguments'])
                except (ValueError, KeyError, StopIteration):
                    bad_calls += 1
            invalid_calls += bad_calls
            for k in totals:
                if o[k] is None:
                    totals[k] = None
                else:
                    uint(o[k], 1000000000000)
                    if totals[k] is not None:
                        totals[k] += o[k]
            details.append({'id': case['id'], 'answer_correct': answer_ok, 'query_correct': query_ok,
                            'invalid_tool_calls': bad_calls, 'packet_bytes': len(encode(built['packet']).encode())})
        runs.append({'name': run['name'], 'guidance': run['guidance'], 'cases': len(ids), 'answers_correct': correct,
                     'queries_correct': queries, 'invalid_queries': invalid_queries, 'invalid_tool_calls': invalid_calls,
                     'packet_bytes': packet_bytes, **totals, 'details': details})
    return {'version': 1, 'runs': runs}

def process(v):
    fields(v, ['project', 'op', 'args'])
    p = definition(v['project'])
    a = v['args']
    if v['op'] == 'evaluate':
        return evaluate(p, a)
    if v['op'] == 'tools':
        fields(a, ['recipe'])
        r = next(r for r in p['recipes'] if r['name'] == a['recipe'])
        return [{'type': 'function', 'function': {**next(t for t in p['tools'] if t['name'] == n), 'strict': False}} for n in r['tools']]
    if v['op'] == 'features':
        fields(a, ['dataset', 'revision', 'policies'])
        uint(a['revision'], 1000000)
        need(a['revision'] > 0 and 0 < len(a['policies']) <= 16)
        d = next(d for d in p['datasets'] if d['name'] == a['dataset'])
        features = []
        for policy in a['policies']:
            fields(policy, ['name', 'min', 'max', 'invert', 'missing', 'outside'])
            f = next(f for f in d['fields'] if f['name'] == policy['name'])
            need(f['type'] in ('number', 'integer') and f['visibility'] == 'model' and f['unit'] is not None)
            need(policy['missing'] in ('unknown', 'error') and policy['outside'] in ('unknown', 'error', 'clamp') and type(policy['invert']) is bool)
            need(-1000000000 <= policy['min'] < policy['max'] <= 1000000000)
            features.append({'name': policy['name'], 'unit': f['unit'], **{k: v for k, v in policy.items() if k != 'name'}})
        need(len(set(f['name'] for f in features)) == len(features))
        return {'version': 1, 'id': a['dataset'], 'revision': a['revision'], 'features': features}
    if v['op'] == 'query':
        fields(a, ['recipe', 'snapshot', 'sql'])
        r = next(r for r in p['recipes'] if r['name'] == a['recipe'])
        selections = selected(p, snapshot(p, a['snapshot']), r)
        import query_eval
        items, table, conds, order, limit = query_eval.parse_sql(a['sql'])
        d = next(d for d in p['datasets'] if d['name'] == table)
        x = next(x for x in r['datasets'] if x['name'] == table)
        columns = x['view'].get('fields', [f['name'] for f in d['fields'] if f['visibility'] == 'model'])
        used = []
        for item in items: query_eval.columns_of(item, used)
        used += [c for c, _, _ in conds] + ([order[0]] if order else [])
        need(set(used) <= set(columns))
        return query_eval.run_sql(a['sql'], {x['name']: rows for x, rows, desc, diag in selections})
    if v['op'] == 'build':
        fields(a, ['recipe', 'snapshot'])
        return build(p, snapshot(p, a['snapshot']), a['recipe'])
    if v['op'] == 'validate':
        fields(a, ['name', 'arguments'])
        t = next(t for t in p['tools'] if t['name'] == a['name'])
        need(len(encode(a['arguments']).encode()) <= 65536)
        validate(t['parameters'], a['arguments'])
        return {'name': a['name'], 'arguments': a['arguments']}
    if v['op'] == 'doctor':
        fields(a, [])
        warnings = []
        for d in p['datasets']:
            if d['max_age'] is None:
                warnings.append({'code': 'unbounded_age', 'path': d['name']})
            for f in d['fields']:
                if f['type'] in ('number', 'integer') and f['unit'] is None:
                    warnings.append({'code': 'missing_unit', 'path': d['name'] + '.' + f['name']})
        return {'version': 1, 'datasets': len(p['datasets']), 'tools': len(p['tools']), 'recipes': len(p['recipes']), 'warnings': warnings}
    raise ValueError('unknown op')

def fixture(seed=1):
    rng = random.Random(seed)
    def f(n, t, unit=None, visibility='model'):
        return {'name': n, 'type': t, 'description': n.replace('_', ' '), 'unit': unit, 'nullable': False, 'visibility': visibility}
    p = {'version': 1, 'datasets': [{'name': 'stock', 'description': 'Items visible to this actor.', 'id': 'id', 'max_age': 5,
          'fields': [f('id', 'string'), f('kind', 'string'), f('count', 'integer', 'items'), f('secret', 'string', visibility='host')]}],
         'tools': [{'name': 'inspect_item', 'description': 'Inspect one currently accessible item.', 'parameters': {'type': 'object', 'properties': {'id': {'type': 'string', 'maxLength': 256}}, 'required': ['id'], 'additionalProperties': False}}],
         'recipes': [{'name': 'conversation', 'budget_bytes': 4096, 'datasets': [{'name': 'stock', 'mode': 'rows', 'view': {'where': {'kind': 'food'}, 'sort_by': '-count', 'limit': 3, 'fields': ['id', 'count']}, 'priority': 5, 'required': False}], 'tools': ['inspect_item'], 'sections': [{'name': 'event', 'priority': 8, 'required': True}]}]}
    s = {'now': 10, 'tables': {'stock': {'updated_at': 8, 'rows': [{'id': f'grove:item_{i}', 'kind': rng.choice(['food', 'tool']), 'count': rng.randrange(1, 100), 'secret': 'host-only'} for i in range(24)]}}, 'sections': {'event': 'Choose supplies.'}}
    return p, s

def suite_fixture():
    cases = []
    observations = []
    for i, seed in enumerate([201, 202, 203]):
        p, s = fixture(seed)
        rows = selected(p, snapshot(p, s), p['recipes'][0])[0][1]
        expected = sum(row['count'] for row in rows)
        ident = 'state_' + str(i)
        cases.append({'id': ident, 'recipe': 'conversation', 'snapshot': s,
                      'question': 'What is the total count in the selected view?', 'expected': expected})
        observations.append({'case_id': ident, 'sql': 'SELECT SUM(count) FROM stock', 'answer': expected,
                             'input_tokens': None, 'output_tokens': None, 'latency_micros': None,
                             'tool_calls': [{'name': 'inspect_item', 'arguments': {'id': 'grove:item_0'}}]})
    incorrect = copy.deepcopy(observations)
    incorrect[0]['sql'] = 'SELECT SUM(secret) FROM stock'
    incorrect[0]['answer'] = 0
    incorrect[1]['tool_calls'][0]['arguments']['unexpected'] = True
    incorrect[2]['input_tokens'] = 50
    return {'version': 1, 'cases': cases, 'runs': [
        {'name': 'scripted_correct', 'recipe': None, 'guidance': 'standard', 'observations': observations},
        {'name': 'scripted_errors', 'recipe': None, 'guidance': 'explicit-selection', 'observations': incorrect}]}

def cases():
    p, s = fixture()
    out = [('build', {'project': p, 'op': 'build', 'args': {'recipe': 'conversation', 'snapshot': s}}, False)]
    for seed in range(2, 102):
        p, s = fixture(seed)
        r = p['recipes'][0]
        if seed % 2:
            r['datasets'][0]['mode'] = 'schema'
        if seed % 3 == 0:
            r['budget_bytes'] = 750
        out.append((f'state_{seed}', {'project': p, 'op': 'build', 'args': {'recipe': 'conversation', 'snapshot': s}}, False))
    p, s = fixture()
    out += [('doctor', {'project': p, 'op': 'doctor', 'args': {}}, False),
            ('valid_call', {'project': p, 'op': 'validate', 'args': {'name': 'inspect_item', 'arguments': {'id': 'grove:Élan.9007199254740993'}}}, False)]
    suite = suite_fixture()
    out.append(('evaluation', {'project': fixture()[0], 'op': 'evaluate', 'args': suite}, False))
    for kind in ['duplicate_observation', 'unknown_case', 'missing_observation', 'invalid_guidance']:
        bad_suite = copy.deepcopy(suite)
        obs = bad_suite['runs'][0]['observations']
        if kind == 'duplicate_observation': obs[1] = copy.deepcopy(obs[0])
        if kind == 'unknown_case': obs[0]['case_id'] = 'unknown'
        if kind == 'missing_observation': obs.pop()
        if kind == 'invalid_guidance': bad_suite['runs'][0]['guidance'] = 'guess'
        out.append((kind, {'project': fixture()[0], 'op': 'evaluate', 'args': bad_suite}, True))
    p, s = fixture()
    out.append(('tools', {'project': p, 'op': 'tools', 'args': {'recipe': 'conversation'}}, False))
    out.append(('features', {'project': p, 'op': 'features', 'args': {'dataset': 'stock', 'revision': 1, 'policies': [{'name': 'count', 'min': 0, 'max': 100, 'invert': False, 'missing': 'unknown', 'outside': 'clamp'}]}}, False))
    out.append(('query', {'project': p, 'op': 'query', 'args': {'recipe': 'conversation', 'snapshot': s, 'sql': 'SELECT SUM(count) FROM stock'}}, False))
    out.append(('secret_query', {'project': p, 'op': 'query', 'args': {'recipe': 'conversation', 'snapshot': s, 'sql': 'SELECT secret FROM stock'}}, True))
    empty = copy.deepcopy(s)
    empty['tables']['stock']['rows'] = []
    out.append(('empty_secret_query', {'project': p, 'op': 'query', 'args': {'recipe': 'conversation', 'snapshot': empty, 'sql': 'SELECT secret FROM stock'}}, True))
    out.append(('empty_schema', {'project': p, 'op': 'build', 'args': {'recipe': 'conversation', 'snapshot': empty}}, False))
    rich = copy.deepcopy(p)
    rich['tools'][0]['parameters'] = {'type': 'object', 'properties': {
        'targets': {'type': 'array', 'maxItems': 3, 'items': {'type': 'object', 'properties': {
            'id': {'type': 'string', 'maxLength': 32}, 'count': {'type': ['integer', 'null'], 'minimum': 1, 'maximum': 9007199254740992}},
            'required': ['id'], 'additionalProperties': False}},
        'kind': {'type': 'string', 'maxLength': 8, 'enum': ['food', 'tool']}},
        'required': ['targets'], 'additionalProperties': False}
    for n, args, error in [
        ('nested', {'targets': [{'id': 'Élan', 'count': None}]}, False),
        ('optional', {'targets': [{'id': 'a'}], 'kind': 'food'}, False),
        ('bound', {'targets': [{'id': 'a', 'count': 9007199254740993}]}, True),
        ('fraction', {'targets': [{'id': 'a', 'count': 1.5}]}, True),
        ('enum', {'targets': [], 'kind': 'other'}, True),
        ('array_limit', {'targets': [{'id': 'a'}] * 4}, True),
        ('nested_extra', {'targets': [{'id': 'a', 'extra': False}]}, True)]:
        out.append((n, {'project': rich, 'op': 'validate', 'args': {'name': 'inspect_item', 'arguments': args}}, error))
    unsupported = copy.deepcopy(rich)
    unsupported['tools'][0]['parameters']['$ref'] = '#/$defs/unknown'
    out.append(('unsupported_schema', {'project': unsupported, 'op': 'doctor', 'args': {}}, True))
    warnings = copy.deepcopy(p)
    warnings['datasets'][0]['max_age'] = None
    warnings['datasets'][0]['fields'][2]['unit'] = None
    out.append(('warnings', {'project': warnings, 'op': 'doctor', 'args': {}}, False))
    for kind in ['budget_drop', 'required_budget', 'missing_optional']:
        p, s = fixture()
        if kind == 'budget_drop': p['recipes'][0]['budget_bytes'] = 550
        if kind == 'required_budget': p['recipes'][0]['budget_bytes'] = 530
        if kind == 'missing_optional': p['recipes'][0]['sections'].append({'name':'memories','priority':2,'required':False})
        out.append((kind, {'project':p,'op':'build','args':{'recipe':'conversation','snapshot':s}}, kind == 'required_budget'))
    duplicate_enum = copy.deepcopy(rich)
    duplicate_enum['tools'][0]['parameters']['properties']['kind']['enum'] = ['food', 'food']
    out.append(('duplicate_enum', {'project':duplicate_enum,'op':'doctor','args':{}}, True))
    base = out[0][1]
    for n in ['stale', 'future', 'duplicate_id', 'wrong_type', 'unknown_field', 'secret_projection', 'secret_filter', 'unknown_dataset', 'missing_section', 'unknown_recipe', 'version', 'unknown_config', 'duplicate_field']:
        v = copy.deepcopy(base)
        p, a = v['project'], v['args']
        t = a['snapshot']['tables']['stock']
        if n == 'stale': t['updated_at'] = 0
        if n == 'future': t['updated_at'] = 11
        if n == 'duplicate_id': t['rows'][1]['id'] = t['rows'][0]['id']
        if n == 'wrong_type': t['rows'][0]['count'] = True
        if n == 'unknown_field': t['rows'][0]['extra'] = 1
        if n == 'secret_projection': p['recipes'][0]['datasets'][0]['view']['fields'] = ['secret']
        if n == 'secret_filter': p['recipes'][0]['datasets'][0]['view']['where']['secret'] = 'x'
        if n == 'unknown_dataset': p['recipes'][0]['datasets'][0]['name'] = 'unknown'
        if n == 'missing_section': del a['snapshot']['sections']['event']
        if n == 'unknown_recipe': a['recipe'] = 'unknown'
        if n == 'version': p['version'] = 2
        if n == 'unknown_config': p['unexpected'] = True
        if n == 'duplicate_field': p['datasets'][0]['fields'].append(p['datasets'][0]['fields'][0])
        out.append((n, v, True))
    for n, args in [('missing', {}), ('extra', {'id': 'a', 'extra': 2}), ('wrong', {'id': 2}), ('too_long', {'id': 'x' * 257})]:
        out.append(('call_' + n, {'project': fixture()[0], 'op': 'validate', 'args': {'name': 'inspect_item', 'arguments': args}}, True))
    return out

def main():
    all_cases = cases()
    native = '--check' in sys.argv
    counts = {'cases': len(all_cases), 'success': 0, 'rejected': 0, 'native_comparisons': 0, 'api_spend_usd': 0}
    for n, v, error in all_cases:
        try:
            expected = process(v)
            need(not error)
            counts['success'] += 1
        except (ValueError, KeyError, StopIteration, TypeError, __import__('query_eval').QueryError):
            if not error:
                raise
            expected = None
            counts['rejected'] += 1
        if native:
            run = subprocess.run([str(ROOT / 'target/debug/gamectx'), 'kit'], input=encode(v), text=True, capture_output=True, check=False)
            need((run.returncode != 0) == error)
            if not error:
                need(run.stdout.rstrip('\n') == encode(expected))
            counts['native_comparisons'] += 1
    p, s = fixture()
    data = snapshot(p, s)
    b = build(p, data, 'conversation')
    counts['source_rows'] = len(s['tables']['stock']['rows'])
    counts['selected_rows'] = b['inspection']['datasets'][0]['selected_rows']
    counts['source_table_bytes'] = len(encode(s['tables']['stock']['rows']).encode())
    counts['selected_table_bytes'] = len(encode(b['packet']['stock']).encode())
    counts['packet_bytes'] = len(encode(b['packet']).encode())
    print(json.dumps(counts, indent=2))

if __name__ == '__main__':
    main()
