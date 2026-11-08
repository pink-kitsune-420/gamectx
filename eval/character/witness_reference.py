"""Independent original witness bridge reference: world events to Life experiences.
Python standard library only; nothing here calls a model.

The witness bridge turns:
1. Timeline entries (or module outcomes from recipes, species, barter, things) +
2. A list of beings with locations and senses
into:
- One `direct` Life experience per perceiving witness
- `report` Life experiences when told by an informed character.

Every produced experience conforms strictly to Life's `validate_event`:
  token: uint (1..2^32-1)
  at: uint (clock)
  kind: 'direct' | 'report'
  tag: ident
  subject: ident
  source: {id: str, speaker: str, text: str}
  audience: [str] (names of participants, must include actor)
  text: str (<= 240 chars, no control)
  causes: [uint] (token causes)
  retain: bool
"""
import copy, json, re

MAX_BEINGS = 64
MAX_OPS = 64
MAX_SAFE = 9007199254740991


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


def boolean(b):
    assert type(b) is bool
    return b


def keys(v, required, optional=()):
    assert isinstance(v, dict) and set(required) <= set(v) <= set(required) | set(optional)


def extract_entry(item):
    """Extract timeline-ready entry from either raw timeline entry or module outcome."""
    if 'entry' in item and isinstance(item['entry'], dict):
        return item['entry']
    return item


def can_perceive(being, entry):
    """
    Evaluate whether a being with current senses and location perceives an event.
    Returns (perceived: bool, reason: str).
    """
    being_id = being['id']
    being_place = being.get('place')
    senses = being.get('senses', {})
    conditions = senses.get('conditions', [])

    # 1. Physical incapacity
    if 'dead' in conditions:
        return False, 'dead'
    if 'unconscious' in conditions:
        return False, 'unconscious'

    # 2. Asleep: can only wake up if noise is loud (>= 80)
    noise = entry.get('noise', 10)
    if 'asleep' in conditions:
        if noise < 80:
            return False, 'asleep_noise_too_low'

    # 3. Visibility and scope rules
    vis = entry.get('visibility', 'public')
    actors = entry.get('actors', [])
    witnesses = entry.get('witnesses', [])
    event_place = entry.get('place')

    # Actors always perceive their own action if conscious
    if being_id in actors:
        return True, 'actor'

    # Private entries are known only to actors
    if vis == 'private':
        return False, 'private_not_actor'

    # Explicitly named witnesses
    is_named_witness = being_id in witnesses

    # Location check
    in_same_place = being_place is not None and event_place is not None and being_place == event_place

    # Occlusion check
    occluded = senses.get('occluded', False)

    # Sensory faculties
    sight = senses.get('sight', True)
    hearing = senses.get('hearing', True)

    if in_same_place:
        if not occluded and sight:
            return True, 'seen_in_place'
        if hearing and noise >= 15:
            return True, 'heard_in_place'
        if is_named_witness:
            return True, 'witness_in_place'
        return False, 'occluded_cannot_sense'

    # If in different place
    if is_named_witness and not occluded and sight:
        return True, 'witness_line_of_sight'

    # Loud public events can be heard from adjacent places
    if vis == 'public' and hearing and noise >= 60:
        return True, 'heard_at_distance'

    return False, 'different_place'


def make_direct_experience(token, being_id, entry):
    at = entry.get('at', 0)
    tag = entry.get('kind', 'event')
    actors = entry.get('actors', [])
    subject = actors[0] if actors else entry.get('place', being_id)
    summary = entry.get('summary', f"{tag} occurred.")

    return {
        'token': token,
        'at': at,
        'kind': 'direct',
        'tag': tag,
        'subject': subject,
        'source': {
            'id': f"wit_{token}",
            'speaker': being_id,
            'text': summary,
        },
        'audience': [being_id],
        'text': summary,
        'causes': [],
        'retain': True,
    }


def make_report_experience(token, listener_id, teller_id, event, at=None):
    event_at = at if at is not None else event.get('at', 0)
    tag = event.get('tag', event.get('kind', 'report'))
    subject = event.get('subject', teller_id)
    text_content = event.get('text', event.get('summary', 'Reported event.'))
    cause_token = event.get('token')
    causes = [cause_token] if cause_token is not None else []

    return {
        'token': token,
        'at': event_at,
        'kind': 'report',
        'tag': tag,
        'subject': subject,
        'source': {
            'id': f"rep_{token}",
            'speaker': teller_id,
            'text': text_content,
        },
        'audience': sorted([listener_id, teller_id]),
        'text': text_content,
        'causes': causes,
        'retain': True,
    }


class WitnessBridge:
    def __init__(self, token_counter=1000):
        self.next_token = token_counter

    def allocate_token(self):
        t = self.next_token
        self.next_token += 1
        return t

    def op_witness(self, entry_item, beings, base_token=None):
        entry = extract_entry(entry_item)
        results = []

        cur_token = base_token if base_token is not None else self.next_token

        for b in sorted(beings, key=lambda x: x['id']):
            perceived, reason = can_perceive(b, entry)
            out = {
                'being': b['id'],
                'perceived': perceived,
                'reason': reason,
            }
            if perceived:
                token = b.get('token', cur_token)
                if base_token is None:
                    cur_token += 1
                    self.next_token = max(self.next_token, cur_token)
                exp = make_direct_experience(token, b['id'], entry)
                out['experience'] = exp
            results.append(out)

        return {
            'entry_kind': entry.get('kind', 'event'),
            'outcomes': results,
            'perceived_count': sum(1 for r in results if r['perceived']),
        }

    def op_tell(self, teller, listener, event, at=None, token=None, teller_place=None, listener_place=None):
        ident(teller)
        ident(listener)
        assert teller != listener, "cannot tell oneself"

        if teller_place is not None and listener_place is not None:
            assert teller_place == listener_place, "teller and listener must be in the same place to speak"

        t = token if token is not None else self.allocate_token()
        clock = at if at is not None else event.get('at', 0)
        exp = make_report_experience(t, listener, teller, event, at=clock)

        return {
            'ok': True,
            'teller': teller,
            'listener': listener,
            'experience': exp,
        }

    def workflow(self, req):
        keys(req, ['ops'], ['beings'])
        ops = req['ops']
        assert isinstance(ops, list) and len(ops) <= MAX_OPS

        default_beings = req.get('beings', [])

        results = []
        for op_call in ops:
            keys(op_call, ['op'], ['args'])
            op_name = text(op_call['op'], 32)
            args = op_call.get('args', {})

            if op_name == 'witness':
                keys(args, ['entry'], ['beings', 'base_token'])
                beings = args.get('beings', default_beings)
                base_token = args.get('base_token')
                results.append(self.op_witness(args['entry'], beings, base_token=base_token))

            elif op_name == 'tell':
                keys(args, ['teller', 'listener', 'event'], ['at', 'token', 'teller_place', 'listener_place'])
                res = self.op_tell(
                    args['teller'],
                    args['listener'],
                    args['event'],
                    at=args.get('at'),
                    token=args.get('token'),
                    teller_place=args.get('teller_place'),
                    listener_place=args.get('listener_place'),
                )
                results.append(res)

            else:
                assert False, f"unknown op {op_name}"

        return {'results': results}


def workflow(req):
    bridge = WitnessBridge()
    return bridge.workflow(req)
