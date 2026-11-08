"""World communities cases. Expectations written before running the reference; the
`communities` local kind must match the reference byte for byte. Offline only (--cap 0).

  python3 -I eval/knowledge/communities_cases.py --cap 0 [--write-conformance] [--random N --seed S]
"""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]
import argparse, copy, random, subprocess
import communities_reference as r

ROOT = _pl.Path(__file__).resolve().parents[2]

SAMPLE_COMMUNITIES = [
    dict(
        id='ember_wardens',
        label='The Ember Wardens',
        description='Guardians of Ember Vale and the mountain pass.',
        home_places=['ember_keep', 'pass_watchtower'],
        members=['warden_kael', 'guard_bran'],
        customs=['protect_travelers', 'honor_oaths', 'no_poaching'],
        relations=[
            dict(target='iron_clan', stance='friendly', trust=40),
            dict(target='shadow_syndicate', stance='hostile', trust=-80),
            dict(target='merchant_guild', stance='allied', trust=70),
        ],
        market=dict(
            buys=[
                dict(tag='weapon', price_mult=1200),
                dict(kind='iron_ingot', price_mult=1100),
                dict(tag='food', price_mult=1000),
            ],
            sells=[
                dict(kind='torch', price_mult=800),
                dict(kind='healing_salve', price_mult=1000),
            ]
        )
    ),
    dict(
        id='iron_clan',
        label='The Iron Clan',
        description='Master miners and metalworkers of the deep crags.',
        home_places=['deep_forge', 'mine_entrance'],
        members=['smith_torin'],
        customs=['fair_weight', 'sacred_forge'],
        relations=[
            dict(target='ember_wardens', stance='friendly', trust=35),
        ],
        market=dict(
            buys=[
                dict(tag='food', price_mult=1300),
                dict(tag='precious', price_mult=1200),
            ],
            sells=[
                dict(kind='iron_ingot', price_mult=900),
                dict(kind='smith_hammer', price_mult=1000),
            ]
        )
    ),
    dict(
        id='neutral_nomads',
        label='Valley Nomads',
        description='Roaming wanderers open to fair exchange.',
        home_places=['vale_camp'],
        members=['nomad_ria'],
        customs=['hospitality_to_strangers'],
        relations=[],
        market=dict(
            buys=[],
            sells=[]
        )
    )
]

SAMPLE_THINGS = dict(
    version=1,
    materials=[
        dict(id='iron', label='Iron'),
        dict(id='wood', label='Timber'),
        dict(id='herb', label='Wild Herbs'),
    ],
    kinds=[
        dict(id='sword', label='Iron Sword', category='weapon', materials=[], functions=[], weight=6, base_value=50, durability=100, stackable=False, tags=['weapon', 'metal']),
        dict(id='iron_ingot', label='Iron Ingot', category='material', materials=[], functions=[], weight=5, base_value=10, durability=100, stackable=True, tags=['metal']),
        dict(id='torch', label='Pitch Torch', category='tool', materials=[], functions=[], weight=1, base_value=2, durability=20, stackable=True, tags=['light']),
        dict(id='healing_salve', label='Healing Salve', category='food', materials=[], functions=[], weight=1, base_value=15, durability=50, stackable=True, tags=['medicine', 'food']),
        dict(id='bread', label='Rye Bread', category='food', materials=[], functions=[], weight=1, base_value=4, durability=10, stackable=True, tags=['food', 'ration']),
        dict(id='smith_hammer', label='Smith Hammer', category='tool', materials=[], functions=[], weight=4, base_value=20, durability=150, stackable=False, tags=['tool']),
        dict(id='gem', label='Raw Ruby', category='material', materials=[], functions=[], weight=1, base_value=100, durability=200, stackable=True, tags=['precious']),
    ],
    things=[
        dict(id='player_sword', kind='sword', location=dict(holder='player'), condition=100, owner='player', quantity=1, passengers=[]),
        dict(id='player_bread', kind='bread', location=dict(holder='player'), condition=100, owner='player', quantity=5, passengers=[]),
        dict(id='player_gem', kind='gem', location=dict(holder='player'), condition=100, owner='player', quantity=1, passengers=[]),
        dict(id='player_hammer', kind='smith_hammer', location=dict(holder='player'), condition=100, owner='player', quantity=1, passengers=[]),
        dict(id='warden_torch', kind='torch', location=dict(place='ember_keep'), condition=100, owner='ember_wardens', quantity=5, passengers=[]),
        dict(id='warden_salve', kind='healing_salve', location=dict(holder='warden_kael'), condition=100, owner='warden_kael', quantity=2, passengers=[]),
        dict(id='forge_ingots', kind='iron_ingot', location=dict(place='deep_forge'), condition=100, owner='iron_clan', quantity=10, passengers=[]),
        dict(id='smith_tool', kind='smith_hammer', location=dict(holder='smith_torin'), condition=100, owner='smith_torin', quantity=1, passengers=[]),
    ]
)


def op(name, at=0, **args): return dict(op=name, at=at, args=args)
def run(ops, communities=None, things=None):
    return dict(
        communities=communities if communities is not None else copy.deepcopy(SAMPLE_COMMUNITIES),
        things=things if things is not None else copy.deepcopy(SAMPLE_THINGS),
        ops=ops
    )


CASES = [
    (
        'check_relation_allied',
        run([op('check_relation', community='ember_wardens', target='merchant_guild')])
    ),
    (
        'check_relation_neutral_default',
        run([op('check_relation', community='ember_wardens', target='unknown_visitor')])
    ),
    (
        'check_relation_hostile',
        run([op('check_relation', community='ember_wardens', target='shadow_syndicate')])
    ),
    (
        'set_relation_updates_trust_and_stance',
        run([
            op('set_relation', at=10, community='ember_wardens', target='player', stance='friendly', trust=45, place='ember_keep'),
            op('check_relation', community='ember_wardens', target='player')
        ])
    ),
    (
        'add_member_updates_community',
        run([
            op('add_member', at=15, community='ember_wardens', actor='recruiter_jon', place='ember_keep')
        ])
    ),
    (
        'remove_member_updates_community',
        run([
            op('remove_member', at=20, community='ember_wardens', actor='guard_bran', place='ember_keep')
        ])
    ),
    (
        'set_customs_updates_community',
        run([
            op('set_customs', at=25, community='ember_wardens', customs=['defend_the_weak', 'keep_vigil'], place='ember_keep')
        ])
    ),
    (
        'appraise_fair_trade_acceptable',
        run([
            op('appraise', community='ember_wardens', actor='player',
               offer_things=['player_sword'], request_things=['warden_salve'],
               trader='warden_kael')
        ])
    ),
    (
        'appraise_insufficient_offer_counter_add_item',
        run([
            op('appraise', community='ember_wardens', actor='player',
               offer_things=['player_bread'], request_things=['warden_salve'],
               trader='warden_kael')
        ])
    ),
    (
        'appraise_insufficient_offer_counter_remove_request',
        run([
            # Player only holds player_bread, request salve and torches
            op('appraise', community='ember_wardens', actor='player',
               offer_things=['player_bread'], request_things=['warden_salve', 'warden_torch'],
               trader='warden_kael')
        ], things=dict(
            version=1,
            materials=SAMPLE_THINGS['materials'],
            kinds=SAMPLE_THINGS['kinds'],
            things=[
                dict(id='player_bread', kind='bread', location=dict(holder='player'), condition=100, owner='player', quantity=1, passengers=[]),
                dict(id='warden_torch', kind='torch', location=dict(holder='warden_kael'), condition=100, owner='warden_kael', quantity=1, passengers=[]),
                dict(id='warden_salve', kind='healing_salve', location=dict(holder='warden_kael'), condition=100, owner='warden_kael', quantity=1, passengers=[]),
            ]
        ))
    ),
    (
        'appraise_unwanted_goods_rejected',
        run([
            # ember_wardens buys weapon, iron_ingot, food. Gem is precious (not bought).
            op('appraise', community='ember_wardens', actor='player',
               offer_things=['player_gem'], request_things=['warden_salve'],
               trader='warden_kael')
        ])
    ),
    (
        'appraise_hostile_relation_rejected',
        run([
            op('appraise', community='ember_wardens', actor='shadow_agent',
               offer_things=['agent_sword'], request_things=['warden_salve'],
               trader='warden_kael')
        ], communities=[
            dict(
                id='ember_wardens',
                label='The Ember Wardens',
                description='Guardians.',
                home_places=['ember_keep'],
                members=['warden_kael'],
                customs=[],
                relations=[dict(target='shadow_agent', stance='hostile', trust=-75)],
                market=dict(buys=[dict(tag='weapon', price_mult=1000)], sells=[dict(kind='healing_salve', price_mult=1000)])
            )
        ], things=dict(
            version=1,
            materials=SAMPLE_THINGS['materials'],
            kinds=SAMPLE_THINGS['kinds'],
            things=[
                dict(id='agent_sword', kind='sword', location=dict(holder='shadow_agent'), condition=100, owner='shadow_agent', quantity=1, passengers=[]),
                dict(id='warden_salve', kind='healing_salve', location=dict(holder='warden_kael'), condition=100, owner='warden_kael', quantity=1, passengers=[]),
            ]
        ))
    ),
    (
        'barter_successful_atomic_transfer',
        run([
            op('barter', at=30, community='ember_wardens', actor='player', place='ember_keep',
               offer_things=['player_sword'], request_things=['warden_salve'],
               trader='warden_kael')
        ])
    ),
    (
        'barter_insufficient_value_fails_and_reverts',
        run([
            op('barter', at=35, community='ember_wardens', actor='player', place='ember_keep',
               offer_things=['player_bread'], request_things=['warden_salve'],
               trader='warden_kael')
        ], things=dict(
            version=1,
            materials=SAMPLE_THINGS['materials'],
            kinds=SAMPLE_THINGS['kinds'],
            things=[
                dict(id='player_bread', kind='bread', location=dict(holder='player'), condition=100, owner='player', quantity=1, passengers=[]),
                dict(id='warden_salve', kind='healing_salve', location=dict(holder='warden_kael'), condition=100, owner='warden_kael', quantity=1, passengers=[]),
            ]
        ))
    ),
    (
        'barter_with_trader_generous_trust_boost',
        run([
            # Player already has relation, offers expensive player_gem to iron_clan for cheap smith_tool
            op('set_relation', at=38, community='iron_clan', target='player', stance='friendly', trust=30, place='deep_forge'),
            op('barter', at=40, community='iron_clan', actor='player', place='deep_forge',
               offer_things=['player_gem'], request_things=['smith_tool'],
               trader='smith_torin')
        ])
    ),
    (
        'barter_hostile_refusal_reverts',
        run([
            op('set_relation', at=42, community='iron_clan', target='player', stance='hostile', trust=-70, place='deep_forge'),
            op('barter', at=45, community='iron_clan', actor='player', place='deep_forge',
               offer_things=['player_gem'], request_things=['smith_tool'],
               trader='smith_torin')
        ])
    ),
    (
        'barter_thing_not_held_fails',
        run([
            op('barter', at=50, community='ember_wardens', actor='player', place='ember_keep',
               offer_things=['forge_ingots'], request_things=['warden_salve'],
               trader='warden_kael')
        ])
    ),
    (
        'barter_thing_not_available_fails',
        run([
            op('barter', at=55, community='ember_wardens', actor='player', place='ember_keep',
               offer_things=['player_sword'], request_things=['forge_ingots'],
               trader='warden_kael')
        ])
    ),
    (
        'multi_op_workflow_chain',
        run([
            op('check_relation', community='ember_wardens', target='iron_clan'),
            op('set_relation', at=60, community='ember_wardens', target='iron_clan', stance='allied', trust=85, place='ember_keep'),
            op('add_member', at=65, community='ember_wardens', actor='novice_ella', place='ember_keep'),
            op('set_customs', at=70, community='ember_wardens', customs=['honor_allies', 'eternal_watch'], place='ember_keep'),
            op('check_relation', community='ember_wardens', target='iron_clan')
        ])
    ),
    (
        'check_relation_with_numeric_trust_override',
        run([
            op('set_relation', at=75, community='iron_clan', target='outcast', stance='wary', trust=-40, place='deep_forge'),
            op('check_relation', community='iron_clan', target='outcast')
        ])
    ),
    (
        'barter_multiple_items_trade',
        run([
            op('barter', at=80, community='ember_wardens', actor='player', place='ember_keep',
               offer_things=['player_sword', 'player_bread'], request_things=['warden_salve', 'warden_torch'],
               trader='warden_kael')
        ])
    ),
    (
        'market_buys_and_sells_rule_matching',
        run([
            op('appraise', community='iron_clan', actor='player',
               offer_things=['player_bread'], request_things=['forge_ingots'])
        ])
    ),
    (
        'empty_market_accepts_all_at_default',
        run([
            op('appraise', community='neutral_nomads', actor='player',
               offer_things=['player_gem'], request_things=['nomad_bread'])
        ], things=dict(
            version=1,
            materials=SAMPLE_THINGS['materials'],
            kinds=SAMPLE_THINGS['kinds'],
            things=[
                dict(id='player_gem', kind='gem', location=dict(holder='player'), condition=100, owner='player', quantity=1, passengers=[]),
                dict(id='nomad_bread', kind='bread', location=dict(holder='nomad_ria'), condition=100, owner='neutral_nomads', quantity=2, passengers=[]),
            ]
        ))
    ),
    (
        'set_relation_creates_new_relation',
        run([
            op('set_relation', at=85, community='neutral_nomads', target='traveler', stance='friendly', trust=50, place='vale_camp'),
            op('check_relation', community='neutral_nomads', target='traveler')
        ])
    ),
    (
        'barter_updates_timeline_entry',
        run([
            op('barter', at=90, community='neutral_nomads', actor='player', place='vale_camp',
               offer_things=['player_gem'], request_things=['nomad_bread'])
        ], things=dict(
            version=1,
            materials=SAMPLE_THINGS['materials'],
            kinds=SAMPLE_THINGS['kinds'],
            things=[
                dict(id='player_gem', kind='gem', location=dict(holder='player'), condition=100, owner='player', quantity=1, passengers=[]),
                dict(id='nomad_bread', kind='bread', location=dict(holder='nomad_ria'), condition=100, owner='neutral_nomads', quantity=2, passengers=[]),
            ]
        ))
    ),
    (
        'add_existing_member_is_idempotent',
        run([
            op('add_member', at=95, community='ember_wardens', actor='warden_kael', place='ember_keep')
        ])
    ),
    (
        'remove_nonexistent_member_is_idempotent',
        run([
            op('remove_member', at=100, community='ember_wardens', actor='unknown_person', place='ember_keep')
        ])
    ),
    (
        'appraise_exact_value_trade',
        run([
            op('appraise', community='neutral_nomads', actor='player',
               offer_things=['player_hammer'], request_things=['nomad_hammers'])
        ], things=dict(
            version=1,
            materials=SAMPLE_THINGS['materials'],
            kinds=SAMPLE_THINGS['kinds'],
            things=[
                dict(id='player_hammer', kind='smith_hammer', location=dict(holder='player'), condition=100, owner='player', quantity=1, passengers=[]),
                dict(id='nomad_hammers', kind='smith_hammer', location=dict(holder='nomad_ria'), condition=100, owner='neutral_nomads', quantity=1, passengers=[]),
            ]
        ))
    ),
]

MALFORMED = [
    ('error_unknown_key', dict(unknown='bad', ops=[])),
    ('error_bad_op_name', dict(ops=[dict(op='nonexistent_op')])),
    ('error_too_many_ops', dict(ops=[dict(op='check_relation', args=dict(community='c', target='t'))] * 65)),
    ('error_bad_stance', dict(communities=[
        dict(id='c', label='C', description='D', relations=[dict(target='t', stance='invalid_stance')])
    ], ops=[])),
    ('error_bad_trust_range', dict(communities=[
        dict(id='c', label='C', description='D', relations=[dict(target='t', stance='friendly', trust=150)])
    ], ops=[])),
    ('error_duplicate_community_id', dict(communities=[
        dict(id='c', label='C1', description='D'),
        dict(id='c', label='C2', description='D'),
    ], ops=[])),
    ('error_duplicate_relation_target', dict(communities=[
        dict(id='c', label='C', description='D', relations=[
            dict(target='t', stance='friendly'),
            dict(target='t', stance='neutral'),
        ])
    ], ops=[])),
    ('error_rule_both_kind_and_tag', dict(communities=[
        dict(id='c', label='C', description='D', market=dict(
            buys=[dict(kind='k', tag='t')]
        ))
    ], ops=[])),
]


def random_workflow(seed):
    rng = random.Random(seed)
    num_ops = rng.randint(1, 6)
    ops = []
    comm_ids = ['ember_wardens', 'iron_clan', 'neutral_nomads']
    targets = ['player', 'iron_clan', 'shadow_syndicate', 'stranger_1', 'smith_torin']
    places = ['ember_keep', 'deep_forge', 'vale_camp']
    stances = r.STANCES

    for _ in range(num_ops):
        op_name = rng.choice(['check_relation', 'set_relation', 'add_member', 'remove_member', 'set_customs', 'appraise', 'barter'])
        cid = rng.choice(comm_ids)
        at = rng.randint(0, 1000)

        if op_name == 'check_relation':
            target = rng.choice(targets)
            ops.append(op(op_name, at=at, community=cid, target=target))
        elif op_name == 'set_relation':
            target = rng.choice(targets)
            if target == cid: target = 'player'
            st = rng.choice(stances)
            tr = rng.randint(-100, 100) if rng.random() < 0.7 else None
            ops.append(op(op_name, at=at, community=cid, target=target, stance=st, trust=tr, place=rng.choice(places)))
        elif op_name == 'add_member':
            ops.append(op(op_name, at=at, community=cid, actor=rng.choice(targets), place=rng.choice(places)))
        elif op_name == 'remove_member':
            ops.append(op(op_name, at=at, community=cid, actor=rng.choice(targets), place=rng.choice(places)))
        elif op_name == 'set_customs':
            custs = rng.sample(['honor_oaths', 'defend_home', 'fair_trade', 'sacred_forge', 'open_gates'], rng.randint(1, 3))
            ops.append(op(op_name, at=at, community=cid, customs=custs, place=rng.choice(places)))
        elif op_name in ('appraise', 'barter'):
            p_items = rng.sample(['player_sword', 'player_bread', 'player_gem', 'player_hammer'], rng.randint(1, 2))
            trader = 'warden_kael' if cid == 'ember_wardens' else ('smith_torin' if cid == 'iron_clan' else None)
            if cid == 'ember_wardens':
                req_items = rng.sample(['warden_torch', 'warden_salve'], rng.randint(1, 2))
            elif cid == 'iron_clan':
                req_items = rng.sample(['forge_ingots', 'smith_tool'], rng.randint(1, 2))
            else:
                req_items = ['warden_torch']
            ops.append(op(op_name, at=at, community=cid, actor='player', place=rng.choice(places),
                          offer_things=p_items, request_things=req_items, trader=trader))

    return run(ops)


def run_local(payload):
    proc = subprocess.run(
        [str(ROOT / 'target' / 'debug' / 'gamectx'), 'local', '-'],
        input=r.enc(dict(kind='communities', args=payload)).encode('utf-8') + b'\n',
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True
    )
    line = proc.stdout.strip().split(b'\n')[-1]
    return r.json.loads(line.decode('utf-8'))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--cap', type=int, default=0)
    parser.add_argument('--write-conformance', action='store_true')
    parser.add_argument('--random', type=int, default=0)
    parser.add_argument('--seed', type=int, default=7)
    args = parser.parse_args()

    conf_dir = ROOT / 'conformance' / 'communities'
    if args.write_conformance:
        conf_dir.mkdir(parents=True, exist_ok=True)
        for name, payload in CASES:
            case_d = conf_dir / name
            case_d.mkdir(exist_ok=True)
            res = r.workflow(payload)
            req = dict(kind='communities', args=payload)
            (case_d / 'case.json').write_text(r.enc(dict(op='local')) + '\n')
            (case_d / 'input.json').write_text(r.enc(req) + '\n')
            (case_d / 'expected.json').write_text(r.enc(res) + '\n')
        for name, payload in MALFORMED:
            case_d = conf_dir / name
            case_d.mkdir(exist_ok=True)
            req = dict(kind='communities', args=payload)
            (case_d / 'case.json').write_text(r.enc(dict(op='local', error=True)) + '\n')
            (case_d / 'input.json').write_text(r.enc(req) + '\n')
        print(r.enc(dict(workflows=len(CASES), malformed=len(MALFORMED), actual_model_calls=0, api_spend_usd=0)))
        return

    if args.random > 0:
        mismatches = 0
        for i in range(args.random):
            seed = args.seed + i
            wf = random_workflow(seed)
            expected = r.workflow(wf)
            actual = run_local(wf)
            if r.enc(expected) != r.enc(actual):
                mismatches += 1
                if mismatches <= 3:
                    print(f"Mismatch at seed {seed}:\nExpected: {r.enc(expected)}\nActual:   {r.enc(actual)}")
        print(r.enc(dict(workflows=args.random, mismatches=mismatches, actual_model_calls=0, api_spend_usd=0)))
        if mismatches > 0:
            _sys.exit(1)
        return

    # Check conformance cases
    for name, payload in CASES:
        expected = r.workflow(payload)
        actual = run_local(payload)
        assert r.enc(expected) == r.enc(actual), f"Mismatch in {name}:\nExp: {r.enc(expected)}\nAct: {r.enc(actual)}"

    for name, payload in MALFORMED:
        proc = subprocess.run(
            [str(ROOT / 'target' / 'debug' / 'gamectx'), 'local', '-'],
            input=r.enc(dict(kind='communities', args=payload)).encode('utf-8') + b'\n',
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        assert proc.returncode != 0 or b'"error"' in proc.stdout, f"Malformed case {name} succeeded!"

    print(r.enc(dict(workflows=len(CASES), malformed=len(MALFORMED), actual_model_calls=0, api_spend_usd=0)))


if __name__ == '__main__':
    main()
