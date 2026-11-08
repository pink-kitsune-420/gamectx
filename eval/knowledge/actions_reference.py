"""Independent original action catalog reference: read-only description over existing ops.
Python standard library only; nothing here calls a model.

The catalog covers 11 operational surfaces:
things, recipes, species, communities, inventory, fitness, props, services, reactions,
engine (directions/commands), initiative.

Schema per action:
  id: str (ident)
  label: str
  category: str (one of 11 surfaces)
  description: str
  actor: {species: [str], skills: [{skill, level}], body_state: {max_fatigue, min_condition, disallowed_conditions: [str]}, membership: str|None}
  targets: [{name: str, kind: str, required: bool, description: str}]
  preconditions: {same_place: bool, held: bool, in_reach: bool}
  cost: {time: int, fatigue: int, resources: [{kind, qty}], coins: int}
  effects: {world: str, body: {fatigue_delta: int}, needs: [{need, delta: int}], skills: [{skill, practice: int}], relationships: [{target_slot, attitude, delta: int}]}
  perception: {visibility: 'public'|'witnessed'|'private', noise: int, witnesses: str}
  risk: {failure_chance: int, on_failure: str}
  duration: int
  interruptible: bool
"""
import copy, json, pathlib, re, sys
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / 'character'))
from limits_reference import Limits, limited

MAX_ACTIONS = 256
MAX_OPS = 64
MAX_SAFE = 9007199254740991
SPEC = [('actions', MAX_ACTIONS, 65536)]

CATEGORIES = [
    'things',
    'recipes',
    'species',
    'communities',
    'inventory',
    'fitness',
    'props',
    'services',
    'reactions',
    'engine',
    'initiative',
]

TARGET_KINDS = [
    'thing',
    'person',
    'animal',
    'place',
    'community',
    'prop',
    'recipe',
    'service',
    'direction',
]

VISIBILITIES = ['public', 'witnessed', 'private']


def enc(v):
    return json.dumps(v, ensure_ascii=False, separators=(',', ':'))


def control(s):
    return any(ord(c) < 32 or 127 <= ord(c) <= 159 for c in s)


def text(s, chars):
    assert isinstance(s, str) and 1 <= len(s) <= chars and not control(s)
    return s


def ident(s):
    assert isinstance(s, str) and re.fullmatch('[A-Za-z0-9_:-]{1,64}', s)
    return s


def uint(n, lo=0, hi=MAX_SAFE):
    assert type(n) is int and lo <= n <= hi
    return n


def integer(n, lo=-MAX_SAFE, hi=MAX_SAFE):
    assert type(n) is int and lo <= n <= hi
    return n


def boolean(b):
    assert type(b) is bool
    return b


def keys(v, required, optional=()):
    assert isinstance(v, dict) and set(required) <= set(v) <= set(required) | set(optional)


def validate_target_slot(ts):
    keys(ts, ['name', 'kind', 'required'], ['description'])
    ident(ts['name'])
    assert ts['kind'] in TARGET_KINDS, f"unknown target kind {ts['kind']}"
    boolean(ts['required'])
    if 'description' in ts and ts['description'] is not None:
        text(ts['description'], 256)


def validate_actor(actor):
    keys(actor, [], ['species', 'skills', 'body_state', 'membership'])
    if 'species' in actor and actor['species'] is not None:
        assert isinstance(actor['species'], list) and len(actor['species']) <= 16
        for sp in actor['species']:
            ident(sp)
    if 'skills' in actor and actor['skills'] is not None:
        assert isinstance(actor['skills'], list) and len(actor['skills']) <= 16
        for sk in actor['skills']:
            keys(sk, ['skill', 'level'])
            ident(sk['skill'])
            uint(sk['level'], 0, 100)
    if 'body_state' in actor and actor['body_state'] is not None:
        bs = actor['body_state']
        keys(bs, [], ['max_fatigue', 'min_condition', 'disallowed_conditions'])
        if 'max_fatigue' in bs and bs['max_fatigue'] is not None:
            uint(bs['max_fatigue'], 0, 100)
        if 'min_condition' in bs and bs['min_condition'] is not None:
            uint(bs['min_condition'], 0, 100)
        if 'disallowed_conditions' in bs and bs['disallowed_conditions'] is not None:
            assert isinstance(bs['disallowed_conditions'], list) and len(bs['disallowed_conditions']) <= 16
            for c in bs['disallowed_conditions']:
                ident(c)
    if 'membership' in actor and actor['membership'] is not None:
        ident(actor['membership'])


def validate_preconditions(pre):
    keys(pre, [], ['same_place', 'held', 'in_reach'])
    if 'same_place' in pre and pre['same_place'] is not None:
        boolean(pre['same_place'])
    if 'held' in pre and pre['held'] is not None:
        boolean(pre['held'])
    if 'in_reach' in pre and pre['in_reach'] is not None:
        boolean(pre['in_reach'])


def validate_cost(c):
    keys(c, ['time', 'fatigue'], ['resources', 'coins'])
    uint(c['time'], 0, 1000000)
    uint(c['fatigue'], 0, 100)
    if 'resources' in c and c['resources'] is not None:
        assert isinstance(c['resources'], list) and len(c['resources']) <= 16
        for r in c['resources']:
            keys(r, ['kind', 'qty'])
            ident(r['kind'])
            uint(r['qty'], 1, 1000000)
    if 'coins' in c and c['coins'] is not None:
        uint(c['coins'], 0, 1000000000)


def validate_effects(eff):
    keys(eff, ['world'], ['body', 'needs', 'skills', 'relationships'])
    text(eff['world'], 256)
    if 'body' in eff and eff['body'] is not None:
        keys(eff['body'], [], ['fatigue_delta', 'condition_delta'])
        if 'fatigue_delta' in eff['body'] and eff['body']['fatigue_delta'] is not None:
            integer(eff['body']['fatigue_delta'], -100, 100)
        if 'condition_delta' in eff['body'] and eff['body']['condition_delta'] is not None:
            integer(eff['body']['condition_delta'], -100, 100)
    if 'needs' in eff and eff['needs'] is not None:
        assert isinstance(eff['needs'], list) and len(eff['needs']) <= 16
        for nd in eff['needs']:
            keys(nd, ['need', 'delta'])
            ident(nd['need'])
            integer(nd['delta'], -100, 100)
    if 'skills' in eff and eff['skills'] is not None:
        assert isinstance(eff['skills'], list) and len(eff['skills']) <= 16
        for sk in eff['skills']:
            keys(sk, ['skill', 'practice'])
            ident(sk['skill'])
            uint(sk['practice'], 1, 100)
    if 'relationships' in eff and eff['relationships'] is not None:
        assert isinstance(eff['relationships'], list) and len(eff['relationships']) <= 16
        for rel in eff['relationships']:
            keys(rel, ['target_slot', 'attitude', 'delta'])
            ident(rel['target_slot'])
            ident(rel['attitude'])
            integer(rel['delta'], -100, 100)


def validate_perception(p):
    keys(p, ['visibility'], ['noise', 'witnesses'])
    assert p['visibility'] in VISIBILITIES, f"bad visibility {p['visibility']}"
    if 'noise' in p and p['noise'] is not None:
        uint(p['noise'], 0, 100)
    if 'witnesses' in p and p['witnesses'] is not None:
        text(p['witnesses'], 64)


def validate_risk(r):
    keys(r, ['failure_chance'], ['on_failure'])
    uint(r['failure_chance'], 0, 100)
    if 'on_failure' in r and r['on_failure'] is not None:
        text(r['on_failure'], 256)


def validate_action(act):
    keys(
        act,
        [
            'id',
            'label',
            'category',
            'description',
            'actor',
            'targets',
            'preconditions',
            'cost',
            'effects',
            'perception',
            'risk',
            'duration',
            'interruptible',
        ],
    )
    ident(act['id'])
    text(act['label'], 64)
    assert act['category'] in CATEGORIES, f"unknown category {act['category']}"
    text(act['description'], 256)
    validate_actor(act['actor'])
    assert isinstance(act['targets'], list) and len(act['targets']) <= 8
    target_names = set()
    for ts in act['targets']:
        validate_target_slot(ts)
        assert ts['name'] not in target_names, f"duplicate target slot name {ts['name']}"
        target_names.add(ts['name'])
    validate_preconditions(act['preconditions'])
    validate_cost(act['cost'])
    validate_effects(act['effects'])
    validate_perception(act['perception'])
    validate_risk(act['risk'])
    uint(act['duration'], 0, 1000000)
    boolean(act['interruptible'])


def make_action(
    id,
    label,
    category,
    description,
    species=None,
    skills=None,
    max_fatigue=95,
    min_condition=1,
    disallowed_conditions=None,
    membership=None,
    targets=None,
    same_place=True,
    held=False,
    in_reach=True,
    time=1,
    fatigue=1,
    resources=None,
    coins=0,
    world_effect='executed',
    fatigue_delta=1,
    condition_delta=0,
    needs=None,
    skill_practice=None,
    relationships=None,
    visibility='witnessed',
    noise=10,
    witnesses='same_place',
    failure_chance=0,
    on_failure='none',
    duration=1,
    interruptible=True,
):
    disallowed = disallowed_conditions if disallowed_conditions is not None else ['asleep', 'unconscious', 'dead']
    act = {
        'id': id,
        'label': label,
        'category': category,
        'description': description,
        'actor': {
            'species': species or ['person'],
            'skills': skills or [],
            'body_state': {
                'max_fatigue': max_fatigue,
                'min_condition': min_condition,
                'disallowed_conditions': disallowed,
            },
            'membership': membership,
        },
        'targets': targets or [],
        'preconditions': {
            'same_place': same_place,
            'held': held,
            'in_reach': in_reach,
        },
        'cost': {
            'time': time,
            'fatigue': fatigue,
            'resources': resources or [],
            'coins': coins,
        },
        'effects': {
            'world': world_effect,
            'body': {
                'fatigue_delta': fatigue_delta,
                'condition_delta': condition_delta,
            },
            'needs': needs or [],
            'skills': skill_practice or [],
            'relationships': relationships or [],
        },
        'perception': {
            'visibility': visibility,
            'noise': noise,
            'witnesses': witnesses,
        },
        'risk': {
            'failure_chance': failure_chance,
            'on_failure': on_failure,
        },
        'duration': duration,
        'interruptible': interruptible,
    }
    validate_action(act)
    return act


BUILTIN_ACTIONS = [
    # 1. Things
    make_action(
        'take',
        'Take Item',
        'things',
        'Take an item from the current place or container into holder inventory.',
        targets=[{'name': 'thing', 'kind': 'thing', 'required': True, 'description': 'Item to take'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=1,
        fatigue=1,
        world_effect='transfers thing to actor holder',
        fatigue_delta=1,
        duration=1,
    ),
    make_action(
        'store',
        'Store Item',
        'things',
        'Store a held item inside a container in the current place.',
        targets=[
            {'name': 'thing', 'kind': 'thing', 'required': True, 'description': 'Item to store'},
            {'name': 'container', 'kind': 'thing', 'required': True, 'description': 'Container to receive item'},
        ],
        same_place=True,
        held=True,
        in_reach=True,
        time=1,
        fatigue=1,
        world_effect='stores thing inside container',
        fatigue_delta=1,
        duration=1,
    ),
    make_action(
        'move',
        'Move Thing',
        'things',
        'Move an item or carrier between places.',
        targets=[
            {'name': 'thing', 'kind': 'thing', 'required': True, 'description': 'Thing to move'},
            {'name': 'destination', 'kind': 'place', 'required': True, 'description': 'Destination place'},
        ],
        same_place=True,
        held=False,
        in_reach=True,
        time=2,
        fatigue=2,
        world_effect='moves thing to destination place',
        fatigue_delta=2,
        duration=2,
    ),
    make_action(
        'load',
        'Load Cargo',
        'things',
        'Load an item or cargo into a carrier.',
        targets=[
            {'name': 'thing', 'kind': 'thing', 'required': True, 'description': 'Item to load'},
            {'name': 'carrier', 'kind': 'thing', 'required': True, 'description': 'Carrier vehicle'},
        ],
        same_place=True,
        held=False,
        in_reach=True,
        time=2,
        fatigue=2,
        world_effect='loads thing into carrier cargo',
        fatigue_delta=2,
        duration=2,
    ),
    make_action(
        'unload',
        'Unload Cargo',
        'things',
        'Unload an item from a carrier into the current place.',
        targets=[
            {'name': 'thing', 'kind': 'thing', 'required': True, 'description': 'Item to unload'},
            {'name': 'carrier', 'kind': 'thing', 'required': True, 'description': 'Carrier vehicle'},
        ],
        same_place=True,
        held=False,
        in_reach=True,
        time=2,
        fatigue=2,
        world_effect='unloads thing from carrier into place',
        fatigue_delta=2,
        duration=2,
    ),
    make_action(
        'create',
        'Create Thing',
        'things',
        'Instantiate a new thing in the world.',
        targets=[{'name': 'thing', 'kind': 'thing', 'required': True, 'description': 'Created thing instance'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=5,
        fatigue=3,
        world_effect='creates new thing instance in current place',
        fatigue_delta=3,
        duration=5,
        interruptible=False,
    ),
    make_action(
        'destroy',
        'Destroy Thing',
        'things',
        'Destroy or salvage an item from the world.',
        targets=[{'name': 'thing', 'kind': 'thing', 'required': True, 'description': 'Thing to destroy'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=2,
        fatigue=2,
        world_effect='destroys thing instance and removes it from world',
        fatigue_delta=2,
        duration=2,
        interruptible=False,
    ),
    make_action(
        'travel',
        'Travel with Carrier',
        'things',
        'Travel to a connected destination with a carrier vehicle.',
        targets=[
            {'name': 'destination', 'kind': 'place', 'required': True, 'description': 'Destination place'},
            {'name': 'carrier', 'kind': 'thing', 'required': False, 'description': 'Optional carrier vehicle'},
        ],
        same_place=True,
        held=False,
        in_reach=True,
        time=10,
        fatigue=8,
        world_effect='transports carrier and passengers to destination',
        fatigue_delta=8,
        duration=10,
    ),

    # 2. Recipes
    make_action(
        'craft',
        'Craft Item',
        'recipes',
        'Execute a crafting recipe using inputs and tools to create items.',
        targets=[
            {'name': 'recipe', 'kind': 'recipe', 'required': True, 'description': 'Crafting recipe'},
            {'name': 'station', 'kind': 'prop', 'required': False, 'description': 'Workshop station or anvil'},
        ],
        same_place=True,
        held=False,
        in_reach=True,
        time=20,
        fatigue=12,
        world_effect='consumes inputs and produces crafted outputs',
        fatigue_delta=12,
        skill_practice=[{'skill': 'crafting', 'practice': 10}],
        noise=30,
        duration=20,
    ),
    make_action(
        'cook',
        'Cook Meal',
        'recipes',
        'Cook ingredients at a cooking station to produce food.',
        targets=[
            {'name': 'recipe', 'kind': 'recipe', 'required': True, 'description': 'Cooking recipe'},
            {'name': 'station', 'kind': 'prop', 'required': False, 'description': 'Oven or campfire station'},
        ],
        same_place=True,
        held=False,
        in_reach=True,
        time=15,
        fatigue=8,
        world_effect='consumes ingredients and produces cooked rations',
        fatigue_delta=8,
        skill_practice=[{'skill': 'cooking', 'practice': 8}],
        noise=20,
        duration=15,
    ),
    make_action(
        'build',
        'Construct Structure',
        'recipes',
        'Construct a structure, footbridge, or add places/entrances.',
        targets=[{'name': 'recipe', 'kind': 'recipe', 'required': True, 'description': 'Building recipe'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=60,
        fatigue=25,
        world_effect='modifies place structure, adds links or spots',
        fatigue_delta=25,
        skill_practice=[{'skill': 'carpentry', 'practice': 15}],
        noise=40,
        duration=60,
    ),
    make_action(
        'repair',
        'Repair Item',
        'recipes',
        'Restore durability or condition of a damaged item.',
        targets=[
            {'name': 'thing', 'kind': 'thing', 'required': True, 'description': 'Damaged thing to repair'},
            {'name': 'recipe', 'kind': 'recipe', 'required': True, 'description': 'Repair recipe'},
        ],
        same_place=True,
        held=False,
        in_reach=True,
        time=15,
        fatigue=10,
        world_effect='restores thing condition to full',
        fatigue_delta=10,
        skill_practice=[{'skill': 'crafting', 'practice': 6}],
        noise=25,
        duration=15,
    ),
    make_action(
        'dismantle',
        'Dismantle Item',
        'recipes',
        'Dismantle an item into salvage materials.',
        targets=[{'name': 'thing', 'kind': 'thing', 'required': True, 'description': 'Item to dismantle'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=10,
        fatigue=6,
        world_effect='dismantles item into material components',
        fatigue_delta=6,
        skill_practice=[{'skill': 'crafting', 'practice': 4}],
        noise=20,
        duration=10,
    ),

    # 3. Species
    make_action(
        'pet',
        'Pet Animal',
        'species',
        'Gently pet a domestic or wild animal.',
        targets=[{'name': 'animal', 'kind': 'animal', 'required': True, 'description': 'Animal to pet'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=1,
        fatigue=1,
        world_effect='increases animal trust and calm',
        fatigue_delta=1,
        relationships=[{'target_slot': 'animal', 'attitude': 'trust', 'delta': 5}],
        noise=5,
        duration=1,
    ),
    make_action(
        'feed',
        'Feed Animal',
        'species',
        'Feed an animal suitable food or produce.',
        targets=[
            {'name': 'animal', 'kind': 'animal', 'required': True, 'description': 'Animal to feed'},
            {'name': 'food', 'kind': 'thing', 'required': True, 'description': 'Edible item for animal'},
        ],
        same_place=True,
        held=True,
        in_reach=True,
        time=2,
        fatigue=1,
        world_effect='consumes food and increases animal tameness and trust',
        fatigue_delta=1,
        relationships=[{'target_slot': 'animal', 'attitude': 'trust', 'delta': 10}],
        noise=10,
        duration=2,
    ),
    make_action(
        'calm',
        'Calm Animal',
        'species',
        'Calm a frightened, agitated or aggressive animal.',
        targets=[{'name': 'animal', 'kind': 'animal', 'required': True, 'description': 'Animal to calm'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=3,
        fatigue=2,
        world_effect='reduces animal fear and aggression',
        fatigue_delta=2,
        noise=5,
        duration=3,
    ),
    make_action(
        'lead',
        'Lead Animal',
        'species',
        'Lead a tamed animal along as a companion follower.',
        targets=[{'name': 'animal', 'kind': 'animal', 'required': True, 'description': 'Animal to lead'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=5,
        fatigue=3,
        world_effect='sets animal leading status to actor',
        fatigue_delta=3,
        duration=5,
    ),
    make_action(
        'ride',
        'Ride Animal Mount',
        'species',
        'Mount and ride a trained animal mount.',
        targets=[{'name': 'animal', 'kind': 'animal', 'required': True, 'description': 'Trained mount animal'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=10,
        fatigue=4,
        world_effect='sets actor riding animal mount for rapid travel',
        fatigue_delta=4,
        duration=10,
    ),
    make_action(
        'animal_train',
        'Train Animal',
        'species',
        'Train an animal to increase tameness and obedience.',
        targets=[{'name': 'animal', 'kind': 'animal', 'required': True, 'description': 'Animal to train'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=30,
        fatigue=10,
        world_effect='increases animal tameness and unlocks learned tricks',
        fatigue_delta=10,
        skill_practice=[{'skill': 'animal_handling', 'practice': 12}],
        duration=30,
    ),
    make_action(
        'harvest',
        'Harvest Produce',
        'species',
        'Collect produce from a domestic animal or plant (wool, milk, berries).',
        targets=[{'name': 'animal', 'kind': 'animal', 'required': True, 'description': 'Produce producer'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=5,
        fatigue=3,
        world_effect='harvests produce items into actor inventory',
        fatigue_delta=3,
        duration=5,
    ),
    make_action(
        'behave',
        'Animal Behavior',
        'species',
        'Spontaneous non-verbal animal gesture or sound.',
        species=['animal'],
        targets=[],
        same_place=True,
        held=False,
        in_reach=False,
        time=1,
        fatigue=0,
        world_effect='emits characteristic animal gesture and sound',
        fatigue_delta=0,
        noise=15,
        duration=1,
    ),

    # 4. Communities
    make_action(
        'add_member',
        'Add Community Member',
        'communities',
        'Induct a person into community membership.',
        targets=[
            {'name': 'community', 'kind': 'community', 'required': True, 'description': 'Community faction'},
            {'name': 'person', 'kind': 'person', 'required': True, 'description': 'Candidate member'},
        ],
        same_place=True,
        held=False,
        in_reach=True,
        time=5,
        fatigue=2,
        world_effect='records person as member of community',
        fatigue_delta=2,
        relationships=[{'target_slot': 'person', 'attitude': 'trust', 'delta': 10}],
        duration=5,
    ),
    make_action(
        'remove_member',
        'Remove Community Member',
        'communities',
        'Remove or exile a member from community membership.',
        targets=[
            {'name': 'community', 'kind': 'community', 'required': True, 'description': 'Community faction'},
            {'name': 'person', 'kind': 'person', 'required': True, 'description': 'Exiled member'},
        ],
        same_place=True,
        held=False,
        in_reach=True,
        time=5,
        fatigue=2,
        world_effect='removes person from community members',
        fatigue_delta=2,
        relationships=[{'target_slot': 'person', 'attitude': 'trust', 'delta': -15}],
        duration=5,
    ),
    make_action(
        'set_customs',
        'Set Community Customs',
        'communities',
        'Establish or modify community norms and taboos.',
        targets=[{'name': 'community', 'kind': 'community', 'required': True, 'description': 'Community faction'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=5,
        fatigue=2,
        world_effect='updates community customs rules',
        fatigue_delta=2,
        duration=5,
    ),
    make_action(
        'set_relation',
        'Set Community Relation',
        'communities',
        'Adjust diplomatic stance and trust between factions.',
        targets=[
            {'name': 'community', 'kind': 'community', 'required': True, 'description': 'Source community'},
            {'name': 'target', 'kind': 'community', 'required': True, 'description': 'Target community'},
        ],
        same_place=True,
        held=False,
        in_reach=True,
        time=5,
        fatigue=2,
        world_effect='updates inter-community stance and numeric trust',
        fatigue_delta=2,
        duration=5,
    ),
    make_action(
        'check_relation',
        'Check Community Relation',
        'communities',
        'Inspect stance and trust between communities.',
        targets=[
            {'name': 'community', 'kind': 'community', 'required': True, 'description': 'Source community'},
            {'name': 'target', 'kind': 'community', 'required': True, 'description': 'Target community'},
        ],
        same_place=False,
        held=False,
        in_reach=False,
        time=1,
        fatigue=0,
        world_effect='reports stance and trust score',
        fatigue_delta=0,
        duration=1,
    ),
    make_action(
        'barter',
        'Barter Trade',
        'communities',
        'Exchange offered goods for requested goods at a community market.',
        targets=[
            {'name': 'community', 'kind': 'community', 'required': True, 'description': 'Host community market'},
            {'name': 'offer', 'kind': 'thing', 'required': True, 'description': 'Offered trade goods'},
            {'name': 'request', 'kind': 'thing', 'required': True, 'description': 'Requested trade goods'},
        ],
        same_place=True,
        held=False,
        in_reach=True,
        time=5,
        fatigue=3,
        world_effect='atomic barter exchange of goods and currency',
        fatigue_delta=3,
        relationships=[{'target_slot': 'community', 'attitude': 'trust', 'delta': 5}],
        noise=15,
        duration=5,
    ),

    # 5. Inventory
    make_action(
        'transfer',
        'Transfer Item',
        'inventory',
        'Atomic transfer of items between characters or containers.',
        targets=[
            {'name': 'thing', 'kind': 'thing', 'required': True, 'description': 'Item to transfer'},
            {'name': 'recipient', 'kind': 'person', 'required': True, 'description': 'Recipient character'},
        ],
        same_place=True,
        held=True,
        in_reach=True,
        time=1,
        fatigue=1,
        world_effect='transfers ownership and possession of item',
        fatigue_delta=1,
        duration=1,
    ),

    # 6. Fitness
    make_action(
        'travel_cost',
        'Assess Travel Cost',
        'fitness',
        'Calculate travel duration and fatigue cost based on load and fitness.',
        targets=[{'name': 'destination', 'kind': 'place', 'required': True, 'description': 'Travel destination'}],
        same_place=False,
        held=False,
        in_reach=False,
        time=0,
        fatigue=0,
        world_effect='computes travel duration and exertion cost',
        fatigue_delta=0,
        duration=0,
        interruptible=False,
    ),
    make_action(
        'can_carry',
        'Check Carry Capacity',
        'fitness',
        'Check if additional weight can be carried within fitness limits.',
        targets=[{'name': 'thing', 'kind': 'thing', 'required': True, 'description': 'Item to evaluate weight'}],
        same_place=False,
        held=False,
        in_reach=False,
        time=0,
        fatigue=0,
        world_effect='evaluates carry threshold and penalty burden',
        fatigue_delta=0,
        duration=0,
        interruptible=False,
    ),
    make_action(
        'fitness_train',
        'Physical Conditioning',
        'fitness',
        'Train strength, endurance, or agility through physical exercise.',
        targets=[],
        same_place=True,
        held=False,
        in_reach=False,
        time=30,
        fatigue=20,
        world_effect='improves physical attribute and accrues fatigue',
        fatigue_delta=20,
        skill_practice=[{'skill': 'athletics', 'practice': 15}],
        duration=30,
    ),

    # 7. Props
    make_action(
        'prop_interact',
        'Interact with Prop',
        'props',
        'Trigger registered interaction on a world prop (ring bell, pull lever).',
        targets=[{'name': 'prop', 'kind': 'prop', 'required': True, 'description': 'World prop object'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=1,
        fatigue=1,
        world_effect='executes registered prop interaction and emits narration',
        fatigue_delta=1,
        noise=20,
        duration=1,
    ),

    # 8. Services
    make_action(
        'service',
        'Character Service',
        'services',
        'Receive or deliver an authored character service (inn lodging, healing, guidance).',
        targets=[
            {'name': 'provider', 'kind': 'person', 'required': True, 'description': 'Service provider'},
            {'name': 'service', 'kind': 'service', 'required': True, 'description': 'Authored service'},
        ],
        same_place=True,
        held=False,
        in_reach=True,
        time=10,
        fatigue=2,
        world_effect='fulfills service terms, transfers fee, applies benefits',
        fatigue_delta=2,
        relationships=[{'target_slot': 'provider', 'attitude': 'trust', 'delta': 4}],
        duration=10,
    ),

    # 9. Reactions
    make_action(
        'gift',
        'Give Gift',
        'reactions',
        'Offer a gift of resources or coins to another character.',
        targets=[
            {'name': 'recipient', 'kind': 'person', 'required': True, 'description': 'Recipient person'},
            {'name': 'resource', 'kind': 'thing', 'required': True, 'description': 'Gift item or coins'},
        ],
        same_place=True,
        held=True,
        in_reach=True,
        time=2,
        fatigue=1,
        world_effect='gifts item and increases recipient affinity and trust',
        fatigue_delta=1,
        relationships=[{'target_slot': 'recipient', 'attitude': 'trust', 'delta': 15}],
        duration=2,
    ),
    make_action(
        'loan',
        'Lend Resource',
        'reactions',
        'Lend coins or tools to another character with repayment expectations.',
        targets=[
            {'name': 'borrower', 'kind': 'person', 'required': True, 'description': 'Borrower person'},
            {'name': 'resource', 'kind': 'thing', 'required': True, 'description': 'Loaned resource'},
        ],
        same_place=True,
        held=True,
        in_reach=True,
        time=2,
        fatigue=1,
        world_effect='records active loan obligation and transfers resource',
        fatigue_delta=1,
        relationships=[{'target_slot': 'borrower', 'attitude': 'obligation', 'delta': 20}],
        duration=2,
    ),
    make_action(
        'help',
        'Provide Assistance',
        'reactions',
        'Provide assistance or manual labor to help someone.',
        targets=[{'name': 'person', 'kind': 'person', 'required': True, 'description': 'Person receiving help'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=15,
        fatigue=8,
        world_effect='assists person, fulfilling promise or gaining gratitude',
        fatigue_delta=8,
        relationships=[{'target_slot': 'person', 'attitude': 'trust', 'delta': 10}],
        duration=15,
    ),
    make_action(
        'repay',
        'Repay Debt',
        'reactions',
        'Repay an outstanding loan or debt to a creditor.',
        targets=[{'name': 'lender', 'kind': 'person', 'required': True, 'description': 'Creditor person'}],
        same_place=True,
        held=True,
        in_reach=True,
        time=2,
        fatigue=1,
        world_effect='clears loan record and restores creditor trust',
        fatigue_delta=1,
        relationships=[{'target_slot': 'lender', 'attitude': 'trust', 'delta': 12}],
        duration=2,
    ),
    make_action(
        'debt_query',
        'Query Debt',
        'reactions',
        'Ask about current outstanding loans or debts.',
        targets=[{'name': 'person', 'kind': 'person', 'required': True, 'description': 'Person questioned'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=1,
        fatigue=0,
        world_effect='reports balance of outstanding loans and promises',
        fatigue_delta=0,
        duration=1,
    ),

    # 10. Engine
    make_action(
        'approach',
        'Approach Character or Spot',
        'engine',
        'Walk over close to a character or spot within the current place.',
        targets=[{'name': 'target', 'kind': 'person', 'required': True, 'description': 'Character or spot to approach'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=1,
        fatigue=1,
        world_effect='positions actor adjacent to target',
        fatigue_delta=1,
        duration=1,
    ),
    make_action(
        'enter',
        'Enter Connected Place',
        'engine',
        'Pass through an entrance to enter a connected room or place.',
        targets=[{'name': 'place', 'kind': 'place', 'required': True, 'description': 'Place to enter'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=1,
        fatigue=1,
        world_effect='updates actor location to entered place',
        fatigue_delta=1,
        duration=1,
    ),
    make_action(
        'talk',
        'Speak to Character',
        'engine',
        'Initiate dialogue conversation with another person.',
        targets=[{'name': 'listener', 'kind': 'person', 'required': True, 'description': 'Conversation listener'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=1,
        fatigue=0,
        world_effect='routes dialogue turn and updates participant attitudes',
        fatigue_delta=0,
        noise=15,
        duration=1,
    ),
    make_action(
        'look',
        'Look Around',
        'engine',
        'Observe current place surroundings, spots, props, and present people.',
        targets=[{'name': 'target', 'kind': 'place', 'required': False, 'description': 'Inspected place or feature'}],
        same_place=True,
        held=False,
        in_reach=False,
        time=1,
        fatigue=0,
        world_effect='returns descriptive observation of place and occupants',
        fatigue_delta=0,
        visibility='private',
        noise=0,
        duration=1,
    ),
    make_action(
        'wait',
        'Wait and Rest',
        'engine',
        'Spend a turn resting or observing without changing location.',
        targets=[],
        same_place=False,
        held=False,
        in_reach=False,
        time=1,
        fatigue=0,
        world_effect='recovers minor fatigue and advances clock',
        fatigue_delta=-2,
        visibility='private',
        noise=0,
        duration=1,
    ),

    # 11. Initiative
    make_action(
        'activity',
        'Routine Activity',
        'initiative',
        'Engage in scheduled routine or autonomous task activity.',
        targets=[],
        same_place=True,
        held=False,
        in_reach=False,
        time=15,
        fatigue=5,
        world_effect='advances routine activity progress',
        fatigue_delta=5,
        duration=15,
    ),
    make_action(
        'ask',
        'Ask Question',
        'initiative',
        'Inquire about an unknown fact, rumor, or unfinished topic.',
        targets=[{'name': 'listener', 'kind': 'person', 'required': True, 'description': 'Person questioned'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=1,
        fatigue=0,
        world_effect='raises topic inquiry to listener',
        fatigue_delta=0,
        noise=12,
        duration=1,
    ),
    make_action(
        'followup',
        'Follow Up',
        'initiative',
        'Follow up on previously discussed unfinished business.',
        targets=[{'name': 'listener', 'kind': 'person', 'required': True, 'description': 'Conversation partner'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=1,
        fatigue=0,
        world_effect='resumes unfinished conversation topic',
        fatigue_delta=0,
        noise=12,
        duration=1,
    ),
    make_action(
        'decline',
        'Decline Offer',
        'initiative',
        'Politely decline a request, invitation, or deal.',
        targets=[{'name': 'listener', 'kind': 'person', 'required': True, 'description': 'Person addressed'}],
        same_place=True,
        held=False,
        in_reach=True,
        time=1,
        fatigue=0,
        world_effect='signals polite refusal',
        fatigue_delta=0,
        noise=8,
        duration=1,
    ),
    make_action(
        'gesture',
        'Express Gesture',
        'initiative',
        'Express non-verbal physical gesture or posture.',
        targets=[],
        same_place=True,
        held=False,
        in_reach=False,
        time=1,
        fatigue=0,
        world_effect='displays visual posture or expressive movement',
        fatigue_delta=0,
        noise=2,
        duration=1,
    ),
    make_action(
        'silence',
        'Deliberate Silence',
        'initiative',
        'Hold intentional silence or pause.',
        targets=[],
        same_place=False,
        held=False,
        in_reach=False,
        time=1,
        fatigue=0,
        world_effect='remains silent and yields conversational turn',
        fatigue_delta=0,
        visibility='private',
        noise=0,
        duration=1,
    ),
]


class ActionCatalog:
    def __init__(self, actions=None, cap=MAX_ACTIONS):
        self.cap = cap
        self.actions = {act['id']: copy.deepcopy(act) for act in BUILTIN_ACTIONS}
        if actions:
            for act in actions:
                validate_action(act)
                self.actions[act['id']] = copy.deepcopy(act)

    def to_value(self):
        # Sort by action ID for deterministic serialization
        ordered = [self.actions[k] for k in sorted(self.actions.keys())]
        return {'version': 1, 'actions': ordered}

    def op_list(self, category=None, actor_species=None):
        res = []
        for act_id in sorted(self.actions.keys()):
            act = self.actions[act_id]
            if category is not None and act['category'] != category:
                continue
            if actor_species is not None:
                allowed_species = act['actor'].get('species', [])
                if allowed_species and actor_species not in allowed_species:
                    continue
            res.append(act)
        return {'actions': res, 'count': len(res)}

    def op_describe(self, action_id):
        ident(action_id)
        assert action_id in self.actions, f"action {action_id} not found"
        return {'action': copy.deepcopy(self.actions[action_id])}

    def op_register(self, action):
        validate_action(action)
        assert len(self.actions) < self.cap or action['id'] in self.actions
        self.actions[action['id']] = copy.deepcopy(action)
        return {'registered': action['id']}

    def check_actor(self, act, actor):
        req_actor = act['actor']
        species = actor.get('species')
        if species and req_actor.get('species'):
            if species not in req_actor['species']:
                return False, f"species '{species}' not permitted"

        body_state = req_actor.get('body_state', {})
        if 'max_fatigue' in body_state:
            fatigue = actor.get('fitness', {}).get('fatigue', actor.get('fatigue', 0))
            if fatigue > body_state['max_fatigue']:
                return False, f"too fatigued ({fatigue} > {body_state['max_fatigue']})"

        if 'min_condition' in body_state:
            cond = actor.get('fitness', {}).get('condition', actor.get('condition', 100))
            if cond < body_state['min_condition']:
                return False, f"condition too low ({cond} < {body_state['min_condition']})"

        disallowed = body_state.get('disallowed_conditions', [])
        actor_conds = actor.get('conditions', [])
        for c in actor_conds:
            if c in disallowed:
                return False, f"cannot act while {c}"

        req_skills = req_actor.get('skills', [])
        actor_skills = actor.get('skills', {})
        for req_sk in req_skills:
            sk_id = req_sk['skill']
            req_lvl = req_sk['level']
            actual_lvl = actor_skills.get(sk_id, 0)
            if actual_lvl < req_lvl:
                return False, f"skill {sk_id} level {actual_lvl} < {req_lvl}"

        req_mem = req_actor.get('membership')
        if req_mem:
            actor_mems = actor.get('communities', actor.get('factions', []))
            if req_mem not in actor_mems:
                return False, f"requires membership in {req_mem}"

        return True, 'ok'

    def op_check(self, action_id, actor, bindings=None, place=None):
        ident(action_id)
        assert action_id in self.actions, f"action {action_id} not found"
        act = self.actions[action_id]
        bindings = bindings or {}

        ok, reason = self.check_actor(act, actor)
        if not ok:
            return {
                'feasible': False,
                'action': action_id,
                'reason': reason,
            }

        # Check required target slots
        for ts in act['targets']:
            slot_name = ts['name']
            if ts['required'] and slot_name not in bindings:
                return {
                    'feasible': False,
                    'action': action_id,
                    'reason': f"missing required target slot '{slot_name}'",
                }

        # Precondition checks
        pre = act['preconditions']
        if pre.get('held', False):
            # Thing must be in actor's inventory
            target_thing = bindings.get('thing') or bindings.get('resource')
            if target_thing:
                inv = actor.get('inventory', [])
                if target_thing not in inv:
                    return {
                        'feasible': False,
                        'action': action_id,
                        'reason': f"item '{target_thing}' is not held in inventory",
                    }

        return {
            'feasible': True,
            'action': action_id,
            'cost': copy.deepcopy(act['cost']),
            'effects': copy.deepcopy(act['effects']),
            'perception': copy.deepcopy(act['perception']),
            'duration': act['duration'],
            'interruptible': act['interruptible'],
        }

    def op_available(self, actor, place=None, targets=None, category=None):
        targets = targets or []
        candidates_by_kind = {}
        for t in targets:
            k = t.get('kind', 'thing')
            candidates_by_kind.setdefault(k, []).append(t.get('id', ''))

        res = []
        for act_id in sorted(self.actions.keys()):
            act = self.actions[act_id]
            if category is not None and act['category'] != category:
                continue

            ok, _ = self.check_actor(act, actor)
            if not ok:
                continue

            # Check if all required target slots can be satisfied by present targets
            can_bind = True
            bindings = {}
            for ts in act['targets']:
                slot_name = ts['name']
                slot_kind = ts['kind']
                if ts['required']:
                    available = candidates_by_kind.get(slot_kind, [])
                    if not available:
                        can_bind = False
                        break
                    bindings[slot_name] = available[0]
                else:
                    available = candidates_by_kind.get(slot_kind, [])
                    if available:
                        bindings[slot_name] = available[0]

            if can_bind:
                chk = self.op_check(act_id, actor, bindings=bindings, place=place)
                if chk['feasible']:
                    res.append({
                        'action': act_id,
                        'category': act['category'],
                        'label': act['label'],
                        'bindings': bindings,
                        'cost': copy.deepcopy(act['cost']),
                        'duration': act['duration'],
                    })

        return {'available': res, 'count': len(res)}

    def workflow(self, req):
        keys(req, ['ops'], ['catalog', 'actions'])
        ops = req['ops']
        assert isinstance(ops, list) and len(ops) <= MAX_OPS

        if 'actions' in req and req['actions'] is not None:
            for act in req['actions']:
                validate_action(act)
                self.actions[act['id']] = copy.deepcopy(act)
            assert len(self.actions) <= self.cap

        results = []
        for op_call in ops:
            keys(op_call, ['op'], ['at', 'args'])
            op_name = text(op_call['op'], 32)
            args = op_call.get('args', {})

            if op_name == 'list':
                keys(args, [], ['category', 'actor_species'])
                cat = args.get('category')
                species = args.get('actor_species')
                if cat:
                    assert cat in CATEGORIES
                if species:
                    ident(species)
                results.append(self.op_list(category=cat, actor_species=species))

            elif op_name == 'describe':
                keys(args, ['id'])
                act_id = ident(args['id'])
                results.append(self.op_describe(act_id))

            elif op_name == 'available':
                keys(args, ['actor'], ['place', 'targets', 'category'])
                act = args['actor']
                plc = args.get('place')
                tgts = args.get('targets', [])
                cat = args.get('category')
                if cat:
                    assert cat in CATEGORIES
                results.append(self.op_available(act, place=plc, targets=tgts, category=cat))

            elif op_name == 'check':
                keys(args, ['action', 'actor'], ['bindings', 'place'])
                act_id = ident(args['action'])
                act = args['actor']
                bnd = args.get('bindings', {})
                plc = args.get('place')
                results.append(self.op_check(act_id, act, bindings=bnd, place=plc))

            elif op_name == 'register':
                keys(args, ['action'])
                action_data = args['action']
                results.append(self.op_register(action_data))

            else:
                assert False, f"unknown op {op_name}"

        return {
            'results': results,
            'catalog': self.to_value(),
        }


def workflow(req):
    lim = Limits(SPEC)
    return limited(req, lim, lambda b: ActionCatalog(None, lim['actions']).workflow(b))
