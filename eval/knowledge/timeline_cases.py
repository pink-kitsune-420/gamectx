"""World timeline cases. Expectations were written before running the reference; the
`timeline` local kind must match the reference byte for byte. Offline only (--cap 0).

  python3 -I eval/knowledge/timeline_cases.py --cap 0 [--write-conformance] [--random N --seed S]
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse, copy, random, subprocess
import timeline_reference as t

ROOT = _pl.Path(__file__).resolve().parents[2]


def rec(at, kind, summary, **kw): return dict(op='record', args=dict(at=at, kind=kind, summary=summary, **kw))
def q(**f): return dict(op='query', args=f)
def run(ops, timeline=None): return dict(timeline=timeline, ops=ops)
def ok(i, **want):
    def check(out):
        r = out['results'][i]
        return r['ok'] and all(r['result'].get(k) == v for k, v in want.items())
    return check
def failed(i): return lambda out: not out['results'][i]['ok']
def seqs(i, want): return lambda out: out['results'][i]['ok'] and [e['seq'] for e in out['results'][i]['result']['entries']] == want
def both(*fs): return lambda out: all(f(out) for f in fs)


# Ember Keep history (original): a forge fire, a stolen ledger and a betrothal.
FIRE = rec(10, 'fire', 'A fire broke out in the Ember Keep forge.', actors=['smith'], place='forge', tags=['disaster'], phrases=['the fire', 'forge fire'])
THEFT = rec(20, 'theft', 'Someone took the steward\'s ledger.', actors=['thief'], place='hall', visibility='witnessed', witnesses=['cook'], things=['ledger'], phrases=['ledger'])
PACT = rec(30, 'pact', 'The miller promised his mill to the keep.', actors=['miller', 'steward'], place='mill', visibility='private', phrases=['the mill'],
           facts=dict(binding=True, price=40, witness='none'))
HISTORY = [FIRE, THEFT, PACT]

CASES = [
    ('record_first', run([FIRE]), ok(0, seq=1), False),
    ('record_defaults', run([rec(5, 'arrival', 'A caravan reached the gate.')]),
     lambda o: o['timeline']['entries'][0] == dict(seq=1, at=5, kind='arrival', summary='A caravan reached the gate.', actors=[], place=None, things=[], tags=[],
                                                   visibility='public', witnesses=[], phrases=[], facts={}, retracted=None, covers=None), False),
    ('record_normalizes_phrases', run([rec(1, 'x', 'S.', phrases=['  The   FIRE! '])]), lambda o: o['timeline']['entries'][0]['phrases'] == ['the fire'], False),
    ('record_three', run(HISTORY), both(ok(0, seq=1), ok(1, seq=2), ok(2, seq=3), lambda o: o['timeline']['next'] == 4), False),
    ('record_backfill_order', run([rec(50, 'b', 'Later.'), rec(5, 'a', 'Long ago.'), q(order='oldest')]), seqs(2, [2, 1]), False),
    ('query_newest_default', run(HISTORY + [q()]), seqs(3, [3, 2, 1]), False),  # no known_to: the host sees everything
    ('known_public_only', run(HISTORY + [q(known_to='baker')]), seqs(3, [1]), False),
    ('known_witness', run(HISTORY + [q(known_to='cook')]), seqs(3, [2, 1]), False),
    ('known_actor_private', run(HISTORY + [q(known_to='miller')]), seqs(3, [3, 1]), False),
    ('known_witness_not_private', run(HISTORY + [rec(40, 'x', 'Secret.', actors=['a'], visibility='private', witnesses=['cook']), q(known_to='cook')]), seqs(4, [2, 1]), False),
    ('filter_actor', run(HISTORY + [q(actor='steward')]), seqs(3, [3]), False),
    ('filter_place', run(HISTORY + [q(place='forge')]), seqs(3, [1]), False),
    ('filter_thing', run(HISTORY + [q(thing='ledger')]), seqs(3, [2]), False),
    ('filter_kind', run(HISTORY + [q(kind='theft')]), seqs(3, [2]), False),
    ('filter_tag', run(HISTORY + [q(tag='disaster')]), seqs(3, [1]), False),
    ('filter_window', run(HISTORY + [q(since=15, until=30)]), seqs(3, [3, 2]), False),
    ('about_phrase', run(HISTORY + [q(about='What about the fire?')]), seqs(3, [1]), False),
    ('about_whole_words', run(HISTORY + [q(about='the miller')]), seqs(3, []), False),
    ('about_short_inside_phrase', run(HISTORY + [q(about='Forge')]), seqs(3, [1]), False),
    ('limit_and_total', run(HISTORY + [q(limit=1)]), both(seqs(3, [3]), ok(3, total=3)), False),
    ('retract_hides', run(HISTORY + [dict(op='retract', args=dict(seq=2, reason='Recorded by mistake.')), q()]), seqs(4, [3, 1]), False),
    ('retract_included', run(HISTORY + [dict(op='retract', args=dict(seq=2, reason='Wrong.')), q(include_retracted=True)]), seqs(4, [3, 2, 1]), False),
    ('retract_twice_fails', run(HISTORY + [dict(op='retract', args=dict(seq=2, reason='A.')), dict(op='retract', args=dict(seq=2, reason='B.'))]), failed(4), False),
    ('retract_missing_fails', run(HISTORY + [dict(op='retract', args=dict(seq=9, reason='A.'))]), failed(3), False),
    ('compact_two', run(HISTORY + [dict(op='compact', args=dict(through=2, summary='Early troubles at the keep.')), q(order='oldest')]),
     both(ok(3, seq=2, count=2), seqs(4, [2, 3]), lambda o: o['timeline']['entries'][0]['covers'] == {'from': 1, 'through': 2, 'count': 2}
          and o['timeline']['entries'][0]['at'] == 20 and o['timeline']['entries'][0]['kind'] == 'summary'), False),
    ('compact_nested_counts', run(HISTORY + [rec(40, 'x', 'Four.'), dict(op='compact', args=dict(through=2, summary='A.')),
                                             dict(op='compact', args=dict(through=3, summary='B.', kind='era'))]),
     both(ok(5, seq=3, count=3), lambda o: o['timeline']['entries'][0]['covers'] == {'from': 1, 'through': 3, 'count': 3}), False),
    ('compact_one_fails', run(HISTORY + [dict(op='compact', args=dict(through=1, summary='A.'))]), failed(3), False),
    ('compact_missing_seq_fails', run(HISTORY + [dict(op='compact', args=dict(through=9, summary='A.'))]), failed(3), False),
    ('record_after_compact_keeps_seq', run(HISTORY + [dict(op='compact', args=dict(through=3, summary='A.')), rec(50, 'y', 'Next.')]), ok(4, seq=4), False),
    ('bad_dup_actor', run([rec(1, 'x', 'S.', actors=['a', 'a'])]), failed(0), False),
    ('bad_dup_phrase', run([rec(1, 'x', 'S.', phrases=['The fire', 'the FIRE'])]), failed(0), False),
    ('bad_empty_phrase', run([rec(1, 'x', 'S.', phrases=['!!'])]), failed(0), False),
    ('bad_visibility', run([rec(1, 'x', 'S.', visibility='hidden')]), failed(0), False),
    ('bad_summary_long', run([rec(1, 'x', 'S' * 241)]), failed(0), False),
    ('bad_fact_value', run([rec(1, 'x', 'S.', facts=dict(a=1.5))]), failed(0), False),
    ('bad_window', run(HISTORY + [q(since=30, until=10)]), failed(3), False),
    ('bad_limit', run(HISTORY + [q(limit=65)]), failed(3), False),
    ('bad_unknown_op', run([dict(op='erase', args={})]), failed(0), False),
    ('failed_op_atomic', run([FIRE, rec(2, 'x', 'S.', actors=['a', 'a']), rec(3, 'y', 'T.')]), both(failed(1), ok(2, seq=2)), False),
    ('resume_timeline', run([rec(60, 'z', 'Later.'), q(order='oldest')], timeline=t.workflow(run(HISTORY))['timeline']), both(ok(0, seq=4), seqs(1, [1, 2, 3, 4])), False),
    ('error_unsorted_entries', run([], timeline=dict(version=1, next=3, entries=[dict(t.workflow(run(HISTORY))['timeline']['entries'][1]),
                                                                            dict(t.workflow(run(HISTORY))['timeline']['entries'][0])])), None, True),
    ('error_seq_not_below_next', run([], timeline=dict(version=1, next=2, entries=t.workflow(run(HISTORY))['timeline']['entries'])), None, True),
    ('error_unnormalized_phrase', run([], timeline=dict(version=1, next=2, entries=[dict(t.workflow(run([FIRE]))['timeline']['entries'][0], phrases=['The Fire'])])), None, True),
    ('error_bad_covers', run([], timeline=dict(version=1, next=2, entries=[dict(t.workflow(run([FIRE]))['timeline']['entries'][0], covers={'from': 1, 'through': 1, 'count': 2})])), None, True),
    ('error_too_many_ops', run([q()] * 65), None, True),
    ('error_unknown_key', dict(timeline=None, ops=[], extra=1), None, True),
]


def random_case(rng):
    actors = ['smith', 'cook', 'thief', 'miller', 'steward']
    ops = []
    for _ in range(rng.randint(1, 40)):
        r = rng.random()
        if r < 0.5:
            e = dict(at=rng.randint(0, 100), kind=rng.choice(['fire', 'trade', 'theft', 'pact']), summary=rng.choice(['A.', 'Something happened.', 'x' * rng.choice([1, 240, 241])]))
            if rng.random() < 0.6: e['actors'] = rng.sample(actors, rng.randint(0, 3)) + (['smith', 'smith'] if rng.random() < 0.05 else [])
            if rng.random() < 0.4: e['place'] = rng.choice(['forge', 'hall', 'mill'])
            if rng.random() < 0.5: e['visibility'] = rng.choice(t.VISIBILITY + ['secret'])
            if rng.random() < 0.4: e['witnesses'] = rng.sample(actors, rng.randint(0, 3))
            if rng.random() < 0.4: e['phrases'] = rng.sample(['the fire', 'Fire!', 'mill', 'the mill', '??', 'ledger'], rng.randint(0, 3))
            if rng.random() < 0.3: e['tags'] = rng.sample(['a', 'b', 'c'], rng.randint(0, 2))
            if rng.random() < 0.2: e['facts'] = {'k': rng.choice([True, 3, 'v', 1.5])}
            ops.append(dict(op='record', args=e))
        elif r < 0.6: ops.append(dict(op='retract', args=dict(seq=rng.randint(0, 12), reason='Mistake.')))
        elif r < 0.68: ops.append(dict(op='compact', args=dict(through=rng.randint(0, 12), summary='Earlier days.', **({'kind': 'era'} if rng.random() < 0.3 else {}))))
        else:
            f = {}
            for k, vals in [('known_to', actors + ['nobody']), ('actor', actors), ('place', ['forge', 'mill']), ('kind', ['fire', 'trade']), ('tag', ['a', 'b']),
                            ('about', ['the fire', 'mill', 'what of the mill?', 'ledger', '!!']), ('order', ['newest', 'oldest', 'random']),
                            ('limit', [1, 3, 64, 0]), ('since', [0, 30]), ('until', [50, 100, 10]), ('include_retracted', [True, False])]:
                if rng.random() < 0.25: f[k] = rng.choice(vals)
            ops.append(dict(op='query', args=f))
    return run(ops)


def call(req):
    return subprocess.run([str(ROOT / 'target/debug/gamectx'), 'local', '-'], input=t.enc(req), capture_output=True, text=True)


def main():
    p = argparse.ArgumentParser(); p.add_argument('--cap', type=float, required=True); p.add_argument('--prototype-only', action='store_true')
    p.add_argument('--write-conformance', action='store_true'); p.add_argument('--random', type=int, default=0); p.add_argument('--seed', type=int, default=1)
    a = p.parse_args(); assert a.cap == 0, 'timeline cases are offline only'
    if a.random:
        rng = random.Random(a.seed); mismatches = 0
        for _ in range(a.random):
            args = random_case(rng); req = dict(kind='timeline', args=args)
            got = t.workflow(copy.deepcopy(args)); x = call(req)
            if x.returncode != 0 or x.stdout.strip() != t.enc(got): mismatches += 1; print('MISMATCH', t.enc(args)[:600])
        print(t.enc(dict(workflows=a.random, mismatches=mismatches, actual_model_calls=0, api_spend_usd=0))); return
    for n, args, check, error in CASES:
        try: got = t.workflow(copy.deepcopy(args)); assert not error, n
        except (AssertionError, KeyError, TypeError, ValueError, StopIteration, IndexError):
            assert error, n; got = None
        if check is not None: assert check(got), (n, t.enc(got)[:1500])
        req = dict(kind='timeline', args=args)
        if not a.prototype_only:
            x = call(req); assert (x.returncode != 0) == error, (n, x.stderr)
            if not error: assert x.stdout.strip() == t.enc(got), (n, x.stdout[:1500], t.enc(got)[:1500])
        if a.write_conformance:
            dd = ROOT / 'conformance/timeline' / n; new = not dd.exists(); dd.mkdir(parents=True, exist_ok=True)
            for k, v in [('case', dict(op='local', **(dict(error=True) if error else {}))), ('input', req)] + ([] if error else [('expected', got)]):
                f = dd / (k + '.json')
                if new: f.write_text(t.enc(v) + '\n')
                else: assert f.read_text() == t.enc(v) + '\n', f
    print(t.enc(dict(workflows=len(CASES), malformed=sum(e for *_, e in CASES), actual_model_calls=0, api_spend_usd=0)))


if __name__ == '__main__':
    main()
