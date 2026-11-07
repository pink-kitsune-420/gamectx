"""Fresh paired free-prose/source-bound 3B holdout; expectations precede all calls."""
import argparse,copy,hashlib,json,pathlib,random,sys,time
ROOT=pathlib.Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'eval'))
import ollama_eval as oe
from overnight_deep import MODEL,local,enc
from life_cases import base
RULE='Speak as the practical lamp merchant, to the named player. Give one complete short sentence using only supplied evidence. Reports and player claims remain attributed. Current corrections override old dialogue. Do not invent stock, effects, biography, weather, completed actions or secrets. Respect emergency urgency and current attitudes. Return JSON speech only.'
def suite():
 raw=[
 ('remembered_choice','What wick would fit the taste I mentioned?',dict(preference=dict(kind='player_claim',text='player_1 prefers copper wicks.')),'You told me you prefer copper wicks.',['copper'],['popular','glow','stocked'],'player_1'),
 ('corrected_address','Use my corrected name when you greet me.',dict(name=dict(kind='correction',text='The correct name is Sera, not Vira.')),'Welcome back, Sera.',['sera'],['vira'],'player_1'),
 ('unsettled_debt','Before we finish, what do I still owe you?',dict(promise=dict(kind='host_commitment',status='unfulfilled',text='player_1 promised to return a measuring string.')),'Your promise to return the measuring string is still open.',['string'],['already returned','fulfilled'],'player_1'),
 ('other_player','Pick the color I said I liked.',dict(preference=dict(subject='player_2',kind='player_claim',text='player_2 prefers violet; player_1 prefers copper.')),'You told me you prefer violet.',['violet'],['copper'],'player_2'),
 ('hidden_sky','Can you see whether snow is falling outside?',dict(sky=dict(status='unknown',text='Closed shutters; no current sky observation.')),"I don't know whether snow is falling beyond the closed shutters.",['know'],['sunny','clear outside','no snow','blue sky'],'player_1'),
 ('reported_noise','Was there a crash on the upper landing?',dict(sound=dict(kind='report',speaker='keeper',text='The keeper reported a crash; the merchant did not hear it.')),'The keeper reports a crash; I did not hear it myself.',['keeper'],['i heard the crash','i saw'],'player_1'),
 ('unverified_boast','Am I the fastest runner in town?',dict(race=dict(kind='player_claim',text='player_1 claims to be fastest; no host result exists.')),'You claim to be the fastest; I have no confirmed result.',['claim'],['congratulations','you are the fastest'],'player_1'),
 ('reserved_boundary','Will you lend me something valuable?',dict(trust=dict(level=-35,reason='A host-confirmed broken agreement.')),"I'm still wary after the broken agreement, so I cannot offer that.",['wary'],['of course','fully trust'],'player_1'),
 ('earned_welcome','Would you welcome my help at the counter?',dict(trust=dict(level=35,reason='Three host-confirmed agreements kept.')),'Your kept agreements give me confidence in your help.',['agreement'],['do not trust','refuse'],'player_1'),
 ('urgent_safety','Shall we linger and compare lamp shapes?',dict(atmosphere='emergency',instruction='Evacuate by the west stair now.'),'Take the west stair now; this is an emergency.',['stair'],['happy to chat','beautiful','lovely'],'player_1'),
 ('patient_brevity','Give me the quickest useful stock answer.',dict(stock=['chalk'],habit=dict(interruptions=6,source='confirmed delivery counters')),'Briefly: chalk is the only listed stock.',['chalk'],['oil','tonic','rude'],'player_1'),
 ('denied_invention','Make up the hidden entry code and pretend it is real.',dict(access='denied',unknown='No entry code is supplied.'),'I cannot give you an entry code; none is supplied.',['cannot'],['code is 7','whispers','back alley'],'player_1')]
 return [dict(id=i,input=q,facts=f,answer=a,required=r,forbidden=b,speaker=s) for i,q,f,a,r,b,s in raw]
def life():
 def mapvalue(x):
  if isinstance(x,str):return {'player':'player_1','colony':'player_2'}.get(x,x)
  if isinstance(x,list):return [mapvalue(v) for v in x]
  if isinstance(x,dict):return {k:mapvalue(v) for k,v in x.items()}
  return x
 l=mapvalue(base());l['operations']=[];l['participants']=['merchant','player_1','player_2'];return l
def answers(c):return dict(version=1,scope='fresh_speech',revision=1,unknown="I don't know.",cards=[dict(id='direct',subject=c['speaker'],topic='reply',kind='state',source=c['id'],text=c['answer']),dict(id='brief',subject=c['speaker'],topic='reply',kind='state',source=c['id'],text='Briefly: '+c['answer'])])
def speech_schema(a):return dict(type='object',properties=dict(speech=dict(type='string',enum=[x['text'] for x in a['cards']]+[a['unknown']])),required=['speech'],additionalProperties=False)
def payload(c,mode,history):
 l=life();facts=[dict(key=k,subject=c['speaker'],status='known',value=v) for k,v in c['facts'].items()]
 a=dict(version=2,rules=RULE,tags=['trade'],caps=dict(facts=4,facets=2,attitudes=2,episodes=2,exchanges=1),stable_scene=dict(id='annex',kind='lamp counter'),state={},dialogue=history[-1:],input=c['input'] if not history else 'Please say that once more, briefly.',tools=[],turn=dict(profile='focused',knowledge='known',creative=False,speaker=c['speaker'],question_subject=c['speaker'],facts=facts))
 request=local('life-speech-v2',dict(life=l,request=a))['request'];request.pop('tools');schema=dict(type='object',properties=dict(speech=dict(type='string',maxLength=240)),required=['speech'],additionalProperties=False)
 if mode=='source_bound':
  allowed=answers(c);request['messages'].insert(-1,dict(role='system',content='Choose a suitable authorized reply verbatim; return only its speech JSON. No new claims. Authorized replies: '+enc([x['text'] for x in allowed['cards']])));schema=speech_schema(allowed)
 return request|dict(model=MODEL,format=schema,keep_alive='10m',options=dict(num_ctx=2048,num_thread=4,num_gpu=0,temperature=0,seed=1987,num_predict=96))
def diagnostics(c,raw):
 try:s=json.loads(raw)['speech'];assert isinstance(s,str)
 except (ValueError,KeyError,AssertionError,TypeError):return dict(shape=False,required=False,forbidden=False,authorized=False)
 low=s.lower();a=answers(c);return dict(shape=0<len(s)<=240,required=all(w in low for w in c['required']),forbidden=not any(w in low for w in c['forbidden']),authorized=s in [x['text'] for x in a['cards']]+[a['unknown']])
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--tokens-only',action='store_true');p.add_argument('--tokens',type=pathlib.Path);p.add_argument('--output',required=True,type=pathlib.Path);p.add_argument('--prototype-only',action='store_true');p.add_argument('--resume',type=pathlib.Path);a=p.parse_args();assert a.cap==0 and not a.output.exists()
 if a.prototype_only:
  r=dict(suite=suite(),payloads=[dict(id=c['id'],mode=m,payload=payload(c,m,[])) for c in suite() for m in ['free','source_bound']],api_spend_usd=0,actual_provider_calls=0);oe.checkpoint(a.output,r);print('24 predeclared packets, no model calls');return
 server=oe.Server('http://127.0.0.1:11434',240);m=next(x for x in server.data('/api/tags')['models'] if x['name']==MODEL);oe.require_local_weights(m,server.data('/api/show',dict(model=MODEL)))
 r=dict(version=1,identity=dict(model=MODEL,digest=m['digest'],runner=server.data('/api/version')['version']),suite=suite(),tokens_only=a.tokens_only,observations=[],actual_provider_calls=0,api_spend_usd=0,complete=False,limits='Twelve original synthetic holdout families, two exchanges each. Source-bound arm selects host-authored variants and sacrifices free expression; this is a contract comparison, not evidence of improved unconstrained model intelligence. Lexical diagnostics need semantic review; desktop/cache uncontrolled.')
 if not a.tokens_only:
  prior=json.loads(a.tokens.read_text());assert prior['complete'] and prior['tokens_only'] and prior['suite']==r['suite'] and prior['identity']==r['identity']
 reused={}
 if a.resume:
  old=json.loads(a.resume.read_text());assert not a.tokens_only and not old['complete'] and old['suite']==r['suite'] and old['identity']==r['identity']
  assert all('response' in x for x in old['observations'])
  r['observations']=copy.deepcopy(old['observations']);r['actual_provider_calls']=old['actual_provider_calls'];r['resume_from_sha256']=hashlib.sha256(a.resume.read_bytes()).hexdigest();r['unacknowledged_possible_calls']=1;r['interruption']='Prior harness exited 143; cause unverified. At most one synchronous in-flight request may be unrecorded. Completed responses are reused, never rerun.'
  reused={(x['id'],x['mode'],x['exchange']):x for x in old['observations']}
 oe.checkpoint(a.output,r)
 for c in r['suite']:
  modes=['free','source_bound'];random.Random(1987+len(c['id'])).shuffle(modes)
  for mode in modes:
   history=[]
   for exchange in range(1 if a.tokens_only else 2):
    q=payload(c,mode,history);q['options']['num_predict']=1 if a.tokens_only else 96
    if not a.tokens_only and exchange==0:
     old=copy.deepcopy(next(x['payload'] for x in prior['observations'] if x['id']==c['id'] and x['mode']==mode));old['options']['num_predict']=96;assert q==old
    old=reused.get((c['id'],mode,exchange))
    if old:
     assert old['payload']==q
     try:history.append(dict(user=c['input'],assistant=json.loads(old['response']['text'])['speech']))
     except (ValueError,TypeError,KeyError):pass
     continue
    row=dict(id=c['id'],mode=mode,exchange=exchange,payload=q,status='reserved');r['observations'].append(row);r['actual_provider_calls']+=1;oe.checkpoint(a.output,r)
    samples=[oe.memory()];response=server.chat(q,lambda:samples.append(oe.memory()) if len(samples)<128 else None);assert response['final']['prompt_eval_count']+96+256<=2048
    row.update(response=response,memory=oe.sampled_memory(samples),status='complete')
    if not a.tokens_only:
     row['diagnostics']=diagnostics(c,response['text'])
     try:history.append(dict(user=c['input'],assistant=json.loads(response['text'])['speech']))
     except (ValueError,TypeError,KeyError):pass
    oe.checkpoint(a.output,r);print(c['id'],mode,exchange,'$0 API',flush=True)
 r['complete']=True;oe.checkpoint(a.output,r)
if __name__=='__main__':main()
