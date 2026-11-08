"""Witness bridge cases: world events to Life experiences.
Expectations were written before running the reference; the `witness` local kind
must match the reference byte for byte. Offline only (--cap 0).

  python3 -I eval/character/witness_cases.py --cap 0 [--write-conformance] [--random N --seed S]
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
import argparse, copy, random, subprocess
import witness_reference as r

ROOT = _pl.Path(__file__).resolve().parents[2]


def op(name, **kwargs):
    return dict(op=name, args=kwargs)


def run(ops, **kwargs):
    req = dict(ops=ops)
    req.update(kwargs)
    return req


CASES = [
    # 1. Baker witnesses cart crash on the road (Baker Scene step 4)
    (
        "baker_witnesses_cart_crash",
        run([
            op(
                'witness',
                entry=dict(
                    at=100,
                    kind='crash',
                    summary='A grain cart overturned in the road.',
                    place='road',
                    actors=['carter'],
                    witnesses=['baker'],
                    visibility='witnessed',
                    noise=40,
                ),
                beings=[
                    dict(id='baker', place='road', senses=dict(sight=True, hearing=True), token=101),
                    dict(id='apprentice', place='mill', senses=dict(sight=True, hearing=True, occluded=True), token=102),
                ],
            )
        ]),
        lambda out: (
            out['results'][0]['outcomes'][0]['being'] == 'apprentice' and
            out['results'][0]['outcomes'][0]['perceived'] is False and
            out['results'][0]['outcomes'][1]['being'] == 'baker' and
            out['results'][0]['outcomes'][1]['perceived'] is True and
            out['results'][0]['outcomes'][1]['experience']['kind'] == 'direct' and
            out['results'][0]['outcomes'][1]['experience']['token'] == 101
        ),
        False,
    ),

    # 2. Tell crash report from Baker to Player (Baker Scene step 7)
    (
        "baker_tells_player_report",
        run([
            op(
                'tell',
                teller='baker',
                listener='player',
                event=dict(
                    token=101,
                    at=100,
                    kind='crash',
                    tag='crash',
                    subject='carter',
                    summary='A grain cart overturned in the road.',
                ),
                at=120,
                token=201,
                teller_place='bakery',
                listener_place='bakery',
            )
        ]),
        lambda out: (
            out['results'][0]['ok'] is True and
            out['results'][0]['experience']['kind'] == 'report' and
            out['results'][0]['experience']['token'] == 201 and
            out['results'][0]['experience']['at'] == 120 and
            out['results'][0]['experience']['source']['speaker'] == 'baker' and
            out['results'][0]['experience']['causes'] == [101]
        ),
        False,
    ),

    # 3. Module outcome from recipe: smith crafting sword
    (
        "recipe_outcome_witness",
        run([
            op(
                'witness',
                entry=dict(
                    entry=dict(
                        at=50,
                        kind='crafting',
                        summary='smith crafted iron_sword at forge.',
                        place='forge',
                        actors=['smith'],
                        witnesses=['apprentice'],
                        visibility='witnessed',
                        noise=30,
                    ),
                    ok=True,
                    recipe='craft_sword',
                ),
                beings=[
                    dict(id='smith', place='forge', senses=dict(sight=True), token=1),
                    dict(id='apprentice', place='forge', senses=dict(sight=True), token=2),
                    dict(id='guard', place='gatehouse', senses=dict(sight=True), token=3),
                ],
            )
        ]),
        lambda out: (
            out['results'][0]['perceived_count'] == 2 and
            out['results'][0]['outcomes'][0]['being'] == 'apprentice' and out['results'][0]['outcomes'][0]['perceived'] is True and
            out['results'][0]['outcomes'][1]['being'] == 'guard' and out['results'][0]['outcomes'][1]['perceived'] is False and
            out['results'][0]['outcomes'][2]['being'] == 'smith' and out['results'][0]['outcomes'][2]['perceived'] is True
        ),
        False,
    ),

    # 4. Private event: only actor perceives, co-located bystander does not
    (
        "private_event_actor_only",
        run([
            op(
                'witness',
                entry=dict(
                    at=70,
                    kind='secret_pact',
                    summary='A quiet whispered promise.',
                    place='tavern_corner',
                    actors=['spy_1', 'spy_2'],
                    visibility='private',
                    noise=5,
                ),
                beings=[
                    dict(id='spy_1', place='tavern_corner', token=10),
                    dict(id='eavesdropper', place='tavern_corner', token=11),
                ],
            )
        ]),
        lambda out: (
            out['results'][0]['outcomes'][0]['being'] == 'eavesdropper' and
            out['results'][0]['outcomes'][0]['perceived'] is False and
            out['results'][0]['outcomes'][0]['reason'] == 'private_not_actor' and
            out['results'][0]['outcomes'][1]['being'] == 'spy_1' and
            out['results'][0]['outcomes'][1]['perceived'] is True
        ),
        False,
    ),

    # 5. Sleeping character ignores quiet noise
    (
        "asleep_ignores_quiet_event",
        run([
            op(
                'witness',
                entry=dict(
                    at=80,
                    kind='sneak',
                    summary='A thief stepped through the hallway.',
                    place='bedroom',
                    actors=['thief'],
                    visibility='witnessed',
                    noise=15,
                ),
                beings=[
                    dict(id='sleeper', place='bedroom', senses=dict(conditions=['asleep']), token=1),
                ],
            )
        ]),
        lambda out: (
            out['results'][0]['outcomes'][0]['perceived'] is False and
            out['results'][0]['outcomes'][0]['reason'] == 'asleep_noise_too_low'
        ),
        False,
    ),

    # 6. Sleeping character wakes to loud noise (>= 80)
    (
        "asleep_wakes_to_loud_explosion",
        run([
            op(
                'witness',
                entry=dict(
                    at=85,
                    kind='explosion',
                    summary='A thunderous blast shattered the window.',
                    place='bedroom',
                    actors=['blaster'],
                    visibility='public',
                    noise=90,
                ),
                beings=[
                    dict(id='sleeper', place='bedroom', senses=dict(conditions=['asleep']), token=2),
                ],
            )
        ]),
        lambda out: (
            out['results'][0]['outcomes'][0]['perceived'] is True and
            out['results'][0]['outcomes'][0]['experience']['kind'] == 'direct'
        ),
        False,
    ),

    # 7. Unconscious character never perceives even loud event
    (
        "unconscious_never_perceives",
        run([
            op(
                'witness',
                entry=dict(
                    at=90,
                    kind='gong',
                    summary='A loud gong rang out.',
                    place='temple',
                    actors=['monk'],
                    visibility='public',
                    noise=95,
                ),
                beings=[
                    dict(id='patient', place='temple', senses=dict(conditions=['unconscious']), token=3),
                ],
            )
        ]),
        lambda out: (
            out['results'][0]['outcomes'][0]['perceived'] is False and
            out['results'][0]['outcomes'][0]['reason'] == 'unconscious'
        ),
        False,
    ),

    # 8. Dead character never perceives
    (
        "dead_never_perceives",
        run([
            op(
                'witness',
                entry=dict(
                    at=95,
                    kind='ceremony',
                    summary='A solemn chant.',
                    place='tomb',
                    actors=['priest'],
                    visibility='public',
                    noise=50,
                ),
                beings=[
                    dict(id='ghost', place='tomb', senses=dict(conditions=['dead']), token=4),
                ],
            )
        ]),
        lambda out: (
            out['results'][0]['outcomes'][0]['perceived'] is False and
            out['results'][0]['outcomes'][0]['reason'] == 'dead'
        ),
        False,
    ),

    # 9. Loud public event heard at distance
    (
        "loud_public_event_heard_at_distance",
        run([
            op(
                'witness',
                entry=dict(
                    at=105,
                    kind='alarm_bell',
                    summary='The town alarm bell rang furiously.',
                    place='belltower',
                    actors=['watchman'],
                    visibility='public',
                    noise=75,
                ),
                beings=[
                    dict(id='villager', place='farmland', senses=dict(hearing=True), token=5),
                ],
            )
        ]),
        lambda out: (
            out['results'][0]['outcomes'][0]['perceived'] is True and
            out['results'][0]['outcomes'][0]['reason'] == 'heard_at_distance'
        ),
        False,
    ),

    # 10. Deaf character co-located with occluded event
    (
        "deaf_occluded_cannot_perceive",
        run([
            op(
                'witness',
                entry=dict(
                    at=110,
                    kind='rattle',
                    summary='A wooden box rattled.',
                    place='cellar',
                    actors=['rat'],
                    visibility='witnessed',
                    noise=25,
                ),
                beings=[
                    dict(id='deaf_worker', place='cellar', senses=dict(hearing=False, occluded=True), token=6),
                ],
            )
        ]),
        lambda out: (
            out['results'][0]['outcomes'][0]['perceived'] is False and
            out['results'][0]['outcomes'][0]['reason'] == 'occluded_cannot_sense'
        ),
        False,
    ),

    # 11. Species module outcome (feeding animal)
    (
        "species_outcome_witness",
        run([
            op(
                'witness',
                entry=dict(
                    entry=dict(
                        at=115,
                        kind='feed',
                        summary='hero fed canine_1 at kennel.',
                        place='kennel',
                        actors=['hero'],
                        witnesses=['kennel_master'],
                        visibility='witnessed',
                        noise=10,
                    ),
                    ok=True,
                    op='feed',
                ),
                beings=[
                    dict(id='hero', place='kennel', token=10),
                    dict(id='kennel_master', place='kennel', token=11),
                ],
            )
        ]),
        lambda out: out['results'][0]['perceived_count'] == 2,
        False,
    ),

    # 12. Communities barter outcome
    (
        "barter_outcome_witness",
        run([
            op(
                'witness',
                entry=dict(
                    entry=dict(
                        at=125,
                        kind='barter',
                        summary='hero bartered with merchants at square.',
                        place='square',
                        actors=['hero', 'merchants'],
                        visibility='witnessed',
                        noise=20,
                    ),
                    ok=True,
                ),
                beings=[
                    dict(id='hero', place='square', token=15),
                    dict(id='merchants', place='square', token=16),
                ],
            )
        ]),
        lambda out: out['results'][0]['perceived_count'] == 2,
        False,
    ),

    # 13. Things module outcome (create thing)
    (
        "things_outcome_witness",
        run([
            op(
                'witness',
                entry=dict(
                    entry=dict(
                        at=130,
                        kind='create',
                        summary='smith created iron_anvil at forge.',
                        place='forge',
                        actors=['smith'],
                        visibility='witnessed',
                        noise=30,
                    ),
                    ok=True,
                ),
                beings=[
                    dict(id='smith', place='forge', token=20),
                ],
            )
        ]),
        lambda out: out['results'][0]['perceived_count'] == 1 and out['results'][0]['outcomes'][0]['experience']['tag'] == 'create',
        False,
    ),

    # 14. Multiple ops in single request: witness then tell
    (
        "workflow_witness_and_tell",
        run([
            op(
                'witness',
                entry=dict(
                    at=140,
                    kind='storm',
                    summary='Lightning struck the tower.',
                    place='tower',
                    actors=['nature'],
                    visibility='public',
                    noise=80,
                ),
                beings=[dict(id='wizard', place='tower', token=30)],
            ),
            op(
                'tell',
                teller='wizard',
                listener='apprentice',
                event=dict(token=30, at=140, kind='storm', summary='Lightning struck the tower.'),
                at=145,
                token=31,
                teller_place='sanctum',
                listener_place='sanctum',
            ),
        ]),
        lambda out: len(out['results']) == 2 and out['results'][1]['ok'] is True,
        False,
    ),

    # 15. Default token counter allocation
    (
        "default_token_allocation",
        run([
            op(
                'witness',
                entry=dict(
                    at=150,
                    kind='song',
                    summary='A minstrel sang a ballad.',
                    place='tavern',
                    actors=['minstrel'],
                    visibility='public',
                    noise=30,
                ),
                beings=[
                    dict(id='patron_a', place='tavern'),
                    dict(id='patron_b', place='tavern'),
                ],
            )
        ]),
        lambda out: (
            out['results'][0]['outcomes'][0]['experience']['token'] <
            out['results'][0]['outcomes'][1]['experience']['token']
        ),
        False,
    ),

    # 16. Tell requires different teller and listener
    (
        "malformed_tell_self",
        run([
            op(
                'tell',
                teller='solo',
                listener='solo',
                event=dict(summary='Note to self.'),
            )
        ]),
        None,
        True,
    ),

    # 17. Tell in different places fails
    (
        "malformed_tell_different_places",
        run([
            op(
                'tell',
                teller='alice',
                listener='bob',
                event=dict(summary='Hello.'),
                teller_place='castle',
                listener_place='forest',
            )
        ]),
        None,
        True,
    ),

    # 18. Malformed: unknown op
    (
        "malformed_unknown_op",
        run([op('teleport')]),
        None,
        True,
    ),

    # 19. Malformed: missing entry in witness
    (
        "malformed_missing_entry",
        run([op('witness', beings=[])]),
        None,
        True,
    ),

    # 20. Malformed: bad teller ident
    (
        "malformed_bad_teller_ident",
        run([
            op(
                'tell',
                teller='bad id with space',
                listener='bob',
                event=dict(summary='msg'),
            )
        ]),
        None,
        True,
    ),

    # 21. Malformed: bad listener ident
    (
        "malformed_bad_listener_ident",
        run([
            op(
                'tell',
                teller='alice',
                listener='bad!ident',
                event=dict(summary='msg'),
            )
        ]),
        None,
        True,
    ),

    # 22. Malformed: ops exceeding MAX_OPS
    (
        "malformed_too_many_ops",
        run([op('tell', teller='a', listener='b', event=dict()) for _ in range(65)]),
        None,
        True,
    ),

    # 23. Malformed: beings exceeding MAX_BEINGS
    (
        "malformed_too_many_beings",
        run([
            op(
                'witness',
                entry=dict(at=10, kind='test', summary='test'),
                beings=[dict(id=f'b_{i}') for i in range(65)],
            )
        ]),
        None,
        True,
    ),

    # 24. Malformed: missing op field
    (
        "malformed_missing_op",
        dict(ops=[dict(args=dict())]),
        None,
        True,
    ),

    # 25. Malformed: negative clock in entry
    (
        "malformed_negative_clock",
        run([
            op(
                'witness',
                entry=dict(at=-10, kind='bad', summary='bad'),
                beings=[],
            )
        ]),
        None,
        True,
    ),
]


def random_workflow(rng):
    ops = []
    num_ops = rng.randint(1, 4)
    places = ['square', 'bakery', 'mill', 'road', 'forest']
    beings_pool = ['baker', 'miller', 'player', 'guard', 'smith', 'apprentice']

    for _ in range(num_ops):
        op_choice = rng.choice(['witness', 'tell'])
        if op_choice == 'witness':
            evt_place = rng.choice(places)
            vis = rng.choice(['public', 'witnessed', 'private'])
            actors = [rng.choice(beings_pool)]
            witnesses = [rng.choice(beings_pool)]
            noise = rng.randint(5, 95)
            at = rng.randint(10, 500)
            kind = rng.choice(['crash', 'craft', 'barter', 'feed', 'travel', 'fire'])

            selected_beings = []
            for b_id in sorted(rng.sample(beings_pool, rng.randint(1, 4))):
                b_place = evt_place if rng.random() < 0.6 else rng.choice(places)
                conds = []
                if rng.random() < 0.15:
                    conds.append('asleep')
                elif rng.random() < 0.05:
                    conds.append('unconscious')
                selected_beings.append(dict(
                    id=b_id,
                    place=b_place,
                    senses=dict(
                        sight=True,
                        hearing=True,
                        occluded=(b_place != evt_place and rng.random() < 0.5),
                        conditions=conds,
                    ),
                    token=rng.randint(1000, 9999),
                ))

            entry = dict(
                at=at,
                kind=kind,
                summary=f"A {kind} occurred at {evt_place}.",
                place=evt_place,
                actors=actors,
                witnesses=witnesses,
                visibility=vis,
                noise=noise,
            )
            ops.append(op('witness', entry=entry, beings=selected_beings))
        else: # tell
            teller, listener = rng.sample(beings_pool, 2)
            common_place = rng.choice(places)
            at = rng.randint(10, 500)
            token = rng.randint(1000, 9999)
            ops.append(op(
                'tell',
                teller=teller,
                listener=listener,
                event=dict(
                    token=rng.randint(100, 999),
                    at=at - 10,
                    kind='event',
                    summary=f"Event told by {teller}.",
                ),
                at=at,
                token=token,
                teller_place=common_place,
                listener_place=common_place,
            ))

    return run(ops)


def call(req):
    return subprocess.run([str(ROOT / 'target/debug/gamectx'), 'local', '-'], input=r.enc(req), capture_output=True, text=True)


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--cap', type=float, required=True)
    p.add_argument('--prototype-only', action='store_true')
    p.add_argument('--write-conformance', action='store_true')
    p.add_argument('--random', type=int, default=0)
    p.add_argument('--seed', type=int, default=7)
    a = p.parse_args()

    if a.random > 0:
        rng = random.Random(a.seed)
        mismatches = 0
        for _ in range(a.random):
            args = random_workflow(rng)
            req = dict(kind='witness', args=args)
            got = r.workflow(copy.deepcopy(args))
            if not a.prototype_only:
                x = call(req)
                if x.returncode != 0 or x.stdout.strip() != r.enc(got):
                    mismatches += 1
                    print('MISMATCH', r.enc(args)[:600])
                    print('Ref :', r.enc(got)[:600])
                    print('Rust:', x.stdout.strip()[:600])
                    print('Err :', x.stderr[:600])
                    assert False, "differential fuzzing mismatch"
        print(r.enc(dict(workflows=a.random, mismatches=mismatches, actual_model_calls=0, api_spend_usd=0)))
        return

    for n, args, check, error in CASES:
        try:
            got = r.workflow(copy.deepcopy(args))
            assert not error, n
        except (AssertionError, KeyError, TypeError, ValueError, StopIteration, IndexError):
            assert error, n
            got = None
        if check is not None:
            assert check(got), (n, r.enc(got)[:1500])
        req = dict(kind='witness', args=args)
        if not a.prototype_only:
            x = call(req)
            assert (x.returncode != 0) == error, (n, x.stderr)
            if not error:
                assert x.stdout.strip() == r.enc(got), (n, x.stdout[:1500], r.enc(got)[:1500])
        if a.write_conformance:
            dd = ROOT / 'conformance/witness' / n
            new = not dd.exists()
            dd.mkdir(parents=True, exist_ok=True)
            for k, v in [('case', dict(op='local', **(dict(error=True) if error else {}))), ('input', req)] + ([] if error else [('expected', got)]):
                f = dd / (k + '.json')
                if new:
                    f.write_text(r.enc(v) + '\n')
                else:
                    assert f.read_text() == r.enc(v) + '\n', f
    print(r.enc(dict(workflows=len(CASES), malformed=sum(e for *_, e in CASES), actual_model_calls=0, api_spend_usd=0)))


if __name__ == '__main__':
    main()
