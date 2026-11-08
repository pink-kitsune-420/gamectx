"""Independent original species reference: animals, plants, people, hybrids,
temperament, and bounded animal interactions (pet, feed, calm, lead, ride, load, train, harvest).
Python standard library only; nothing here calls a model.
"""
import copy, json, re

MAX_SPECIES = 256
MAX_ANIMALS = 256
MAX_OPS = 64
MAX_SAFE = 9007199254740991

KINDS = ['animal', 'plant', 'person', 'hybrid']
DIETS = ['herbivore', 'carnivore', 'omnivore', 'photosynthetic', 'scavenger', 'mineral']


def enc(v): return json.dumps(v, ensure_ascii=False, separators=(',', ':'))
def control(s): return any(ord(c) < 32 or 127 <= ord(c) <= 159 for c in s)
def text(s, chars):
    assert isinstance(s, str) and 1 <= len(s) <= chars and not control(s); return s
def ident(s): assert isinstance(s, str) and re.fullmatch('[A-Za-z0-9_:-]{1,64}', s); return s
def uint(n, lo=0, hi=MAX_SAFE): assert type(n) is int and lo <= n <= hi; return n
def boolean(b): assert type(b) is bool; return b
def keys(v, required, optional=()): assert isinstance(v, dict) and set(required) <= set(v) <= set(required) | set(optional)


def validate_produce(p):
    keys(p, ['kind', 'amount', 'interval'])
    ident(p['kind']); uint(p['amount'], 1, 1000000000); uint(p['interval'], 1, 1000000000)


def validate_temperament(t):
    keys(t, ['boldness', 'sociability', 'aggression', 'trainability'])
    uint(t['boldness'], 0, 100)
    uint(t['sociability'], 0, 100)
    uint(t['aggression'], 0, 100)
    uint(t['trainability'], 0, 100)


def validate_species(s):
    keys(s, ['id', 'label', 'kind'],
         ['parents', 'diet', 'habitats', 'lifespan', 'temperament',
          'tameable', 'rideable', 'carrier_capacity', 'produce', 'sounds', 'gestures'])
    ident(s['id']); text(s['label'], 48)
    assert s['kind'] in KINDS
    if s['kind'] == 'hybrid':
        assert 'parents' in s and isinstance(s['parents'], list) and 1 <= len(s['parents']) <= 2
        for p in s['parents']: ident(p)
    else:
        assert 'parents' not in s or s['parents'] is None

    if 'diet' in s and s['diet'] is not None:
        assert s['diet'] in DIETS
    if 'habitats' in s and s['habitats'] is not None:
        assert isinstance(s['habitats'], list) and len(s['habitats']) <= 16
        for h in s['habitats']: ident(h)
    if 'lifespan' in s and s['lifespan'] is not None:
        uint(s['lifespan'], 1, 1000000)
    if 'temperament' in s and s['temperament'] is not None:
        validate_temperament(s['temperament'])
    if 'tameable' in s and s['tameable'] is not None:
        boolean(s['tameable'])
    if 'rideable' in s and s['rideable'] is not None:
        boolean(s['rideable'])
    if 'carrier_capacity' in s and s['carrier_capacity'] is not None:
        uint(s['carrier_capacity'], 0, 1000000000)
    if 'produce' in s and s['produce'] is not None:
        assert isinstance(s['produce'], list) and len(s['produce']) <= 8
        for pr in s['produce']: validate_produce(pr)
    if 'sounds' in s and s['sounds'] is not None:
        assert isinstance(s['sounds'], list) and len(s['sounds']) <= 16
        for snd in s['sounds']: text(snd, 32)
    if 'gestures' in s and s['gestures'] is not None:
        assert isinstance(s['gestures'], list) and len(s['gestures']) <= 16
        for gst in s['gestures']: text(gst, 48)


def validate_animal(a):
    keys(a, ['id', 'species'], ['tameness', 'trust', 'lead_by', 'ridden_by', 'temperament', 'last_harvest'])
    ident(a['id']); ident(a['species'])
    if 'tameness' in a and a['tameness'] is not None:
        uint(a['tameness'], 0, 100)
    if 'trust' in a and a['trust'] is not None:
        assert isinstance(a['trust'], dict) and len(a['trust']) <= 64
        for k, v in a['trust'].items():
            ident(k); assert type(v) is int and -100 <= v <= 100
    if 'lead_by' in a and a['lead_by'] is not None:
        ident(a['lead_by'])
    if 'ridden_by' in a and a['ridden_by'] is not None:
        ident(a['ridden_by'])
    if 'temperament' in a and a['temperament'] is not None:
        validate_temperament(a['temperament'])
    if 'last_harvest' in a and a['last_harvest'] is not None:
        assert isinstance(a['last_harvest'], dict) and len(a['last_harvest']) <= 16
        for k, v in a['last_harvest'].items():
            ident(k); uint(v, 0, MAX_SAFE)


class SpeciesCatalog:
    def __init__(self, species=None):
        self.species = {}
        if species is not None:
            assert isinstance(species, list) and len(species) <= MAX_SPECIES
            for s in species:
                validate_species(s)
                assert s['id'] not in self.species
                self.species[s['id']] = copy.deepcopy(s)
        self._resolve_hybrids()

    def _resolve_hybrids(self):
        # Resolve inheritance for hybrids without cycles
        resolved = {}
        def resolve(sid, stack=()):
            if sid in resolved: return resolved[sid]
            assert sid in self.species, f"parent species {sid} not found"
            assert sid not in stack, f"species inheritance cycle: {sid}"
            raw = self.species[sid]
            if raw['kind'] != 'hybrid':
                resolved[sid] = copy.deepcopy(raw)
                return resolved[sid]
            # Blend from parents
            parents = [resolve(p, stack + (sid,)) for p in raw['parents']]
            p0 = parents[0]
            merged = copy.deepcopy(p0)
            merged['id'] = raw['id']
            merged['label'] = raw['label']
            merged['kind'] = 'hybrid'
            merged['parents'] = list(raw['parents'])

            # Blend second parent if present
            if len(parents) > 1:
                p1 = parents[1]
                # Combine habitats
                habs = list(p0.get('habitats', []))
                for h in p1.get('habitats', []):
                    if h not in habs: habs.append(h)
                merged['habitats'] = habs
                # Average temperament
                t0 = p0.get('temperament', dict(boldness=50, sociability=50, aggression=50, trainability=50))
                t1 = p1.get('temperament', dict(boldness=50, sociability=50, aggression=50, trainability=50))
                merged['temperament'] = dict(
                    boldness=(t0['boldness'] + t1['boldness']) // 2,
                    sociability=(t0['sociability'] + t1['sociability']) // 2,
                    aggression=(t0['aggression'] + t1['aggression']) // 2,
                    trainability=(t0['trainability'] + t1['trainability']) // 2,
                )
                # Combine sounds & gestures
                snds = list(p0.get('sounds', []))
                for snd in p1.get('sounds', []):
                    if snd not in snds: snds.append(snd)
                merged['sounds'] = snds
                gsts = list(p0.get('gestures', []))
                for gst in p1.get('gestures', []):
                    if gst not in gsts: gsts.append(gst)
                merged['gestures'] = gsts

            # Apply explicit overrides from raw hybrid definition
            for k in ['diet', 'habitats', 'lifespan', 'tameable', 'rideable', 'carrier_capacity', 'produce', 'sounds', 'gestures']:
                if k in raw and raw[k] is not None:
                    merged[k] = copy.deepcopy(raw[k])
            if 'temperament' in raw and raw['temperament'] is not None:
                merged['temperament'] = copy.deepcopy(raw['temperament'])

            resolved[sid] = merged
            return merged

        for sid in list(self.species.keys()):
            resolve(sid)
        self.resolved = resolved

    def get(self, sid):
        return self.resolved.get(sid)

    def to_value(self):
        return list(self.species.values())


class Animals:
    def __init__(self, animals=None):
        self.animals = {}
        if animals is not None:
            assert isinstance(animals, list) and len(animals) <= MAX_ANIMALS
            for a in animals:
                validate_animal(a)
                assert a['id'] not in self.animals
                self.animals[a['id']] = copy.deepcopy(a)

    def to_value(self):
        res = []
        for a in self.animals.values():
            item = dict(id=a['id'], species=a['species'])
            if 'tameness' in a and a['tameness'] is not None:
                item['tameness'] = a['tameness']
            if 'trust' in a and a['trust'] is not None:
                item['trust'] = a['trust']
            if 'lead_by' in a:
                item['lead_by'] = a['lead_by']
            if 'ridden_by' in a:
                item['ridden_by'] = a['ridden_by']
            if 'temperament' in a and a['temperament'] is not None:
                item['temperament'] = a['temperament']
            if 'last_harvest' in a and a['last_harvest'] is not None:
                item['last_harvest'] = a['last_harvest']
            res.append(item)
        return res

    def get(self, aid):
        return self.animals.get(aid)

    def get_or_create(self, aid, species_id):
        if aid not in self.animals:
            self.animals[aid] = dict(id=aid, species=species_id, tameness=50, trust={}, lead_by=None, ridden_by=None)
        return self.animals[aid]


class SpeciesWorld:
    def __init__(self, species=None, animals=None):
        self.catalog = SpeciesCatalog(species)
        self.animals = Animals(animals)

    def op_pet(self, at, actor, animal_id, place):
        an = self.animals.get(animal_id)
        assert an is not None, f"animal {animal_id} not found"
        sp = self.catalog.get(an['species'])
        assert sp is not None, f"species {an['species']} not found"

        trust_map = an.setdefault('trust', {})
        cur_trust = trust_map.get(actor, 0)
        tameness = an.get('tameness', 50)
        temp = an.get('temperament') or sp.get('temperament', dict(boldness=50, sociability=50, aggression=50, trainability=50))

        # Check if animal permits petting: aggression - trust - tameness/2 <= 40
        threat = temp['aggression'] - cur_trust - (tameness // 2)
        if threat > 50:
            # Rejects / snaps
            sound = sp.get('sounds', ['growl'])[0] if sp.get('sounds') else 'growl'
            gesture = sp.get('gestures', ['steps back'])[0] if sp.get('gestures') else 'steps back'
            trust_map[actor] = max(-100, cur_trust - 5)
            summary = f"{actor} tried to pet {animal_id}, but it snapped ({sound})."
            entry = dict(
                at=at, kind="animal_interaction", summary=summary, actors=[actor, animal_id],
                place=place, things=[], tags=["species", "animal", "pet", "failed"],
                visibility="public", witnesses=[], phrases=[sp['label'], animal_id, "pet"],
                facts=dict(interaction="pet", success=False, trust=trust_map[actor])
            )
            return dict(ok=False, success=False, trust=trust_map[actor], sound=sound, gesture=gesture, entry=entry)

        # Successful pet
        delta = 10 if temp['sociability'] >= 50 else 5
        new_trust = min(100, cur_trust + delta)
        trust_map[actor] = new_trust
        sound = sp.get('sounds', ['purr'])[0] if sp.get('sounds') else 'purr'
        gesture = sp.get('gestures', ['nudges hand'])[0] if sp.get('gestures') else 'nudges hand'
        summary = f"{actor} petted {animal_id} ({gesture})."
        entry = dict(
            at=at, kind="animal_interaction", summary=summary, actors=[actor, animal_id],
            place=place, things=[], tags=["species", "animal", "pet", "success"],
            visibility="public", witnesses=[], phrases=[sp['label'], animal_id, "pet"],
            facts=dict(interaction="pet", success=True, trust=new_trust)
        )
        return dict(ok=True, success=True, trust=new_trust, sound=sound, gesture=gesture, entry=entry)

    def op_feed(self, at, actor, animal_id, place, food_thing, things_state):
        an = self.animals.get(animal_id)
        assert an is not None, f"animal {animal_id} not found"
        sp = self.catalog.get(an['species'])
        assert sp is not None, f"species {an['species']} not found"

        # Check food item in things_state
        things_list = things_state.setdefault('things', [])
        kinds_map = {k['id']: k for k in things_state.get('kinds', [])}

        food_item = None
        for t in things_list:
            if t['id'] == food_thing:
                loc = t.get('location', {})
                if loc.get('holder') == actor or loc.get('place') == place:
                    food_item = t
                    break
        assert food_item is not None, f"food item {food_thing} not found or not accessible"

        food_kind = kinds_map.get(food_item['kind'], {})
        diet = sp.get('diet', 'herbivore')
        # Diet check:
        tags = food_kind.get('tags', [])
        is_meat = 'meat' in tags or food_kind.get('category') == 'food' and 'meat' in food_kind.get('id', '')
        is_plant = 'plant' in tags or 'timber' in tags or 'ration' in tags or food_kind.get('category') in ['food', 'plant', 'material']

        if diet == 'carnivore' and not is_meat:
            return dict(ok=False, reason="incompatible_diet", diet=diet)
        if diet == 'herbivore' and is_meat:
            return dict(ok=False, reason="incompatible_diet", diet=diet)

        # Consume 1 unit of food
        qty = food_item.get('quantity', 1)
        if qty <= 1:
            things_list.remove(food_item)
        else:
            food_item['quantity'] = qty - 1

        trust_map = an.setdefault('trust', {})
        cur_trust = trust_map.get(actor, 0)
        new_trust = min(100, cur_trust + 15)
        trust_map[actor] = new_trust
        cur_tameness = an.get('tameness', 50)
        new_tameness = min(100, cur_tameness + 5)
        an['tameness'] = new_tameness

        sound = sp.get('sounds', ['munch'])[0] if sp.get('sounds') else 'munch'
        gesture = sp.get('gestures', ['eats eagerly'])[0] if sp.get('gestures') else 'eats eagerly'
        summary = f"{actor} fed {animal_id} with {food_kind.get('label', food_thing)}."
        entry = dict(
            at=at, kind="animal_interaction", summary=summary, actors=[actor, animal_id],
            place=place, things=[food_thing], tags=["species", "animal", "feed"],
            visibility="public", witnesses=[], phrases=[sp['label'], animal_id, "feed"],
            facts=dict(interaction="feed", trust=new_trust, tameness=new_tameness)
        )
        return dict(ok=True, success=True, trust=new_trust, tameness=new_tameness, sound=sound, gesture=gesture, entry=entry)

    def op_calm(self, at, actor, animal_id, place):
        an = self.animals.get(animal_id)
        assert an is not None, f"animal {animal_id} not found"
        sp = self.catalog.get(an['species'])
        assert sp is not None, f"species {an['species']} not found"

        trust_map = an.setdefault('trust', {})
        cur_trust = trust_map.get(actor, 0)
        new_trust = min(100, cur_trust + 5)
        trust_map[actor] = new_trust

        gesture = sp.get('gestures', ['relaxes stance'])[0] if sp.get('gestures') else 'relaxes stance'
        summary = f"{actor} calmed {animal_id} ({gesture})."
        entry = dict(
            at=at, kind="animal_interaction", summary=summary, actors=[actor, animal_id],
            place=place, things=[], tags=["species", "animal", "calm"],
            visibility="public", witnesses=[], phrases=[sp['label'], animal_id, "calm"],
            facts=dict(interaction="calm", trust=new_trust)
        )
        return dict(ok=True, success=True, trust=new_trust, gesture=gesture, entry=entry)

    def op_lead(self, at, actor, animal_id, place, stop=False):
        an = self.animals.get(animal_id)
        assert an is not None, f"animal {animal_id} not found"
        sp = self.catalog.get(an['species'])
        assert sp is not None, f"species {an['species']} not found"

        if stop:
            an['lead_by'] = None
            summary = f"{actor} stopped leading {animal_id}."
            entry = dict(
                at=at, kind="animal_interaction", summary=summary, actors=[actor, animal_id],
                place=place, things=[], tags=["species", "animal", "lead", "stop"],
                visibility="public", witnesses=[], phrases=[sp['label'], animal_id, "lead"],
                facts=dict(interaction="lead", leading=False)
            )
            return dict(ok=True, leading=False, entry=entry)

        trust_map = an.setdefault('trust', {})
        cur_trust = trust_map.get(actor, 0)
        tameness = an.get('tameness', 50)
        if cur_trust < 10 and tameness < 30:
            return dict(ok=False, reason="insufficient_trust_or_tameness", trust=cur_trust, tameness=tameness)

        an['lead_by'] = actor
        summary = f"{actor} began leading {animal_id}."
        entry = dict(
            at=at, kind="animal_interaction", summary=summary, actors=[actor, animal_id],
            place=place, things=[], tags=["species", "animal", "lead", "start"],
            visibility="public", witnesses=[], phrases=[sp['label'], animal_id, "lead"],
            facts=dict(interaction="lead", leading=True)
        )
        return dict(ok=True, leading=True, entry=entry)

    def op_ride(self, at, actor, animal_id, place, dismount=False):
        an = self.animals.get(animal_id)
        assert an is not None, f"animal {animal_id} not found"
        sp = self.catalog.get(an['species'])
        assert sp is not None, f"species {an['species']} not found"

        if dismount:
            an['ridden_by'] = None
            summary = f"{actor} dismounted from {animal_id}."
            entry = dict(
                at=at, kind="animal_interaction", summary=summary, actors=[actor, animal_id],
                place=place, things=[], tags=["species", "animal", "ride", "dismount"],
                visibility="public", witnesses=[], phrases=[sp['label'], animal_id, "ride"],
                facts=dict(interaction="ride", riding=False)
            )
            return dict(ok=True, riding=False, entry=entry)

        if not sp.get('rideable', False):
            return dict(ok=False, reason="not_rideable", species=sp['id'])

        trust_map = an.setdefault('trust', {})
        cur_trust = trust_map.get(actor, 0)
        tameness = an.get('tameness', 50)
        if cur_trust < 20 or tameness < 40:
            return dict(ok=False, reason="insufficient_trust_or_tameness", trust=cur_trust, tameness=tameness)

        an['ridden_by'] = actor
        summary = f"{actor} mounted and rode {animal_id}."
        entry = dict(
            at=at, kind="animal_interaction", summary=summary, actors=[actor, animal_id],
            place=place, things=[], tags=["species", "animal", "ride", "mount"],
            visibility="public", witnesses=[], phrases=[sp['label'], animal_id, "ride"],
            facts=dict(interaction="ride", riding=True)
        )
        return dict(ok=True, riding=True, entry=entry)

    def op_train(self, at, actor, animal_id, place):
        an = self.animals.get(animal_id)
        assert an is not None, f"animal {animal_id} not found"
        sp = self.catalog.get(an['species'])
        assert sp is not None, f"species {an['species']} not found"

        if not sp.get('tameable', True):
            return dict(ok=False, reason="not_tameable", species=sp['id'])

        temp = an.get('temperament') or sp.get('temperament', dict(boldness=50, sociability=50, aggression=50, trainability=50))
        cur_tameness = an.get('tameness', 50)
        gain = max(1, temp['trainability'] // 10)
        new_tameness = min(100, cur_tameness + gain)
        an['tameness'] = new_tameness

        summary = f"{actor} trained {animal_id} (tameness +{gain})."
        entry = dict(
            at=at, kind="animal_interaction", summary=summary, actors=[actor, animal_id],
            place=place, things=[], tags=["species", "animal", "train"],
            visibility="public", witnesses=[], phrases=[sp['label'], animal_id, "train"],
            facts=dict(interaction="train", tameness=new_tameness, gain=gain)
        )
        return dict(ok=True, tameness=new_tameness, gain=gain, entry=entry)

    def op_harvest(self, at, actor, animal_id, place, things_state, new_id=None):
        an = self.animals.get(animal_id)
        assert an is not None, f"animal {animal_id} not found"
        sp = self.catalog.get(an['species'])
        assert sp is not None, f"species {an['species']} not found"

        produce_list = sp.get('produce', [])
        if not produce_list:
            return dict(ok=False, reason="no_produce", species=sp['id'])

        last_harvest_map = an.setdefault('last_harvest', {})
        pr = produce_list[0] # harvest primary produce
        last_at = last_harvest_map.get(pr['kind'], 0)
        if at < last_at + pr['interval']:
            return dict(ok=False, reason="not_ready", ready_at=last_at + pr['interval'])

        last_harvest_map[pr['kind']] = at
        things_list = things_state.setdefault('things', [])
        out_id = new_id if new_id else f"{pr['kind']}_{at}"
        new_t = dict(id=out_id, kind=pr['kind'], location=dict(holder=actor), condition=100, owner=actor, quantity=pr['amount'], passengers=[])
        things_list.append(new_t)

        summary = f"{actor} harvested {pr['amount']} {pr['kind']} from {animal_id}."
        entry = dict(
            at=at, kind="animal_interaction", summary=summary, actors=[actor, animal_id],
            place=place, things=[out_id], tags=["species", "animal", "harvest", pr['kind']],
            visibility="public", witnesses=[], phrases=[sp['label'], animal_id, pr['kind']],
            facts=dict(interaction="harvest", product=pr['kind'], amount=pr['amount'])
        )
        return dict(ok=True, product=pr['kind'], amount=pr['amount'], item=new_t, entry=entry)

    def op_behave(self, actor, animal_id, place):
        an = self.animals.get(animal_id)
        assert an is not None, f"animal {animal_id} not found"
        sp = self.catalog.get(an['species'])
        assert sp is not None, f"species {an['species']} not found"

        trust_map = an.get('trust', {})
        cur_trust = trust_map.get(actor, 0)
        tameness = an.get('tameness', 50)
        temp = an.get('temperament') or sp.get('temperament', dict(boldness=50, sociability=50, aggression=50, trainability=50))

        # Determine behavioural attitude
        if cur_trust >= 30 and tameness >= 50:
            attitude = "affectionate"
            sound = sp.get('sounds', ['contented sound'])[0] if sp.get('sounds') else 'contented sound'
            gesture = sp.get('gestures', ['leans against actor'])[0] if sp.get('gestures') else 'leans against actor'
        elif cur_trust < -10 or temp['aggression'] >= 70:
            attitude = "hostile"
            sound = sp.get('sounds', ['threatening growl'])[0] if sp.get('sounds') else 'threatening growl'
            gesture = sp.get('gestures', ['bares teeth'])[0] if sp.get('gestures') else 'bares teeth'
        elif temp['boldness'] < 30:
            attitude = "skittish"
            sound = sp.get('sounds', ['wary snort'])[0] if sp.get('sounds') else 'wary snort'
            gesture = sp.get('gestures', ['keeps distance'])[0] if sp.get('gestures') else 'keeps distance'
        else:
            attitude = "calm"
            sound = sp.get('sounds', ['soft breath'])[0] if sp.get('sounds') else 'soft breath'
            gesture = sp.get('gestures', ['observes calmly'])[0] if sp.get('gestures') else 'observes calmly'

        return dict(ok=True, animal=animal_id, attitude=attitude, sound=sound, gesture=gesture)

    def workflow(self, req):
        assert isinstance(req, dict)
        keys(req, ['ops'], ['species', 'animals', 'things'])
        ops = req['ops']
        assert isinstance(ops, list) and len(ops) <= MAX_OPS
        if 'species' in req and req['species'] is not None:
            self.catalog = SpeciesCatalog(req['species'])
        if 'animals' in req and req['animals'] is not None:
            self.animals = Animals(req['animals'])
        things_state = req.get('things', dict(version=1, materials=[], kinds=[], things=[]))

        results = []
        for o in ops:
            keys(o, ['op'], ['args', 'at'])
            op = o['op']
            args = o.get('args', {})
            at = uint(o.get('at', 0))

            backup_animals = copy.deepcopy(self.animals.animals)
            backup_things = copy.deepcopy(things_state)
            try:
                if op == 'pet':
                    keys(args, ['actor', 'animal', 'place'])
                    res = self.op_pet(at, args['actor'], args['animal'], args['place'])
                    results.append(dict(ok=True, result=res))
                elif op == 'feed':
                    keys(args, ['actor', 'animal', 'place', 'food_thing'])
                    res = self.op_feed(at, args['actor'], args['animal'], args['place'], args['food_thing'], things_state)
                    if res.get('ok'):
                        results.append(dict(ok=True, result=res))
                    else:
                        self.animals.animals = backup_animals
                        things_state = backup_things
                        results.append(dict(ok=False, result=res))
                elif op == 'calm':
                    keys(args, ['actor', 'animal', 'place'])
                    res = self.op_calm(at, args['actor'], args['animal'], args['place'])
                    results.append(dict(ok=True, result=res))
                elif op == 'lead':
                    keys(args, ['actor', 'animal', 'place'], ['stop'])
                    res = self.op_lead(at, args['actor'], args['animal'], args['place'], stop=args.get('stop', False))
                    if res.get('ok'):
                        results.append(dict(ok=True, result=res))
                    else:
                        self.animals.animals = backup_animals
                        results.append(dict(ok=False, result=res))
                elif op == 'ride':
                    keys(args, ['actor', 'animal', 'place'], ['dismount'])
                    res = self.op_ride(at, args['actor'], args['animal'], args['place'], dismount=args.get('dismount', False))
                    if res.get('ok'):
                        results.append(dict(ok=True, result=res))
                    else:
                        self.animals.animals = backup_animals
                        results.append(dict(ok=False, result=res))
                elif op == 'train':
                    keys(args, ['actor', 'animal', 'place'])
                    res = self.op_train(at, args['actor'], args['animal'], args['place'])
                    if res.get('ok'):
                        results.append(dict(ok=True, result=res))
                    else:
                        self.animals.animals = backup_animals
                        results.append(dict(ok=False, result=res))
                elif op == 'harvest':
                    keys(args, ['actor', 'animal', 'place'], ['new_id'])
                    res = self.op_harvest(at, args['actor'], args['animal'], args['place'], things_state, new_id=args.get('new_id'))
                    if res.get('ok'):
                        results.append(dict(ok=True, result=res))
                    else:
                        self.animals.animals = backup_animals
                        things_state = backup_things
                        results.append(dict(ok=False, result=res))
                elif op == 'behave':
                    keys(args, ['actor', 'animal', 'place'])
                    res = self.op_behave(args['actor'], args['animal'], args['place'])
                    results.append(dict(ok=True, result=res))
                else:
                    assert False, f"unknown op {op}"
            except Exception:
                self.animals.animals = backup_animals
                things_state = backup_things
                results.append(dict(ok=False))

        return dict(results=results, species=self.catalog.to_value(), animals=self.animals.to_value(), things=things_state)


def workflow(a):
    assert isinstance(a, dict)
    keys(a, ['ops'], ['species', 'animals', 'things'])
    sw = SpeciesWorld(a.get('species'), a.get('animals'))
    return sw.workflow(a)
