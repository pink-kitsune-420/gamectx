"""Predeclared visual director expectations (stage 2.3), checked on the Python reference and
then byte for byte against the Rust `visual-director` local kind. Offline only (--cap 0).

Expectations, written before the Rust director existed:
  V1 With no tier requested the highest tier is used; an entity missing there falls back
     to the next lower tier that has it, and `requested_tier` says what was asked.
  V2 Format profiles change the compiled spec (portrait, scene and sprite keys differ).
  V3 Selectors emit facts only when every `when` value matches with the same JSON type;
     the art bible's occlusion groups still decide which emitted facts are drawn.
  V4 Nothing is queued unless `generate` is true; a queued key is a placeholder until
     approved, then `exact` with reason `reuse`.
  V5 A saved approval is re-verified after a restore: verified -> exact, unverified (host
     cannot find the asset) -> placeholder and the approval is dropped from the save.
  V6 A queued repair is applied once per base key; the repaired spec falls back to the
     approved base art until it is approved itself. (Added after the first run, not
     predeclared: a repair that adds tokens the base art lacks gets no base fallback.)
  V7 Rejecting drops the saved approval. A failed direct (bad repair) changes nothing.
  V8 Malformed config, profiles, selectors, checkpoints and requests are rejected.
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse, copy, json, pathlib, subprocess, sys
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import visual_reference as r
import visual_director_reference as d
ROOT = pathlib.Path(__file__).resolve().parents[2]
BASIC = json.loads((ROOT / 'examples/fixtures/visual/chime-art.json').read_text())


def detailed():
    b = copy.deepcopy(BASIC); b['id'] = 'chime_art_detailed'; b['style'] = 'Original painterly ink illustration with fine linework'
    b['anchors'] = [a for a in b['anchors'] if a['entity'] == 'merchant']
    b['palette'] = ['copper', 'deep blue', 'warm gray', 'lamp gold']; return b


PROFILES = [dict(id='portrait', format='portrait', framing=None, camera_angle=None, size=None),
            dict(id='scene', format='scene', framing='wide room card', camera_angle='eye level wide view', size=None),
            dict(id='sprite', format='sprite', framing='full figure', camera_angle=None, size=dict(width=256, height=512, aspect=dict(width=1, height=2)))]
SELECTORS = [dict(fact='shutters_closed', when=dict(shutters=[True])), dict(fact='visible_sun', when=dict(weather=['sunny'])),
             dict(fact='visible_lamp', when=dict(lamp=['repaired', 'polished']))]


def cfg(**over): return dict(dict(version=1, tiers=[dict(id='basic', bible=BASIC), dict(id='detailed', bible=detailed())], profiles=PROFILES, selectors=SELECTORS), **over)
REGISTRY = dict(enabled=True, capacity=16, queue_limit=2, epoch='director_run')
URI = 'asset://chime/merchant.png'; SHA = 'a' * 64


def request(**over):
    q = dict(tier=None, profile='portrait', entity='merchant', scope='counter', observer='player', listener='player', audience=['player'],
             scene=dict(id='counter', revision=1, epoch=1, at=10, participants=['merchant', 'player']), vars={}, life_revision=None, activity_revision=None, generate=False)
    q.update(over); return q
def direct(missing=(), **over): return dict(op='direct', args=dict(request=request(**over), missing=list(missing)))
def op(k, **args): return dict(op=k, args=args)
def patch(seed=1, **over): return dict(dict(replace_tokens=[], add_tokens=[], negative_tokens=[], forbid_props=[], seed_delta=seed, asset=None), **over)
def flow(ops, checkpoint=None, config=None, registry=REGISTRY): return dict(config=config or cfg(), registry=registry, checkpoint=checkpoint, operations=ops)
def key_of(ops): return d.workflow(flow(ops))['results'][-1]['result']['key']


def res(out, i): x = out['results'][i]; assert x['ok'], (i, x); return x['result']


def cases():
    K = key_of([direct()])
    saved = dict(version=1, approvals=[dict(key=K, asset_uri=URI, digest=SHA)], repairs=[])
    approved = [direct(generate=True), op('submit', key=0, ticket=0, asset_uri=URI, digest=SHA), op('approve', key=0, asset_uri=URI, digest=SHA)]
    out = []
    def add(n, a, check=None, error=False): out.append((n, a, check, error))
    # V1
    add('tier_highest_default', flow([direct()]), lambda o: res(o, 0)['tier'] == 'detailed' and res(o, 0)['requested_tier'] == 'detailed' and res(o, 0)['status'] == 'placeholder' and res(o, 0)['reason'] == 'not_requested')
    add('tier_fallback_scene', flow([direct(entity='counter', profile='scene')]), lambda o: res(o, 0)['tier'] == 'basic' and res(o, 0)['requested_tier'] == 'detailed')
    add('tier_requested_basic', flow([direct(tier='basic')]), lambda o: res(o, 0)['tier'] == 'basic' and res(o, 0)['requested_tier'] == 'basic')
    add('tier_unknown_rejected', flow([direct(tier='ultra')]), lambda o: not o['results'][0]['ok'])
    add('entity_unknown_rejected', flow([direct(entity='ghost')]), lambda o: not o['results'][0]['ok'])
    add('scene_profile_needs_scene_entity', flow([direct(profile='scene')]), lambda o: not o['results'][0]['ok'])
    # V2
    add('profiles_change_spec', flow([direct(tier='basic'), direct(tier='basic', profile='sprite'), direct(tier='basic', entity='counter', profile='scene')]),
        lambda o: len({res(o, i)['key'] for i in range(3)}) == 3)
    add('profile_unknown_rejected', flow([direct(profile='banner')]), lambda o: not o['results'][0]['ok'])
    # V3
    add('selectors_emit', flow([direct(vars=dict(weather='sunny', lamp='polished'))]),
        lambda o: res(o, 0)['facts'] == ['visible_sun', 'visible_lamp'] and res(o, 0)['selected_facts'] == ['visible_lamp', 'visible_sun'])
    add('selectors_occlusion', flow([direct(vars=dict(shutters=True, weather='sunny'))]),
        lambda o: res(o, 0)['facts'] == ['shutters_closed', 'visible_sun'] and res(o, 0)['selected_facts'] == ['shutters_closed'])
    add('selectors_type_strict', flow([direct(vars=dict(shutters=1, weather='rainy'))]), lambda o: res(o, 0)['facts'] == [] and res(o, 0)['selected_facts'] == [])
    add('selectors_change_key', flow([direct(), direct(vars=dict(lamp='repaired'))]), lambda o: res(o, 0)['key'] != res(o, 1)['key'])
    # V4
    add('generate_queues', flow([direct(generate=True)]), lambda o: res(o, 0)['reason'] == 'queued' and res(o, 0)['pending'] and res(o, 0)['generation'] is not None and res(o, 0)['status'] == 'placeholder')
    add('approve_then_exact', flow(approved + [direct()]),
        lambda o: res(o, 3)['status'] == 'exact' and res(o, 3)['reason'] == 'reuse' and o['checkpoint']['approvals'] == [dict(key=K, asset_uri=URI, digest=SHA)])
    add('submit_is_not_approval', flow(approved[:2] + [direct()]), lambda o: res(o, 2)['status'] == 'placeholder' and res(o, 2)['reason'] == 'pending')
    # V5
    add('restore_verified', flow([direct()], checkpoint=saved), lambda o: res(o, 0)['approval'] == 'verified' and res(o, 0)['status'] == 'exact')
    add('restore_unverified', flow([direct(missing=[URI])], checkpoint=saved),
        lambda o: res(o, 0)['approval'] == 'unverified' and res(o, 0)['status'] == 'placeholder' and o['checkpoint']['approvals'] == [])
    add('restore_verified_once', flow([direct(), direct()], checkpoint=saved), lambda o: res(o, 1)['approval'] is None and res(o, 1)['status'] == 'exact')
    # V6
    add('repair_applied_once', flow([direct(), op('repair', base=0, patches=[patch()]), direct(), direct()]),
        lambda o: res(o, 2)['repaired'] and res(o, 2)['key'] != res(o, 0)['key'] and not res(o, 3)['repaired'] and res(o, 3)['key'] == res(o, 2)['key'])
    add('repair_falls_back_to_base', flow(approved + [op('repair', base=0, patches=[patch()]), direct()]),
        lambda o: res(o, 4)['repaired'] and res(o, 4)['status'] == 'fallback' and res(o, 4)['asset']['key'] == K)
    add('repair_new_tokens_no_base_fallback', flow(approved + [op('repair', base=0, patches=[patch(0, add_tokens=['soft lamp light'])]), direct()]),
        lambda o: res(o, 4)['repaired'] and res(o, 4)['status'] == 'placeholder')
    add('repair_restored_reapplied', flow([direct()], checkpoint=dict(saved, approvals=[], repairs=[dict(base=K, patches=[patch(), patch(2)])])),
        lambda o: res(o, 0)['repaired'] and res(o, 0)['key'] != K)
    # V7
    add('reject_drops_approval', flow(approved + [op('reject', key=0, reason='off_model'), direct()]),
        lambda o: o['checkpoint']['approvals'] == [] and res(o, 4)['status'] == 'placeholder' and res(o, 4)['reason'] == 'rejected')
    bad = patch(replace_tokens=[dict(**{'from': 'not a token here', 'to': 'x'})])
    add('failed_direct_is_atomic', flow([direct(), op('repair', base=0, patches=[bad]), direct(), op('checkpoint'), op('repair', base=0, patches=[patch()]), direct()]),
        lambda o: not o['results'][2]['ok'] and res(o, 3)['repairs'][0]['patches'] == [bad] and res(o, 5)['repaired'])
    add('approve_unknown_key_index', flow([op('approve', key=0, asset_uri=URI, digest=SHA)]), lambda o: not o['results'][0]['ok'])
    # V8
    add('bad_tier_duplicate', flow([], config=cfg(tiers=[dict(id='basic', bible=BASIC), dict(id='basic', bible=BASIC)])), error=True)
    add('bad_tiers_empty', flow([], config=cfg(tiers=[])), error=True)
    add('bad_profile_aspect', flow([], config=cfg(profiles=[dict(PROFILES[2], size=dict(width=300, height=512, aspect=dict(width=1, height=2)))])), error=True)
    add('bad_profile_format', flow([], config=cfg(profiles=[dict(PROFILES[0], format='banner')])), error=True)
    add('bad_selector_float', flow([], config=cfg(selectors=[dict(fact='visible_sun', when=dict(weather=[1.5]))])), error=True)
    add('bad_selector_duplicate', flow([], config=cfg(selectors=SELECTORS + SELECTORS[:1])), error=True)
    add('bad_checkpoint_key', flow([], checkpoint=dict(saved, approvals=[dict(key='visual:xyz', asset_uri=URI, digest=SHA)])), error=True)
    add('bad_checkpoint_patches_empty', flow([], checkpoint=dict(saved, approvals=[], repairs=[dict(base=K, patches=[])])), error=True)
    add('bad_request_life_revision', flow([direct(life_revision=1)]), lambda o: not o['results'][0]['ok'])
    add('bad_request_missing_uri', flow([direct(missing=['nope'])]), lambda o: not o['results'][0]['ok'])
    return out


def random_flow(g):
    """One randomized workflow: tiers, profiles, vars, generate, approvals, repairs, restores."""
    ops = []; n = 0
    for _ in range(g.randint(1, 10)):
        c = g.random()
        if n == 0 or c < .45:
            over = dict(generate=g.random() < .5)
            if g.random() < .4: over['tier'] = g.choice(['basic', 'detailed', 'ultra', None])
            if g.random() < .4: over['profile'] = g.choice(['portrait', 'scene', 'sprite', 'banner'])
            if g.random() < .3: over['entity'] = g.choice(['merchant', 'counter', 'ghost'])
            if g.random() < .5: over['vars'] = {k: g.choice([True, False, 1, 0, 'sunny', 'rainy', 'repaired', 'polished']) for k in g.sample(['shutters', 'weather', 'lamp', 'other'], g.randint(1, 4))}
            if g.random() < .05: over['life_revision'] = 1
            ops.append(direct(missing=[URI] if g.random() < .2 else [], **over)); n += 1
        elif c < .6: ops.append(op('approve', key=g.randrange(n + 1), asset_uri=g.choice([URI, 'asset://chime/other.png']), digest=g.choice([SHA, 'b' * 64])))
        elif c < .7: ops.append(op('submit', key=g.randrange(n + 1), ticket=g.randrange(n + 1), asset_uri=URI, digest=g.choice([SHA, 'b' * 64])))
        elif c < .8: ops.append(op('reject', key=g.randrange(n + 1), reason=g.choice(['off_model', ' bad'])))
        elif c < .95:
            ps = [patch(g.randint(0, 3), add_tokens=g.sample(['soft light', 'rain on glass'], g.randint(0, 2)), forbid_props=g.sample(['lamp', 'chime'], g.randint(0, 1))) for _ in range(g.randint(0, 3))]
            if g.random() < .1: ps = [patch(replace_tokens=[{'from': 'nope', 'to': 'x'}])]
            ops.append(op('repair', base=g.randrange(n + 1), patches=ps))
        else: ops.append(op('checkpoint'))
    return flow(ops, registry=dict(REGISTRY, capacity=g.choice([2, 4, 16]), queue_limit=g.choice([0, 1, 2]), enabled=g.random() < .9))


def differential(n, seed):
    import random
    g = random.Random(seed); stats = dict(same=0, mismatch=0, error=0, ops=0, statuses={})
    for i in range(n):
        args = random_flow(g)
        if g.random() < .3:  # continue from a saved checkpoint of an earlier run
            try: args['checkpoint'] = d.workflow(copy.deepcopy(args))['checkpoint']; args['operations'] = [direct(), direct(vars=dict(lamp='repaired'))] + args['operations'][:3]
            except (AssertionError, KeyError, TypeError, ValueError, StopIteration, IndexError): pass
        try: want = r.encode(d.workflow(copy.deepcopy(args)))
        except (AssertionError, KeyError, TypeError, ValueError, StopIteration, IndexError): want = None
        x = subprocess.run([str(ROOT / 'target/debug/gamectx'), 'local', '-'], input=r.encode(dict(kind='visual-director', args=args)), capture_output=True, text=True)
        have = x.stdout.strip() if x.returncode == 0 else None
        if have != want:
            stats['mismatch'] += 1; (ROOT / f'target/director_mismatch_{i}.json').write_text(r.encode(args)); continue
        stats['same' if want else 'error'] += 1
        for row in (json.loads(want)['results'] if want else []):
            stats['ops'] += 1; k = row['result'].get('status', 'ok') if row['ok'] else 'failed'; stats['statuses'][k] = stats['statuses'].get(k, 0) + 1
    return stats


def main():
    p = argparse.ArgumentParser(); p.add_argument('--cap', type=float, required=True); p.add_argument('--prototype-only', action='store_true'); p.add_argument('--write-conformance', action='store_true')
    p.add_argument('--random', type=int, default=0); p.add_argument('--seed', type=int, default=1)
    a = p.parse_args(); assert a.cap == 0, 'visual director cases are offline only'
    if a.random: print(r.encode(dict(differential(a.random, a.seed), actual_model_calls=0, api_spend_usd=0))); return
    rows = cases()
    for n, args, check, error in rows:
        try: got = d.workflow(copy.deepcopy(args)); assert not error, n
        except (AssertionError, KeyError, TypeError, ValueError, StopIteration, IndexError):
            assert error, n; got = None
        if check is not None: assert check(got), (n, json.dumps(got)[:1500])
        req = dict(kind='visual-director', args=args)
        if not a.prototype_only:
            x = subprocess.run([str(ROOT / 'target/debug/gamectx'), 'local', '-'], input=r.encode(req), capture_output=True, text=True)
            assert (x.returncode != 0) == error, (n, x.stderr)
            if not error: assert x.stdout.strip() == r.encode(got), (n, x.stdout[:1500], r.encode(got)[:1500])
        if a.write_conformance:
            dd = ROOT / 'conformance/visual-director' / n; new = not dd.exists(); dd.mkdir(parents=True, exist_ok=True)
            for k, v in [('case', dict(op='local', **(dict(error=True) if error else {}))), ('input', req)] + ([] if error else [('expected', got)]):
                f = dd / (k + '.json')
                if new: f.write_text(r.encode(v) + '\n')
                else: assert f.read_text() == r.encode(v) + '\n', f
    print(r.encode(dict(workflows=len(rows), malformed=sum(e for *_, e in rows), actual_model_calls=0, api_spend_usd=0)))


if __name__ == '__main__':
    main()
