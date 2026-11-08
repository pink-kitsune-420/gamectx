"""Independent v2 speech compiler contract, defined before model observations/Rust."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import copy,json
from life_reference import run
GROUNDED='State only the supplied facts, attitude, and unknown status in one short sentence; do not offer unlisted stock or speculate about unknown weather/history.'
RESERVED={'question_subject','utterance_speaker','fact_subjects'}
def encode(v):return json.dumps(v,ensure_ascii=False,separators=(',',':'))
def validate_turn(t,entities,participants):
 assert set(t)==set('profile knowledge creative speaker question_subject facts'.split())
 assert t['profile'] in ('focused','expanded') and t['knowledge'] in ('known','partial','unknown') and type(t['creative']) is bool
 assert t['speaker'] in participants and t['question_subject'] in entities
 assert len(t['facts'])<=32
 keys=[];unknown=known=0
 for f in t['facts']:
  assert set(f)==set('key subject status value'.split()) and f['subject'] in entities
  assert isinstance(f['key'],str) and 0<len(f['key'])<=64 and f['key'] not in RESERVED and f['key'] not in keys and not any(ord(c)<32 for c in f['key']);keys.append(f['key'])
  assert len(encode(f['value']).encode())<=1024
  if f['status']=='unknown':assert f['value'] is None;unknown+=1
  else:assert f['status']=='known' and f['value'] is not None;known+=1
 assert not(t['knowledge']=='known' and unknown)
 assert t['knowledge']!='partial' or unknown and known
 assert t['knowledge']!='unknown' or unknown
 return t
def scoped_facts(t,actor):
 result=dict(question_subject=t['question_subject'],utterance_speaker=t['speaker'],fact_subjects={f['key']:f['subject'] for f in t['facts']})
 for f in t['facts']:result[f['key']]='unknown_to_'+actor if f['status']=='unknown' else copy.deepcopy(f['value'])
 return result
def character(life,t,caps,tags):
 validate_turn(t,life['setting']['entities'],life['participants'])
 a=copy.deepcopy(life);a['operations'].append(dict(op='context',args=dict(tags=tags,limit=max(caps['facets'],caps['attitudes']))))
 out=run(a)['results'][-1]['result'];out['facets']=out['facets'][:caps['facets']];out['attitudes']=out['attitudes'][:caps['attitudes']]
 expressive=out['policy']['balanced'] and t['knowledge']=='known' and (t['profile']=='expanded' or t['creative'])
 if not expressive:out['policy'].update(balanced=False,improvisation='',expression=GROUNDED)
 return out
def compile_request(life,a):
 assert set(a)==set('version rules tags caps stable_scene state dialogue input tools turn'.split()) and a['version']==2
 caps=a['caps'];assert set(caps)==set('facts facets attitudes episodes exchanges'.split())
 assert all(type(caps[k]) is int and 0<=caps[k]<=n for k,n in [('facts',32),('facets',16),('attitudes',16),('episodes',16),('exchanges',16)])
 t=validate_turn(a['turn'],life['setting']['entities'],life['participants'])
 assert len(t['facts'])<=caps['facts'] and len(a['dialogue'])<=caps['exchanges'] and len(a['tools'])<=32
 for s in [a['rules'],a['input']]:assert isinstance(s,str) and 0<len(s)<=512 and len(s.encode())<=2048 and not any(ord(c)<32 for c in s)
 c=character(life,t,caps,a['tags']);stable={k:v for k,v in c.items() if k in ('facets','policy')};changing={k:v for k,v in c.items() if k not in ('facets','policy')}
 subject_rule="Current utterance speaker: "+t['speaker']+". First-person I/my in the current request refers to that speaker. Question subject: "+t['question_subject']+". Answer about that subject, not your own biography."
 messages=[dict(role='system',content=a['rules']),dict(role='system',content=encode(dict(character=stable,scene=a['stable_scene']))),dict(role='user',content=encode(dict(character=changing,state=dict(context=a['state'],facts=scoped_facts(t,life['actor'])))))]
 for e in a['dialogue']:
  assert set(e)=={'user','assistant'}
  messages.extend([dict(role='user',content=e['user']),dict(role='assistant',content=e['assistant'])])
 # Turn-dependent grounding/subject rules belong near the utterance, outside stable prefix.
 messages.append(dict(role='system',content=subject_rule))
 messages.append(dict(role='user',content='Current request:\n'+a['input']))
 return dict(messages=messages,tools=a['tools'])
def assess_sentence(s,cap):
 assert isinstance(s,str) and len(s)<=1024 and len(s.encode())<=4096 and 1<=cap<=1024
 body=s.rstrip().rstrip('\"\'”’)]').rstrip();complete=body.endswith(('.','!','?','。','！','？')) and not body.endswith(('...','…'))
 count=0;in_run=False
 for i,c in enumerate(s):
  terminal=c in '.!?。！？' and not(c=='.' and i and i+1<len(s) and s[i-1].isascii() and s[i-1].isdigit() and s[i+1].isascii() and s[i+1].isdigit())
  if terminal and not in_run:count+=1
  in_run=terminal
 return dict(characters=len(s),boundary_complete=complete,single_sentence=complete and count==1,at_schema_wall=len(s)==cap,over_limit=len(s)>cap,suspected_cutoff=len(s)==cap and not complete)
