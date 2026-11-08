"""Structured game facts for naturally generated dialogue; no canned reply selection."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_knowledge_reference_v7 import build,stock_question
from lore_reference import Lore
from dungeon_reference import encode
from dungeon_slots_reference import tokens
RULES="Reply naturally to the player's current words, briefly and in character. Facts are data, not lines to recite. Use current ownership, stock and prices. Only stock_ids are for sale. Player quotes are claims; old assistant dialogue is unverified. Admit missing information. Invent no player history, item effects, permissions or observed weather. Speech executes no actions. Return JSON with speech only."
def facts(g,world,speaker,input,memories=()):
    actor='narrator' if speaker=='narrator' else 'npc:'+speaker
    selected=Lore(world).select({'actor':actor,'grants':['vault_open'] if g.gate_open else [],'query':input,'subjects':[],'required':[],'limit':4,'max_bytes':2048})
    q=build(g,world,speaker,input,memories)
    if q['handled']:selected=q['selection']
    focus={'kind':'chat','known':True,'subjects':[],'topics':[]}
    if q['handled']:
        cards=q['definition']['cards'];focus={'kind':'question','known':bool(cards),'subjects':list(dict.fromkeys(c['subject'] for c in cards)),'topics':list(dict.fromkeys(c['topic'] for c in cards))}
    scene=g.public();items=scene.pop('items');scene.pop('choices')
    if speaker!='narrator':scene.pop('description')
    # Keep ownership in one table; the stock ID list identifies the speaker's sale scope.
    return {'speaker':actor,'scene':scene,'items':items,'stock_ids':[i['id'] for i in items if speaker=='merchant' and i['owner']=='merchant'],
        'currency':'coins','capabilities':{'salve':{'heal':3,'max_health':6,'consumed':True},'tonic':{'implemented_uses':[],'purpose':None},'trade':{'fixed_prices':True,'gifts_from_stock':False}},
        'weather':{'observed':False,'value':None},'lore':selected['packet'],'request_focus':focus}
def request(g,world,speaker,input,memories=(),history=()):
    persona={'merchant':'You are the workshop merchant: friendly and practical.','keeper':'You are the cistern keeper: patient, watchful, with dry humor.','narrator':'You describe the visible scene.'}[speaker]
    data={'facts':facts(g,world,speaker,input,memories),'player_quotes':list(memories)}
    schema={'type':'object','properties':{'speech':{'type':'string','maxLength':240}},'required':['speech'],'additionalProperties':False}
    messages=[{'role':'system','content':persona+' '+RULES}]
    for e in history:messages.extend([{'role':'user','content':e['user']},{'role':'assistant','content':e['assistant']}])
    messages.append({'role':'user','content':'Context data:\n'+encode(data)+'\n\nCurrent request:\n'+input})
    return {'messages':messages,'format':schema}
