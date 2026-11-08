"""Roleplay engine cases with predeclared expectations; zero model calls (--cap 0).

Each case lists the outcome fields it must produce, written before any Rust output was
observed. `--write-conformance` writes conformance/roleplay-engine/<case>/ from the
reference; otherwise the Rust CLI (`target/debug/gamectx local -`) must match it byte for byte.
"""
import argparse, copy, json, subprocess
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
from roleplay_engine_reference import ROOT, enc, oracle

CHAR = dict(id='baker', name='Mira')
SCENE = dict(id='bakery', label='the bakery', place='workplace', atmosphere='calm', visit=1)
PROPS = dict(version=1, props=[
    dict(id='oven', label='the brick oven', aliases=['oven'], description='A wide brick oven, still warm from the morning bake.', interactions=[
        dict(id='stoke', phrases=['stokes the oven', 'stoke the oven'], decision=dict(action='use', target='prop:oven'), narration='stokes the oven'),
        dict(id='warm_hands', phrases=['warms their hands by the oven', 'warm my hands by the oven'], decision=None, narration='warms their hands by the oven')]),
    dict(id='bell', label='the brass bell', aliases=['bell'], description='A small brass bell hangs above the door.', interactions=[
        dict(id='ring', phrases=['rings the bell', 'ring the bell'], decision=dict(action='use', target='prop:bell'), narration='rings the brass bell')])])
ACTIONS = dict(version=1, entries=[dict(id='bow', phrases=['bows', 'bows politely'], decision=None, narration='bows politely'),
                                   dict(id='buy_bread', phrases=['buys bread', 'buys a loaf'], decision=dict(action='buy', target='item:bread'), narration='buys a loaf of bread')])
CHOICES = [dict(action='buy', target='item:bread'), dict(action='use', target='prop:oven'), dict(action='go', target='room:street')]
TARGETS = [dict(kind='character', id='baker', names=['Mira', 'baker']), dict(kind='place', id='street', names=['street', 'market street'])]


def cfg(**over):
    features = dict(stage_actions=dict(actions=ACTIONS), props=dict(registry=PROPS))
    for f, v in over.pop('features', {}).items(): features[f] = dict(features.get(f, {}), **v)
    c = dict(version=1, character=CHAR, features=features); c.update(over); return c
def turn(text, token=1, at=1, **over):
    t = dict(listener='player_1', text=text, at=at, token=token, scene=dict(SCENE, props=[dict(id='oven', note=None), dict(id='bell', note='It rang twice this morning.')]), choices=CHOICES, targets=TARGETS)
    t.update(over); return dict(op='turn', args=t)
def heard(text, token=1, n=None): return dict(op='delivered', args=dict(token=token, listener='player_1', text=text, heard=len(text) if n is None else n))
def run(ops, **c): return dict(config=cfg(**c), state=None, ops=ops)

FIRST = 'Hello, I\'m Mira; welcome to the bakery.'
# (name, args, error, predeclared expectation: list of (result index, field subset))
CASES = [
    ('greeting_first', run([turn('Hello!')]), False, [(0, dict(route='greeting', line='intro_first_workplace', text=FIRST, introduced=True))]),
    ('greeting_addressed', run([turn('Hello Mira')]), False, [(0, dict(route='greeting', introduced=True))]),
    ('greeting_continuing', run([turn('Hello!'), heard(FIRST), turn('Hello again', token=2, at=2)]), False,
     [(1, dict(recorded=True)), (2, dict(route='greeting', line='greet_continuing', text='I\'m still listening; what is on your mind?'))]),
    ('greeting_partial_not_counted', run([turn('Hello!'), heard(FIRST, n=5), turn('Hello!', token=2, at=2)]), False,
     [(1, dict(recorded=False, reason='partial')), (2, dict(line='intro_first_workplace', text=FIRST))]),
    ('greeting_guarded', run([turn('Hi', trust=-30)]), False, [(0, dict(line='intro_guarded', text='I\'m Mira; I would prefer we take this slowly.'))]),
    ('greeting_emergency', run([turn('Hello', scene=dict(SCENE, atmosphere='emergency'))]), False, [(0, dict(line='greet_urgent', introduced=False))]),
    ('greeting_disabled_social', run([turn('Hello')], features=dict(greetings=dict(enabled=False))), False,
     [(0, dict(route='social', intent='social_greeting', text='Hello; it is good to have someone to talk to.'))]),
    ('variety_skips_heard', run([turn('Thank you'), heard('You are welcome.'), turn('Thanks', token=2, at=2)]), False, [(2, dict(text='Glad to help.'))]),
    ('variety_off', run([turn('Thank you'), heard('You are welcome.'), turn('Thanks', token=2, at=2)], variety=dict(recent=0)), False, [(2, dict(text='You are welcome.'))]),
    ('wellbeing_then_why', run([turn('How are you?'), turn('Why is that?', token=2, at=2, trust=40)]), False,
     [(0, dict(intent='wellbeing', text='I feel fairly settled and happy to listen.')), (1, dict(intent='mood_reason', line='mood_reason_warm'))]),
    ('elaborate_without_topic', run([turn('Tell me more.')]), False, [(0, dict(intent='clarify_topic', text='Which part would you like me to explain?'))]),
    ('perception_location', run([turn('Where are we?')]), False, [(0, dict(route='perception', text='We are in the bakery.'))]),
    ('perception_props', run([turn('What do you see?')]), False, [(0, dict(line='sight_props', text='I can see the brick oven and the brass bell.'))]),
    ('perception_host_sight', run([turn('What can you see?', scene=dict(SCENE, sight='flour dust in the morning light'))]), False, [(0, dict(line='sight', text='I can see flour dust in the morning light.'))]),
    ('perception_hearing', run([turn('What do you hear?')]), False, [(0, dict(line='hearing_quiet'))]),
    ('perception_name', run([turn("What's your name?")]), False, [(0, dict(text='My name is Mira.'))]),
    ('prop_describe', run([turn('Look at the oven.')]), False, [(0, dict(route='props', prop='oven', text='A wide brick oven, still warm from the morning bake.'))]),
    ('prop_describe_note', run([turn('What is the bell?')]), False, [(0, dict(line='prop_note', text='A small brass bell hangs above the door. It rang twice this morning.'))]),
    # Predeclared route='model' was wrong: 'look' is a command verb, so an absent prop reaches the
    # command grammar and asks for a listed choice (same as Lantern Vault). Corrected after observation.
    ('prop_absent', run([turn('Look at the bell.', scene=dict(SCENE, props=[dict(id='oven', note=None)]))]), False, [(0, dict(route='command', kind='clarify'))]),
    ('prop_interaction', run([turn('*stokes the oven*')]), False, [(0, dict(route='props', kind='action', prop='oven', interaction='stoke', decision=dict(action='use', target='prop:oven')))]),
    ('prop_interaction_plain', run([turn('stoke the oven')]), False, [(0, dict(kind='action', interaction='stoke'))]),
    ('prop_gesture', run([turn('*warms their hands by the oven*')]), False, [(0, dict(kind='action', decision=None, narration='warms their hands by the oven'))]),
    ('prop_blocked', run([turn('*rings the bell*')]), False, [(0, dict(kind='clarify', intent='stage_action_blocked', prop='bell'))]),
    ('props_disabled', run([turn('*stokes the oven*')], features=dict(props=dict(enabled=False))), False, [(0, dict(route='direction', kind='clarify'))]),
    ('stage_action', run([turn('*buys a loaf*')]), False, [(0, dict(route='stage_action', kind='action', id='buy_bread'))]),
    ('stage_gesture', run([turn('*bows politely*')]), False, [(0, dict(kind='action', decision=None))]),
    ('direction_approach', run([turn('*approaches the baker*')]), False, [(0, dict(kind='direction', verb='approach', target='baker'))]),
    ('direction_enter', run([turn('*walks into the market street*')]), False, [(0, dict(kind='direction', verb='enter', target='street'))]),
    ('direction_unknown', run([turn('*dances wildly*')]), False, [(0, dict(kind='clarify', intent='direction_unsupported'))]),
    ('directions_disabled', run([turn('*dances wildly*')], features=dict(directions=dict(enabled=False))), False, [(0, dict(route='model'))]),
    ('command', run([turn('buy the bread')]), False, [(0, dict(route='command', kind='action', decision=dict(action='buy', target='item:bread')))]),
    ('command_clarify', run([turn('buy the cake')]), False, [(0, dict(route='command', kind='clarify', intent='command_clarify'))]),
    ('command_question', run([turn('can I buy bread?')]), False, [(0, dict(route='model'))]),
    ('ooc', run([turn('OOC: shorter replies please')]), False, [(0, dict(route='out_of_character', note='shorter replies please'))]),
    ('ooc_disabled', run([turn('OOC: shorter replies please')], features=dict(out_of_character=dict(enabled=False))), False, [(0, dict(route='model'))]),
    ('disclosure_recall', run([turn('My name is Arin.'), turn('What is my name?', token=2, at=2), turn('I like rye bread', token=3, at=3), turn('What do I like?', token=4, at=4)]), False,
     [(0, dict(text='Arin; thank you for telling me your name.')), (1, dict(text='You told me your name is Arin.')), (2, dict(text='You like rye bread; thank you for telling me.')), (3, dict(text='You told me you like rye bread.'))]),
    ('disclosure_correction', run([turn('Call me Arin, not Aron.')]), False, [(0, dict(line='name_corrected', text='Arin, not Aron; thank you for correcting me.'))]),
    ('disclosure_compound_ignored', run([turn('My name is Arin. Buy bread.')]), False, [(0, dict(route='model'))]),
    ('recall_unknown', run([turn('Who am I?')]), False, [(0, dict(line='recall_name_unknown'))]),
    ('model_turn', run([turn('Tell me a story about the sea.')]), False, [(0, dict(route='model', kind='model'))]),
    ('pack_none', run([turn('Hello!'), turn('Where are we?', token=2), turn('Thanks', token=3)], pack='none'), False,
     [(0, dict(route='model')), (1, dict(route='model')), (2, dict(route='model'))]),
    ('all_off', run([turn('Hello!')], features={f: dict(enabled=False) for f in ['chunks', 'out_of_character', 'stage_actions', 'props', 'directions', 'perception', 'commands', 'disclosures', 'recall', 'greetings', 'social', 'model']}), False,
     [(0, dict(route='none', kind='silent'))]),
    ('override_line', run([turn('Thanks')], lines=[dict(id='thanks', intent='thanks', variants=['Any time, friend.'])]), False, [(0, dict(line='thanks', text='Any time, friend.'))]),
    ('custom_line_first', run([turn('Thanks', trust=60)], lines=[dict(id='thanks_close', intent='thanks', when=dict(trust=dict(min=50)), variants=['For you, always.'])]), False, [(0, dict(line='thanks_close'))]),
    ('custom_phrases', run([turn('Hail')], phrases=dict(ask_name=dict(exact=['hail'])), lines=[]), False, [(0, dict(intent='ask_name', text='My name is Mira.'))]),
    ('long_message', run([turn(('I walked here along the river. ' * 20).strip())]), False, [(0, dict(route='model', long=True))]),
    ('unknown_token', run([heard('Hi.', token=9)]), False, [(0, dict(recorded=False, reason='unknown'))]),
    # Malformed or host-bug inputs.
    ('bad_unknown_key', run([turn('Hi', mood='happy')]), True, []),
    ('bad_listener_is_actor', run([turn('Hi', listener='baker')]), True, []),
    ('bad_scene_place', run([turn('Hi', scene=dict(SCENE, place='castle'))]), True, []),
    ('bad_unknown_prop', run([turn('Hi', scene=dict(SCENE, props=[dict(id='anvil', note=None)]))]), True, []),
    ('bad_template_var', run([], lines=[dict(id='x', intent='thanks', variants=['Hi {password}.'])]), True, []),
    ('bad_unbalanced_brace', run([], lines=[dict(id='x', intent='thanks', variants=['Hi {name.'])]), True, []),
    ('bad_feature', run([], features=dict(telepathy=dict(enabled=True))), True, []),
    ('bad_max_chars', run([], features=dict(chunks=dict(max_chars=99))), True, []),
    ('bad_duplicate_prop_name', run([], features=dict(props=dict(registry=dict(version=1, props=[PROPS['props'][0], dict(PROPS['props'][1], id='bell2', aliases=['oven'])])))), True, []),
    ('bad_delivered_mismatch', run([turn('Thanks'), dict(op='delivered', args=dict(token=1, listener='player_1', text='Something else.', heard=15))]), True, []),
    ('bad_duplicate_token', run([turn('Thanks'), turn('Thanks')]), True, []),
    ('bad_control_text', run([turn('Hi\u0007')]), True, []),
    ('bad_trust', run([turn('Hi', trust=101)]), True, []),
]


# Stage 2.1b: world (codex) and character (sheet) definitions. Original example content.
CODEX = dict(version=1, places=[
    dict(id='bakery', label='the bakery',
         spots=[dict(id='counter', label='the counter', aliases=['counter'], description='A flour-dusted oak counter with a brass till.')],
         ambience=[dict(when={'state.time': ['morning']}, smell='fresh rye bread', light='warm morning sunlight', sound='the oven fire crackling', temperature='warm'),
                   dict(smell='cooling loaves', light='dim lamplight')],
         norms=[dict(id='no_fighting', actions=[dict(action='attack', target=None)], reaction='Not in my bakery! Take that outside.')]),
    dict(id='market_street', label='the market street', ambience=[dict(sound='traders calling out prices')]),
    dict(id='cellar', label='the cellar')],
    entrances=[
        dict(id='front_door', kind='door', label='the green front door', aliases=['front door', 'door'], carries=dict(sound='always'),
             sides=[dict(place='bakery', description='A green door with a brass bell above it.', travel=dict(action='go', target='room:street')),
                    dict(place='market_street', description="The bakery's green door; it smells of bread.", travel=dict(action='go', target='room:bakery'))]),
        dict(id='cellar_hatch', kind='hatch', label='the cellar hatch', aliases=['hatch'],
             sides=[dict(place='bakery', description='A heavy wooden hatch set into the floor.', travel=dict(action='go', target='room:cellar')),
                    dict(place='cellar', description='A ladder leads up to the hatch.')])],
    factions=[dict(id='bakers_guild', label='the Bakers Guild', aliases=['guild', 'bakers guild'], description='The guild sets fair prices for bread in town.',
                   relations=[dict(faction='night_market', stance='wary')]),
              dict(id='night_market', label='the Night Market', aliases=['night market'], description='Traders who sell after dark, few questions asked.')],
    events=[dict(id='harvest_fair', label='the harvest fair', aliases=['fair', 'harvest fair'], description='Stalls fill the square for three days.', start=20, end=25, place='market_street')],
    quests=[dict(id='lost_ring', label='the lost ring', aliases=['my ring', 'ring'], givers=['baker'],
                 stages=[dict(id='asked', description='My ring slipped off while I was kneading. Please look around the cellar.'),
                         dict(id='found', description='You found it! I cannot thank you enough.')])])
SHEET = dict(version=1,
    persona=dict(pronouns='she/her', role='the town baker', summary='I have baked bread here for twenty years, and I know every regular by name.', values=['honest work']),
    speech=dict(formality='casual', avoid_words=['sir']),
    services=[dict(id='sell_bread', label='fresh bread', description="A loaf of the day's bread.", phrases=['sell me bread', 'i want to buy bread'], decision=dict(action='buy', target='item:bread')),
              dict(id='bake_cake', label='a celebration cake', description='A cake baked to order.', phrases=['bake me a cake'], decision=dict(action='order', target='item:cake'), when={'state.guild_member': [True]})],
    goals=[dict(id='win_fair', description='I want my rye to win a ribbon at the harvest fair.', priority=60),
           dict(id='find_ring', description='Honestly, I just want my ring back.', priority=80, when={'state.ring_lost': [True]})],
    needs=[dict(id='rest', label="a good night's sleep", threshold=70)],
    relationships=[dict(**{'with': 'tovin'}, name='Tovin', kind='brother', description='Tovin is my younger brother; he keeps the old cistern.'),
                   dict(**{'with': 'ilen'}, name='Ilen', kind='supplier')],
    factions=[dict(faction='bakers_guild', role='member')],
    secrets=[dict(id='flour_debt', phrases=['debt', 'money troubles', 'owe'], reveal='Between us, I owe the mill for three months of flour.', when=dict(trust=dict(min=50)), cover='My accounts are in perfect order, thank you.'),
             dict(id='tunnels', phrases=['cellar'], reveal='The cellar has a second door to the old tunnels.', when={'state.trusted_friend': [True]})],
    opinions=[dict(id='taxes', phrases=['taxes', 'tax'], variants=['The new taxes hit small bakeries hardest.', 'Taxes again? Do not get me started.'])])
CHOICES_B = CHOICES + [dict(action='attack', target='npc:rat')]
ENTRANCES = [dict(id='front_door', status='open'), dict(id='cellar_hatch', status='locked')]


def runb(ops, **over):
    c = cfg(**over); c.setdefault('codex', CODEX); c.setdefault('sheet', SHEET); return dict(config=c, state=None, ops=ops)
def turnb(text, token=1, at=1, **over):
    t = dict(listener='player_1', text=text, at=at, token=token, scene=dict(SCENE), choices=CHOICES_B, entrances=ENTRANCES, labels=[dict(id='npc:rat', aliases=['rat'])])
    t.update(over); return dict(op='turn', args=t)
HATCH_HIDDEN = [dict(id='front_door', status='open'), dict(id='cellar_hatch', status='hidden')]
PROFILE = 'I have baked bread here for twenty years, and I know every regular by name.'

CASES_B = [
    ('b_exits', runb([turnb('How do I get out?')]), False, [(0, dict(route='entrances', text='You can leave by the green front door and the cellar hatch.'))]),
    ('b_exits_hidden', runb([turnb('How do I get out?', entrances=HATCH_HIDDEN)]), False, [(0, dict(text='You can leave by the green front door.'))]),
    ('b_traverse', runb([turnb('*walks through the front door*')]), False, [(0, dict(route='entrances', kind='action', entrance='front_door', decision=dict(action='go', target='room:street')))]),
    ('b_traverse_locked', runb([turnb('go through the cellar hatch')]), False, [(0, dict(kind='clarify', intent='entrance_blocked', text="It's locked.", entrance='cellar_hatch'))]),
    ('b_traverse_hidden', runb([turnb('go through the hatch', entrances=HATCH_HIDDEN)]), False, [(0, dict(route='command', kind='clarify'))]),
    ('b_describe_entrance', runb([turnb('Look at the hatch.')]), False, [(0, dict(line='entrance_locked', text='A heavy wooden hatch set into the floor. It is locked.'))]),
    ('b_hear_through', runb([turnb('What do you hear?')]), False, [(0, dict(line='hearing_through', text='Through the green front door, I can hear traders calling out prices.'))]),
    ('b_morning', runb([turnb('What do you smell?', state=dict(time='morning')), turnb('What do you hear?', token=2, state=dict(time='morning'))]), False,
     [(0, dict(text='I can smell fresh rye bread.')), (1, dict(text='I can hear the oven fire crackling.'))]),
    ('b_evening_smell', runb([turnb('What can you smell?')]), False, [(0, dict(text='I can smell cooling loaves.'))]),
    ('b_temperature_none', runb([turnb('Is it cold in here?')]), False, [(0, dict(line='temperature_none'))]),
    ('b_spot_approach', runb([turnb('*walks over to the counter*')]), False, [(0, dict(kind='direction', verb='approach', target='counter', spot=True))]),
    ('b_spot_describe', runb([turnb('Look at the counter.')]), False, [(0, dict(route='scenery', text='A flour-dusted oak counter with a brass till.', spot='counter'))]),
    ('b_role_self', runb([turnb('What do you do?'), turnb('Tell me about yourself.', token=2)]), False, [(0, dict(text="I'm the town baker.")), (1, dict(text=PROFILE))]),
    ('b_services', runb([turnb('What do you offer?')]), False, [(0, dict(text='I can offer fresh bread.'))]),
    ('b_services_member', runb([turnb('What do you offer?', state=dict(guild_member=True))]), False, [(0, dict(text='I can offer fresh bread and a celebration cake.'))]),
    ('b_service_request', runb([turnb('I want to buy bread')]), False, [(0, dict(route='services', kind='action', service='sell_bread', decision=dict(action='buy', target='item:bread')))]),
    ('b_service_gated', runb([turnb('Bake me a cake')]), False, [(0, dict(route='model'))]),
    ('b_service_unavailable', runb([turnb('Bake me a cake', state=dict(guild_member=True))]), False, [(0, dict(kind='clarify', intent='service_unavailable', service='bake_cake'))]),
    ('b_goals', runb([turnb('What do you want?'), turnb('What do you want?', token=2, state=dict(ring_lost=True))]), False,
     [(0, dict(route='goals', goal='win_fair', text='I want my rye to win a ribbon at the harvest fair.')), (1, dict(goal='find_ring'))]),
    ('b_need', runb([turnb('How are you?', needs=dict(rest=85)), turnb('How are you?', token=2, needs=dict(rest=10))]), False,
     [(0, dict(line='wellbeing_need', text="I'm managing, though I could really use a good night's sleep.")), (1, dict(line='wellbeing'))]),
    ('b_relationships', runb([turnb('Who is Tovin?'), turnb('Do you know Ilen?', token=2)]), False,
     [(0, dict(route='relationships', person='tovin', text='Tovin is my younger brother; he keeps the old cistern.')), (1, dict(text='Ilen is my supplier.'))]),
    ('b_factions', runb([turnb('Tell me about the guild.'), turnb('What about the night market?', token=2)]), False,
     [(0, dict(line='faction_member', text='I belong to the Bakers Guild. The guild sets fair prices for bread in town.')),
      (1, dict(line='faction_wary', text="I'm wary of the Night Market. Traders who sell after dark, few questions asked."))]),
    ('b_event_timing', runb([turnb('Tell me about the fair.', at=15), turnb('Any news?', token=2, at=21), turnb('What about the harvest fair?', token=3, at=30)]), False,
     [(0, dict(line='event_soon', text='Soon there will be the harvest fair. Stalls fill the square for three days.')),
      (1, dict(line='events', text='Happening now or soon: the harvest fair.')), (2, dict(line='event_over'))]),
    ('b_quest', runb([turnb('Tell me about my ring.', quests=dict(lost_ring='asked'))]), False,
     [(0, dict(route='quests', quest='lost_ring', text='My ring slipped off while I was kneading. Please look around the cellar.'))]),
    ('b_quest_unknown_stage', runb([turnb('Tell me about my ring.')]), False, [(0, dict(route='model'))]),
    ('b_secret_cover', runb([turnb('Do you owe anyone money?')]), False, [(0, dict(route='secrets', intent='cover', text='My accounts are in perfect order, thank you.', secret='flour_debt'))]),
    ('b_secret_reveal', runb([turnb('Do you owe anyone money?', trust=60)]), False, [(0, dict(intent='reveal', text='Between us, I owe the mill for three months of flour.'))]),
    ('b_secret_withheld', runb([turnb('What is in the cellar?')]), False, [(0, dict(intent='secret_withheld', text='That is not something I will talk about.'))]),
    ('b_opinion_variety', runb([turnb('What do you think about taxes?'), heard('The new taxes hit small bakeries hardest.'), turnb('Taxes?', token=2)]), False,
     [(0, dict(route='opinions', opinion='taxes')), (2, dict(text='Taxes again? Do not get me started.'))]),
    ('b_norm', runb([turnb('attack the rat')]), False,
     [(0, dict(route='command', kind='action', decision=dict(action='attack', target='npc:rat'), norm=dict(id='no_fighting', reaction='Not in my bakery! Take that outside.')))]),
    ('b_content', runb([turnb('Tell me a story with lots of blood')], features=dict(content=dict(avoid=[dict(id='gore', phrases=['gore', 'blood'])]))), False,
     [(0, dict(route='content', text="I'd rather not talk about that.", topic='gore'))]),
    ('b_avoid_words', runb([turnb('Thanks')], lines=[dict(id='thanks', intent='thanks', variants=['Thank you kindly, sir.', 'You are most welcome.'])]), False,
     [(0, dict(text='You are most welcome.'))]),
    ('b_new_features_off', runb([turnb('How do I get out?'), turnb('What do you do?', token=2), turnb('Do you owe anyone money?', token=3)],
                                 features={f: dict(enabled=False) for f in ['content', 'entrances', 'scenery', 'persona', 'services', 'secrets', 'opinions', 'relationships', 'factions', 'events', 'quests', 'goals']}), False,
     [(0, dict(route='model')), (1, dict(route='model')), (2, dict(route='model'))]),
    ('b_bad_entrance_id', runb([turnb('Hi', entrances=[dict(id='window', status='open')])]), True, []),
    ('b_bad_entrance_status', runb([turnb('Hi', entrances=[dict(id='front_door', status='ajar')])]), True, []),
    ('b_bad_need', runb([turnb('Hi', needs=dict(hunger=50))]), True, []),
    ('b_bad_quest_stage', runb([turnb('Hi', quests=dict(lost_ring='sold'))]), True, []),
    ('b_bad_state_key', runb([turnb('Hi', state={'bad key': 1})]), True, []),
    ('b_bad_sheet_faction', runb([], sheet=dict(SHEET, factions=[dict(faction='pirates')])), True, []),
    ('b_bad_codex_side', runb([], codex=dict(CODEX, entrances=[dict(id='x', label='the x', sides=[dict(place='moon', description='Grey dust.')])])), True, []),
    ('b_bad_secret_no_when', runb([], sheet=dict(SHEET, secrets=[dict(id='s', phrases=['s'], reveal='r')])), True, []),
    ('b_bad_condition_key', runb([], sheet=dict(SHEET, goals=[dict(id='g', description='d', priority=1, when=dict(mood=['happy']))])), True, []),
]


ASK = 'What do you think of the new mayor?'
def ans(text, token=1, who='player_1'): return dict(op='reply', args=dict(token=token, listener=who, text=text))
def sp(s): return '{"speech": ' + json.dumps(s) + '}'
OK = 'The new mayor seems fair enough to me.'
CASES_C = [
    ('c_request_shape', runb([turnb(ASK)]), False, [(0, dict(route='model', kind='model', input=ASK))]),
    ('c_reply_accepted', runb([turnb(ASK), ans(sp(OK)), heard(OK)]), False,
     [(1, dict(route='model', kind='reply', intent='model', line=None, text=OK)), (2, dict(recorded=True))]),
    ('c_reply_fenced', runb([turnb(ASK), ans('```json\n' + sp(OK) + '\n```')]), False, [(1, dict(kind='reply', text=OK))]),
    ('c_reply_escapes', runb([turnb(ASK), ans('{"speech":"Caf\\u00e9 bread, \\"fresh\\" today."}')]), False, [(1, dict(kind='reply', text='Café bread, "fresh" today.'))]),
    ('c_reject_format', runb([turnb(ASK), ans('Sure! ' + OK), ans('{"speech": "Hi.", "mood": "happy"}'), ans('{"speech": "\\ud800"}')]), False,
     [(1, dict(kind='rejected', reason='format')), (2, dict(kind='rejected', reason='format')), (3, dict(kind='rejected', reason='format'))]),
    ('c_reject_empty', runb([turnb(ASK), ans(sp('   '))]), False, [(1, dict(kind='rejected', reason='empty'))]),
    ('c_reject_length', runb([turnb(ASK, features=None) if False else turnb(ASK), ans(sp('a' * 241))]), False, [(1, dict(kind='rejected', reason='length'))]),
    ('c_reject_control', runb([turnb(ASK), ans(sp('Line one.\nLine two.'))]), False, [(1, dict(kind='rejected', reason='control'))]),
    ('c_reject_stage', runb([turnb(ASK), ans(sp('*smiles* Hello there.')), ans(sp('Hello <b>there</b>.'))]), False,
     [(1, dict(kind='rejected', reason='stage')), (2, dict(kind='rejected', reason='stage'))]),
    ('c_reject_sentences', runb([turnb(ASK), ans(sp('One. Two! Three? Four.')), ans(sp('One. Two! Three. And four'))]), False,
     [(1, dict(kind='rejected', reason='sentences')), (2, dict(kind='rejected', reason='sentences'))]),
    ('c_ellipsis_one_sentence', runb([turnb(ASK), ans(sp('Well... he means well."'))]), False, [(1, dict(kind='reply'))]),
    ('c_reject_ooc', runb([turnb(ASK), ans(sp('As an AI, I have no view on mayors.')), ans(sp('That is not in my instructions.'))]), False,
     [(1, dict(kind='rejected', reason='out_of_character')), (2, dict(kind='rejected', reason='out_of_character'))]),
    ('c_reject_avoided', runb([turnb(ASK), ans(sp('Of course, sir.')), ans(sp('There was blood on the mayor.'))], features=dict(content=dict(avoid=[dict(id='gore', phrases=['blood'])]))), False,
     [(1, dict(kind='rejected', reason='avoided')), (2, dict(kind='rejected', reason='avoided'))]),
    ('c_reject_secret', runb([turnb(ASK), ans(sp('Between us, I owe the mill for three months of flour.'))]), False, [(1, dict(kind='rejected', reason='secret'))]),
    ('c_secret_after_reveal', runb([turnb('Do you owe anyone money?', trust=60), heard('Between us, I owe the mill for three months of flour.'), turnb(ASK, token=2),
                                    ans(sp('Between us, I owe the mill for three months of flour.'), token=2)]), False, [(3, dict(kind='rejected', reason='repeat'))]),
    ('c_reject_identifier', runb([turnb(ASK), ans(sp('Hello player_1, welcome.'))]), False, [(1, dict(kind='rejected', reason='identifier'))]),
    ('c_reject_repeat_then_retry', runb([turnb('Thanks'), heard('You are welcome.'), turnb(ASK, token=2), ans(sp('You are welcome!'), token=2), ans(sp(OK), token=2)]), False,
     [(3, dict(kind='rejected', reason='repeat')), (4, dict(kind='reply', text=OK))]),
    ('c_ticket_consumed', runb([turnb(ASK), ans(sp(OK)), ans(sp(OK))]), True, []),
    ('c_ticket_wrong_listener', runb([turnb(ASK), ans(sp(OK), who='player_2')]), True, []),
    ('c_token_reuse', runb([turnb(ASK), turnb(ASK)]), True, []),
    ('c_brief_trimmed', runb([turnb(ASK)], features=dict(model=dict(brief_chars=512))), False, [(0, dict(route='model'))]),
    ('c_model_params', runb([turnb(ASK)], features=dict(model=dict(max_tokens=64, max_sentences=1, max_chars=120))), False, [(0, dict(route='model'))]),
    ('c_custom_rules', runb([turnb(ASK)], model_rules=['You are {name}. Answer in {sentences} sentence.']), False, [(0, dict(route='model'))]),
    ('c_bad_rules', runb([turnb(ASK)], model_rules=['You are {nobody}.']), True, []),
    ('c_bad_model_param', runb([turnb(ASK)], features=dict(model=dict(max_sentences=9))), True, []),
    ('c_none_pack_no_rules', runb([turnb(ASK)], pack='none'), False, [(0, dict(route='model'))]),
]

SHEET_D = dict(version=1, speech=dict(quirks=['adjusts her flour-dusted apron', 'taps the wooden counter']),
               features=dict(eyes='hazel', hair='braided auburn', skin='fair', scars=['flour burn on left forearm'], height=1.68, build='slender'),
               clothing=['a white flour-dusted apron', 'rolled canvas sleeves'],
               state=dict(posture='standing behind the counter', activity='kneading a batch of sourdough', awareness='alert', expression='gentle smile'),
               sensory=dict(scent='warm yeast and toasted grain', voice_tone='warm and soft'))

CASES_D = [
    ('d_addressing_pause_ellipsis', run([turn('I was just thinking...')]), False, [(0, dict(route='addressing', kind='pause', reason='unfinished'))]),
    ('d_addressing_pause_dash', run([turn('Wait, let me--')]), False, [(0, dict(route='addressing', kind='pause', reason='unfinished'))]),
    ('d_addressing_bystander_audience', run([turn('Hello there', audience='tovin')]), False, [(0, dict(route='addressing', kind='silent', audience='tovin'))]),
    ('d_addressing_vocative_other', run([turn('Tovin, do you have the key?', targets=TARGETS + [dict(kind='character', id='tovin', names=['Tovin'])])]), False, [(0, dict(route='addressing', kind='silent', audience='tovin'))]),
    ('d_addressing_vocative_self', run([turn('Hello, Mira')]), False, [(0, dict(route='greeting', introduced=True))]),
    ('d_addressing_out_of_earshot_dist', run([turn('Hello!', distance='distant')]), False, [(0, dict(route='addressing', kind='silent', reason='out_of_earshot'))]),
    ('d_addressing_out_of_earshot_coords', run([turn('Hello!', coords=[0.0, 0.0, 0.0], speaker_coords=[50.0, 0.0, 0.0])]), False, [(0, dict(route='addressing', kind='silent', reason='out_of_earshot'))]),
    ('d_addressing_facing_other', run([turn('Hello there', speaker_facing='tovin', targets=[dict(kind='character', id='tovin', names=['Tovin'], distance='near')])]), False,
     [(0, dict(route='addressing', kind='silent', audience='tovin'))]),
    ('d_addressing_quirk_cadence', run([turn('Hello there', at=3, audience='tovin')], sheet=SHEET_D), False,
     [(0, dict(route='addressing', kind='action', action='quirk', audience='tovin', narration='taps the wooden counter'))]),
    ('d_stage_gesture_occluded', run([turn('*bows politely*', line_of_sight=False)]), False, [(0, dict(route='stage_action', kind='silent', reason='occluded'))]),
    ('d_scenery_floor', runb([turnb('Look down.', scene=dict(SCENE, floor='worn oak floorboards'))]), False,
     [(0, dict(route='scenery', intent='ask_floor', line='floor', text='The floor is worn oak floorboards.'))]),
    ('d_scenery_floor_none', runb([turnb('What are we standing on?')]), False,
     [(0, dict(route='scenery', intent='ask_floor', line='floor_none', text='Nothing unusual about the ground here.'))]),
    ('d_scenery_ceiling', runb([turnb('Look up.', scene=dict(SCENE, ceiling='vaulted stone arches'))]), False,
     [(0, dict(route='scenery', intent='ask_ceiling', line='ceiling', text='Above us is vaulted stone arches.'))]),
    ('d_scenery_ceiling_sky', runb([turnb('Look at the sky.', scene=dict(SCENE, exterior=True))]), False,
     [(0, dict(route='scenery', intent='ask_ceiling', line='ceiling_sky', text='The open sky stretches above.'))]),
    ('d_scenery_ceiling_none', runb([turnb('What is above us?')]), False,
     [(0, dict(route='scenery', intent='ask_ceiling', line='ceiling_none', text='Nothing unusual above us.'))]),
    ('d_persona_clothing_turn', run([turn('What are you wearing?', clothing=['a woolen cloak', 'leather boots'])]), False,
     [(0, dict(route='persona', intent='ask_clothing', line='clothing', text="I'm wearing a woolen cloak and leather boots."))]),
    ('d_persona_clothing_sheet', run([turn('What are you wearing?')], sheet=SHEET_D), False,
     [(0, dict(route='persona', intent='ask_clothing', line='clothing', text="I'm wearing a white flour-dusted apron and rolled canvas sleeves."))]),
    ('d_persona_clothing_none', run([turn('What are you wearing?')]), False,
     [(0, dict(route='persona', intent='ask_clothing', line='clothing_none', text="Nothing special about what I'm wearing."))]),
    ('d_persona_appearance_sheet', run([turn('What do you look like?')], sheet=SHEET_D), False,
     [(0, dict(route='persona', intent='ask_appearance', line='appearance', text='I have hazel eyes and braided auburn hair.'))]),
    ('d_persona_appearance_none', run([turn('What do you look like?')]), False,
     [(0, dict(route='persona', intent='ask_appearance', line='appearance_none', text='Just an ordinary look, really.'))]),
    ('d_persona_activity_turn', run([turn('What are you doing?', activity='reading a recipe book')]), False,
     [(0, dict(route='persona', intent='ask_activity', line='activity', text='I am reading a recipe book.'))]),
    ('d_persona_activity_sheet', run([turn('What are you doing?')], sheet=SHEET_D), False,
     [(0, dict(route='persona', intent='ask_activity', line='activity', text='I am kneading a batch of sourdough.'))]),
    ('d_persona_activity_none', run([turn('What are you doing?')]), False,
     [(0, dict(route='persona', intent='ask_activity', line='activity_none', text='Not doing much right now.'))]),
    ('d_prop_matter_custom_line', run([turn('Look at the bell.', scene=dict(SCENE, props=[dict(id='bell', note=None)]))],
                                      features=dict(props=dict(registry=dict(version=1, props=[
                                          dict(id='bell', label='the brass bell', aliases=['bell'], description='A small brass bell.',
                                               material='cast bronze', color='aged gold', texture='smooth', interactions=[])
                                      ]))),
                                      lines=[dict(id='prop_matter', intent='describe_prop', when=dict(material=['cast bronze']),
                                                  variants=['{prop} is made of {material} with an {color} hue.'])]), False,
     [(0, dict(route='props', line='prop_matter', text='the brass bell is made of cast bronze with an aged gold hue.'))]),
    ('d_prop_occluded_los', run([turn('Look at the bell.', scene=dict(SCENE, props=[dict(id='oven', note=None), dict(id='bell', note=None, line_of_sight=False)]))]), False,
     [(0, dict(route='command', kind='clarify'))]),
    # Malformed schema cases
    ('d_bad_dimensions_len', run([turn('Hi', dimensions=[1.0, 2.0])]), True, []),
    ('d_bad_dimensions_negative', run([turn('Hi', dimensions=[-1.0, 2.0, 3.0])]), True, []),
    ('d_bad_coords_len', run([turn('Hi', coords=[1.0])]), True, []),
    ('d_bad_prop_mass', run([turn('Hi', scene=dict(SCENE, props=[dict(id='bell', note=None, mass=-2.0)]))]), True, []),
    ('d_bad_prop_state', run([turn('Hi', scene=dict(SCENE, props=[dict(id='bell', note=None, state='energy')]))]), True, []),
    ('d_bad_sheet_height', run([turn('Hi')], sheet=dict(version=1, features=dict(height=-1.5))), True, []),
    ('d_bad_sheet_unknown_key', run([turn('Hi')], sheet=dict(version=1, aura='golden')), True, []),
]

# Addressing follow-up: distance attenuation and group audiences (expectations written before running).
TOVIN = dict(kind='character', id='tovin', names=['Tovin'])
GROUP = ['tovin', 'ilen']
CASES_E = [
    ('e_addressing_too_far', run([turn('Hello there', distance='far')]), False,
     [(0, dict(route='addressing', kind='gesture', reason='too_far', audience='baker'))]),
    ('e_addressing_too_far_coords', run([turn('Hello there', coords=[0.0, 0.0, 0.0], speaker_coords=[8.0, 0.0, 0.0])]), False,
     [(0, dict(route='addressing', kind='gesture', reason='too_far', audience='baker'))]),
    ('e_addressing_within_distance', run([turn('Hello there', distance='conversational')]), False, [(0, dict(route='greeting', introduced=True))]),
    ('e_addressing_far_vocative_heard', run([turn('Hello, Mira', distance='far')]), False, [(0, dict(route='greeting', introduced=True))]),
    ('e_addressing_far_host_audience', run([turn('Hello there', distance='far', audience='baker')]), False, [(0, dict(route='greeting', introduced=True))]),
    ('e_addressing_far_setting', run([turn('Hello there', distance='far')], features=dict(addressing=dict(conversational_distance=12))), False,
     [(0, dict(route='greeting', introduced=True))]),
    ('e_addressing_far_other_closer', run([turn('Hello there', distance='far', targets=TARGETS + [dict(TOVIN, distance='near')])]), False,
     [(0, dict(route='addressing', kind='silent', audience='tovin'))]),
    ('e_addressing_group_includes_me', run([turn('Hello there', audience=['tovin', 'baker'])]), False, [(0, dict(route='greeting', introduced=True))]),
    ('e_addressing_group_bystander', run([turn('Hello there', audience=GROUP)]), False, [(0, dict(route='addressing', kind='silent', audience=GROUP))]),
    ('e_addressing_group_quirk', run([turn('Hello there', at=3, audience=GROUP)], sheet=SHEET_D), False,
     [(0, dict(route='addressing', kind='action', action='quirk', audience=GROUP, narration='taps the wooden counter'))]),
    ('e_addressing_off_group_ignored', run([turn('Hello there', audience=GROUP, distance='far')], features=dict(addressing=dict(enabled=False))), False,
     [(0, dict(route='greeting', introduced=True))]),
    ('e_bad_audience_empty', run([turn('Hi', audience=[])]), True, []),
    ('e_bad_audience_duplicate', run([turn('Hi', audience=['tovin', 'tovin'])]), True, []),
    ('e_bad_audience_too_many', run([turn('Hi', audience=['c%d' % i for i in range(17)])]), True, []),
    ('e_bad_audience_member', run([turn('Hi', audience=['tovin', 'not an id'])]), True, []),
]


def check_requests():
    """Expectation 10 on the reference itself: shape, order and secret gating."""
    r = oracle(copy.deepcopy(CASES_C[0][1]))['results'][0]['request']
    roles = [m['role'] for m in r['messages']]; assert roles == ['system', 'system', 'system', 'user'] and r['messages'][-1]['content'] == ASK
    brief = json.loads(r['messages'][1]['content']); sit = json.loads(r['messages'][2]['content'])
    assert brief['name'] == 'Mira' and brief['role'] == 'the town baker' and 'secrets' not in brief and list(sit)[:2] == ['scene', 'listener']
    assert 'mill' not in enc(r) and r['response_format']['json_schema']['schema']['required'] == ['speech']
    after = oracle(copy.deepcopy(CASES_C[14][1]))['results'][2]['request']
    assert 'I owe the mill' in after['messages'][2]['content']
    small = oracle(copy.deepcopy(CASES_C[20][1]))['results'][0]['request']
    assert len(small['messages'][1]['content']) + len(small['messages'][2]['content']) <= 512 or 'summary' not in small['messages'][1]['content']
    none = oracle(copy.deepcopy(CASES_C[-1][1]))['results'][0]['request']; assert len(none['messages']) == 3


def cases():
    rows = [(n, a, e, w) for n, a, e, w in CASES + CASES_B + CASES_C + CASES_D + CASES_E]
    # Checkpoint round trip: a saved state restores and continues identically.
    first = oracle(copy.deepcopy(run([turn('Hello!'), heard(FIRST), turn('My name is Arin.', token=2, at=2), heard('Arin; thank you for telling me your name.', token=2)])))
    rows.append(('restore_continues', dict(config=cfg(), state=first['state'], ops=[turn('Hello again', token=3, at=3), turn('What is my name?', token=4, at=4)]), False,
                 [(0, dict(line='greet_continuing')), (1, dict(text='You told me your name is Arin.'))]))
    bad = copy.deepcopy(first['state']); bad['pending'] = [dict(token=1, listener='player_1', text='x', scene='bakery', visit=0, at=0, greeting=False, introduced=True)]
    rows.append(('bad_restore_introduced', dict(config=cfg(), state=bad, ops=[]), True, []))
    return rows


def lantern_vault_wording():
    """Expectation 5: every Lantern Vault greeting/social line is in the English pack."""
    import json, re
    pack = json.loads((ROOT / 'src/character/roleplay/engine/en.json').read_text())
    variants = {v for r in pack['lines'] for v in r['variants']}
    web = ROOT / 'examples/lantern-vault/web'
    src = (web / 'greetings.rs').read_text().split('pub fn render')[1].split('let line =')[0] + (web / 'social.rs').read_text()
    lines = [x.replace('{label}', '{scene}') for x in re.findall(r'"((?:[^"\\]|\\.)*)"', src)]
    lines = [x for x in lines if ' ' in x and x[0].isupper()]
    missing = [x for x in lines if x not in variants]
    assert len(lines) >= 100 and not missing, missing
    return len(lines)


def main():
    p = argparse.ArgumentParser(); p.add_argument('--cap', required=True, type=float); p.add_argument('--prototype-only', action='store_true'); p.add_argument('--write-conformance', action='store_true')
    a = p.parse_args(); assert a.cap == 0, 'roleplay engine cases are offline only'
    rows = cases(); ported = lantern_vault_wording(); check_requests()
    for name, args, error, wants in rows:
        try:
            got = oracle(copy.deepcopy(args)); assert not error, name
        except (AssertionError, KeyError, TypeError, ValueError, StopIteration):
            assert error, name; got = None
        for i, want in wants:
            have = got['results'][i]
            assert {k: have.get(k) for k in want} == want, (name, i, have)
        req = dict(kind='roleplay-engine', args=args)
        if not a.prototype_only:
            x = subprocess.run([str(ROOT / 'target/debug/gamectx'), 'local', '-'], input=enc(req), capture_output=True, text=True)
            assert (x.returncode != 0) == error, (name, x.stderr)
            if not error: assert x.stdout.strip() == enc(got), (name, x.stdout, enc(got))
        if a.write_conformance:
            # New cases are written; existing ones must already match byte for byte.
            d = ROOT / 'conformance/roleplay-engine' / name; new = not d.exists(); d.mkdir(parents=True, exist_ok=True)
            for k, v in [('case', dict(op='local', **(dict(error=True) if error else {}))), ('input', req)] + ([] if error else [('expected', got)]):
                f = d / (k + '.json')
                if new: f.write_text(enc(v) + '\n')
                else: assert f.read_text() == enc(v) + '\n', f
    print(enc(dict(workflows=len(rows), lantern_vault_lines=ported, model_calls=0, api_spend_usd=0)))


if __name__ == '__main__':
    main()
