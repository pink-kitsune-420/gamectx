"""Independent perspective/gated compiler. No raw Life inspection enters the prompt."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy
import archetype_reference as r
from life_reference import run
from life_speech_reference import validate_turn,scoped_facts,GROUNDED
from lore_reference import Lore
def compile(a):
 r.fields(a,'life blueprint gates scene receipts lore request');l=a['life'];b=r.blueprint(a['blueprint']);s=r.Scene(a['scene']);q=a['request'];r.fields(q,'trigger revision life_revision listeners subjects tags turn rules stable_scene lore_select affordances tools')
 for receipt in a['receipts']:s.receipt(receipt)
 assert b['modules']['life'] and b['size']!='0b' and l['scope']==s.v['id'] and l['participants']==[p['id'] for p in s.v['participants']]
 assert any(p==dict(id=l['actor'],kind='ai') for p in s.v['participants']) and q['revision']==s.v['revision']
 trigger=next(x for x in s.lines if x['token']==q['trigger']);assert l['actor'] in trigger['heard'] and l['actor'] in trigger['recipients']
 t=validate_turn(q['turn'],l['setting']['entities'],l['participants']);assert t['speaker']==trigger['speaker'] and t['profile']==b['attention'] and len(t['facts'])<=b['caps']['facts']
 r.names(q['listeners']);r.names(q['subjects']);assert q['listeners'] and set(q['listeners']+q['subjects'])<=set(l['participants']);r.line(q['rules']);r.names(q['tags'],8)
 a1=copy.deepcopy(l);a1['operations'].append(dict(op='inspect',args={}));inspection=run(a1)['results'][-1]['result'];assert q['life_revision']==inspection['revision']
 cfg=dict(enabled=False,balanced=False,deception=False,mistaken=False,tools=[])
 for x in (l['defaults'],l['overrides']):cfg.update(x)
 assert cfg['enabled'] and not(cfg['balanced'] and not b['policies']['balanced']) and not(cfg['deception'] and not b['policies']['deception']) and not(cfg['mistaken'] and not b['policies']['mistaken'])
 def visible(x):return set(l['participants'])<=set(x['audience']) and (not q['tags'] or set(q['tags'])&set(x['tags']))
 def select(kind,items):return r.gate_select(dict(rules=a['gates'],actor=l['actor'],speaker=trigger['speaker'],listeners=q['listeners'],participants=l['participants'],revision=inspection['revision'],expected_revision=q['life_revision'],attitudes=[dict(kind=x['definition']['kind'],subject=x['definition']['subject'],level=x['level']) for x in inspection['attitudes']],kind=kind,items=items))
 facets=select('facet',[f for f in l['definition']['facets'] if visible(f)])
 subjects=list(dict.fromkeys([trigger['speaker']]+q['listeners']+q['subjects']))
 attitudes=[x for subject in subjects for x in inspection['attitudes'] if x['definition']['subject']==subject and visible(x['definition']) and (cfg['mistaken'] or x['definition']['kind']!='misconception')][:b['caps']['attitudes']]
 expressive=cfg['balanced'] and t['knowledge']=='known' and (t['profile']=='expanded' or t['creative'])
 from life_speech_reference import character
 base=character(l,t,b['caps'],q['tags']);base['facets']=facets['packet'][:b['caps']['facets']];base['attitudes']=attitudes
 lore=select('lore',a['lore']['entries']);sel=q['lore_select'];assert sel['actor']==l['actor'] and sel['limit']<=b['caps']['facts']
 required_denied=any(x['id'] in sel['required'] for x in lore['withheld']) if b['modules']['lore'] else False
 for f in q['affordances']:r.fields(f,'id text audience');r.name(f['id']);r.line(f['text']);r.names(f['audience'])
 offered=select('affordance',[x for x in q['affordances'] if set(l['participants'])<=set(x['audience'])])
 assert len(q['affordances'])<=32
 inspection=dict(facets_withheld=facets['withheld'],lore_withheld=lore['withheld'],affordances_withheld=offered['withheld'],scene_revision=s.v['revision'],life_revision=inspection['revision'])
 if required_denied:return dict(request=None,refusal='I cannot share that or offer it yet.',inspection=inspection)
 cards=Lore(dict(version=a['lore']['version'],entries=lore['packet'])).select(sel)['packet'] if b['modules']['lore'] else []
 assert len(cards)+len(t['facts'])<=b['caps']['facts']
 for tool in q['tools']:assert tool['type']=='function' and tool['function']['name'] in b['tools'] and tool['function']['name'] in cfg['tools']
 assert len(q['tools'])<=2
 stable={k:v for k,v in base.items() if k in ('facets','policy')};changing={k:v for k,v in base.items() if k not in ('facets','policy')}
 episodes=recall(inspection_events(l),trigger['text'],q['tags'],b['caps']['episodes']) if b['modules']['recall'] else []
 state=dict(character=changing,facts=scoped_facts(t,l['actor']),lore=cards,affordances=offered['packet'],episodes=episodes)
 history=s.render(l['actor'],b['caps']['exchanges']*2);label='['+trigger['speaker']+' -> '+','.join(trigger['recipients'])+'] '+trigger['text']
 # Exclude the current receipt from history; it appears once, as a direct request.
 prior=copy.deepcopy(s);prior.lines=[x for x in prior.lines if x['token']<trigger['token']];history=prior.render(l['actor'],b['caps']['exchanges']*2)
 messages=[dict(role='system',content=q['rules']),dict(role='system',content=r.encode(dict(character=stable,scene=q['stable_scene']))),dict(role='user',content=r.encode(state))]+history+[dict(role='system',content='Current utterance speaker: '+trigger['speaker']+'. First-person I/my refers to that speaker. Question subject: '+t['question_subject']+'.'),dict(role='user',content='Current request:\n'+label)]
 return dict(request=dict(messages=messages,tools=q['tools']),refusal=None,inspection=inspection)
def inspection_events(l):
 a=copy.deepcopy(l);a['operations'].append(dict(op='inspect',args={}));return run(a)['results'][-1]['result']['events']
def words(text):
 import re
 result=[]
 for w in re.findall(r'\w+',text.lower()):
  if w.isascii() and w.isdigit():continue
  for v in [w]+([w[:-1]] if len(w)>3 and w.endswith('s') else []):
   if v not in result:result.append(v)
 return result
def recall(events,text,tags,cap):
 wanted=words(text+' '+' '.join(tags));parts=[]
 for e in events:parts.append(set(words(e['text'])+[e['tag']]+([e['subject']] if e['subject'] in tags else [])))
 weights={w:1/sum(w in p for p in parts) if any(w in p for p in parts) else 0 for w in wanted}
 ranked=sorted(range(len(events)),key=lambda i:(-sum(weights[w] for w in wanted if w in parts[i]),-int(events[i]['retain']),-events[i]['at'],-i))
 for n in range(cap,0,-1):
  selected=set(ranked[:n]);old=None
  while old!=selected:
   old=set(selected)
   for i in list(selected):
    for j,e in enumerate(events):
     if e['token'] in events[i]['causes'] or events[i]['token'] in e['causes']:selected.add(j)
  if len(selected)<=cap:return sorted([copy.deepcopy(events[i]) for i in selected],key=lambda e:e['token'])
 return []
