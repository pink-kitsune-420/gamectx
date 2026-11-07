"""Actual archived Life -> focused packets -> serial local cohort. Original simulation."""
import argparse,copy,hashlib,importlib.util,json,pathlib,subprocess,sys,time
ROOT=pathlib.Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'eval'))
import ollama_eval as oe
from overnight_deep import MODEL,local,enc
from character_cases import view
SPECS=[('preference','player_1',[1],'Which wick would suit me?','You told me you prefer amber wicks.','amber'),('correction','player_1',[2,3],'Please greet me by name.','Hello, Vela.','vela'),('promise','player_1',[4],'What should I take care of first?','Your promise to return the borrowed lamp is still open.','lamp'),('other_player','player_2',[5],'Which wick matches my taste?','You told me you prefer blue wicks.','blue')]
def build(holdout=False,safe=True,cli=False,optional_preferences=False):
 def rewrite(s):
  if not holdout:return s
  for old,new in [('amber','copper'),('blue','ivory'),('Vela','Lina'),('Vira','Lira'),('borrowed lamp','borrowed measuring cord')]:s=s.replace(old,new)
  return s
 specifications=[(f,p,r,rewrite(q),rewrite(a),'cord' if holdout and needle=='lamp' else 'lina' if holdout and needle=='vela' else rewrite(needle)) for f,p,r,q,a,needle in SPECS]
 spec=importlib.util.spec_from_file_location('bridge',ROOT/'bindings/python/gamectx_client.py');b=importlib.util.module_from_spec(spec);spec.loader.exec_module(b)
 pack=json.loads((ROOT/'examples/cast/chime-town.json').read_text());people=pack['players']+[c['id'] for c in pack['characters']];packets=[];restores=0;checks=0
 library=ROOT/'bindings/native/target/debug/libgamectx_native.so'
 transport=b.Client(None,ROOT/'target/debug/gamectx') if cli or not library.is_file() else b.NativeClient(None,library)
 with transport as client:
  for ch in pack['characters']:
   actor=ch['id'];identity=dict(campaign='night_campaign',branch='chapter_a',instance=actor)
   def fresh(index):
    n=b.Character(client,'campaign_'+actor+'_'+str(index),'campaign_save',actor,people);n.life_configure(pack['setting'],ch['definition'],dict(enabled=True),{});return n
   n=fresh(0);v=view();proof=dict(id='open_promise',speaker='player_1',text=rewrite('I will return the borrowed lamp.'));v['sources']=[proof];n.bind(v);n.enable(['promise_remember']);note=n.call('promise_remember',dict(promisor='player_1',beneficiary=actor,task=rewrite('Return the borrowed lamp.'),source='open_promise'),7);n.progress(dict(id=note['id'],expected=0,state='accepted',source=proof))
   for turn in range(1,1001):
    early={1:('player_1','preference','player_1 said: I prefer amber wicks.',[]),2:('player_1','name','player_1 said: My name is Vira.',[]),3:('player_1','correction','player_1 corrected the name: Vela, not Vira.',[2]),4:('player_1','promise','player_1 promised to return the borrowed lamp; still unfulfilled.',[]),5:('player_2','preference','player_2 said: I prefer blue wicks.',[])}
    subject,tag,text,causes=early.get(turn,('player_1' if turn%2 else 'player_2','chatter','Routine talk in the '+['counter','stair','tavern'][(turn//100)%3]+'.',[]))
    n.life_experience(dict(token=turn,at=turn,kind='correction' if turn==3 else 'direct',tag=tag,subject=subject,source=dict(id='campaign_receipt_'+str(turn),speaker=subject,text=rewrite(text)),audience=people,text=rewrite(text),causes=causes,retain=turn<=5))
    if turn%100==0:
     n.life('consolidate',start=turn-99,end=turn);n.life('ack',through=turn);n.life('evict',through=turn);saved=n.life('save',identity=identity);other=fresh(turn);other.life('restore',identity=identity,state=saved,fence=dict(min_revision=0,max_revision=saved['life']['state']['revision'],min_token=0,max_token=turn,max_tick=turn),min_ticket=0,min_proposal=0,min_receipt=0,min_note=0,max_tick=turn);n.close();n=other;restores+=1
     state=n.life('save',identity=identity);assert len(state['interactions']['progress'])==1 and state['interactions']['progress'][0]['state']=='accepted';checks+=1
   if actor=='guard':n.close();continue
   for family,player,required,question,answer,needle in specifications:
    selection=dict(input=question,tags=[],subjects=[player],required=[] if optional_preferences and family in ['preference','other_player'] else required,count=2 if optional_preferences and family in ['preference','other_player'] else len(required),archive=True);selected=n.life_recall_selected(selection);assert [e['token'] for e in selected]==required and all(e['subject']==player for e in selected)
    saved=n.life('save',identity=identity);notes=[note for note in saved['interactions']['notes']['notes'] if note['subject']==player];progress=saved['interactions']['progress'];assert len(notes)==(1 if player=='player_1' else 0)
    args=dict(version=2,rules='Reply as '+actor+' to the addressing player, in one short sentence. Quoted reports and claims remain attributed. Current correction overrides the old name. Only choose an authorized reply; return speech JSON.',tags=['trade'],caps=dict(facts=4,facets=2,attitudes=2,episodes=2,exchanges=1),stable_scene=dict(id='tavern',description='A host-confirmed meeting after the counter and stair chapters.'),state=dict(episodes=selected,accepted_notes=notes,progress=progress if notes else []),dialogue=[],input=question,tools=[],turn=dict(profile='focused',knowledge='known',creative=False,speaker=player,question_subject=player,facts=[]))
    request=n.life('compile_speech',request=args);request.pop('tools')
    # Explicit host attention selection uses validated current Life attitudes, not model claims.
    changing=json.loads(request['messages'][2]['content']);changing['character']['attitudes']=[a for a in n.life('inspect')['life']['attitudes'] if a['definition']['subject']==player][:2];request['messages'][2]['content']=enc(changing)
    scene=dict(version=1,id='campaign_save',epoch=1,revision=1000,participants=[dict(id=p,kind='player' if p.startswith('player_') else 'ai') for p in people]);other_actor='keeper' if actor!='keeper' else 'merchant'
    receipts=[dict(token=1,speaker=other_actor,recipients=['player_2'],heard=[other_actor,'player_2',actor],text=rewrite('player_2 told me they prefer blue wicks.'),parent=None),dict(token=2,speaker=actor,recipients=['player_1'],heard=[actor,'player_1'],text='I will use what you told me.',parent=None),dict(token=3,speaker='guard',recipients=['player_2'],heard=['guard','player_2'],text='GUARD_ONLY_TEXT',parent=None)]
    rendered=local('party',dict(scene=scene,ledger=dict(limit=0,window=1),operations=[dict(op='receipt',args=r) for r in receipts]+[dict(op='render',args=dict(actor=actor,limit=2))]))['results'][-1]['result'];assert len(rendered)==2 and [m['role'] for m in rendered]==['user','assistant'] and 'GUARD_ONLY_TEXT' not in enc(rendered)
    request['messages'][3:3]=rendered
    inspection=None
    if safe:
     cp=subprocess.run([str(ROOT/'target/overnight-campaign-compile')],input=enc(dict(pack=pack,actor=actor,player=player,saved=saved,identity=identity,scene=scene,receipts=receipts,speech=args,selection=selection)),capture_output=True,text=True);assert cp.returncode==0,cp.stderr
     compiled=json.loads(cp.stdout);request=compiled['request'];request.pop('tools');inspection=compiled['inspection'];assert compiled['selected']==selected
     from archetype_reference import gate_select
     actual=n.life('inspect')['life'];levels=[dict(kind=a['definition']['kind'],subject=a['definition']['subject'],level=a['level']) for a in actual['attitudes']]
     expected=gate_select(dict(rules=ch['gates'],actor=actor,speaker=player,listeners=[player],participants=people,revision=actual['revision'],expected_revision=actual['revision'],attitudes=levels,kind='facet',items=[f for f in ch['definition']['facets'] if set(people)<=set(f['audience']) and 'trade' in f['tags']]))
     assert json.loads(request['messages'][1]['content'])['character']['facets']==expected['packet'][:2]
     assert 'copper sparrow' not in enc(request) and 'silver chime opens' not in enc(request) and 'GUARD_ONLY_TEXT' not in enc(request)
     assert request['messages'][3:5]==rendered
     assert all(a['definition']['subject']==player for a in json.loads(request['messages'][2]['content'])['character']['attitudes'])
    answers=dict(version=1,scope='campaign_save',revision=1000,unknown="I don't know.",cards=[dict(id='selected',subject=player,topic=family,kind='state',source='archived_campaign',text=answer)])
    schema=local('source-speech',dict(answers=answers,calls=[]))['schema'];request['messages'].insert(-1,dict(role='system',content='Select the authorized sentence verbatim: '+answer))
    packets.append(dict(inspection=inspection,actor=actor,player=player,family=family,required_tokens=required,selected=selected,promise_notes=notes,needle=needle,answers=answers,request=request|dict(model=MODEL,format=schema,keep_alive='10m',options=dict(num_ctx=2048,num_thread=4,num_gpu=0,temperature=0,seed=2041,num_predict=96))))
   n.close()
 return dict(version=1,gate_aware=safe,holdout=holdout,events=4000,chapters_per_character=10,restores=restores,accepted_promise_checks=checks,guard_model_calls=0,packets=packets,actual_provider_calls=0,api_spend_usd=0,limits='Original deterministic campaign; fixed synthetic chapter noise, not 1000 generated exchanges. Host explicitly enables guard Life for storage tests; its ambient voice remains zero-model. Host filters current validated attitudes by player. Hearing/delivery are simulated host confirmations, not physical audio. This combines previously tested memory facts, so it is integration/regression evidence, not unseen semantic accuracy.')
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--build',action='store_true');p.add_argument('--plan',type=pathlib.Path);p.add_argument('--tokens-only',action='store_true');p.add_argument('--free',action='store_true');p.add_argument('--required',action='store_true');p.add_argument('--holdout',action='store_true');p.add_argument('--safe',action='store_true');p.add_argument('--legacy-renderer',action='store_true');p.add_argument('--cli',action='store_true');p.add_argument('--optional-preferences',action='store_true');p.add_argument('--tokens',type=pathlib.Path);p.add_argument('--output',required=True,type=pathlib.Path);a=p.parse_args();assert a.cap==0 and not a.output.exists()
 if a.build:
  r=build(a.holdout,not a.legacy_renderer,a.cli,a.optional_preferences);oe.checkpoint(a.output,r);print(enc(dict(events=r['events'],restores=r['restores'],accepted_promise_checks=r['accepted_promise_checks'],packets=len(r['packets']),api_spend_usd=0)));return
 plan=json.loads(a.plan.read_text());assert plan.get('gate_aware',False) or a.legacy_renderer,'Legacy ungated plans require explicit --legacy-renderer';assert plan['restores']==40 and plan['accepted_promise_checks']==40 and len(plan['packets'])==12
 server=oe.Server('http://127.0.0.1:11434',240);m=next(x for x in server.data('/api/tags')['models'] if x['name']==MODEL);oe.require_local_weights(m,server.data('/api/show',dict(model=MODEL)));identity=dict(model=MODEL,digest=m['digest'])
 if not a.tokens_only:
  prior=json.loads(a.tokens.read_text());assert prior['complete'] and prior['identity']==identity and prior['plan_sha256']==hashlib.sha256(a.plan.read_bytes()).hexdigest()
 report=dict(version=1,mode='free' if a.free else 'required' if a.required else 'source_bound',identity=identity,plan_sha256=hashlib.sha256(a.plan.read_bytes()).hexdigest(),tokens_only=a.tokens_only,observations=[],actual_provider_calls=0,api_spend_usd=0,complete=False,limits=plan['limits']);history={};oe.checkpoint(a.output,report)
 for packet in plan['packets']:
  q=copy.deepcopy(packet['request']);key=(packet['actor'],packet['player'])
  if not a.tokens_only and key in history:
   old=history[key];q['messages'][3:5]=[dict(role='user',content='['+packet['player']+' -> '+packet['actor']+'] '+old['input']),dict(role='assistant',content='['+packet['actor']+' -> '+packet['player']+'] '+old['speech'])]
  if a.required:q['format']=local('required-speech',dict(answers=packet['answers'],calls=[]))['schema']
  if a.free:
   q['format']=dict(type='object',properties=dict(speech=dict(type='string',maxLength=240)),required=['speech'],additionalProperties=False)
   q['messages'][0]['content']=q['messages'][0]['content'].replace('Only choose an authorized reply; return speech JSON.','Use only the supplied current evidence, not invented history or effects; return speech JSON.')
   q['messages']=[m for m in q['messages'] if not m['content'].startswith('Select the authorized sentence verbatim: ')]
  row=dict(actor=packet['actor'],player=packet['player'],family=packet['family'],calls=[]);report['observations'].append(row)
  for phase,predict in [('preflight',1)]+([] if a.tokens_only else [('generation',96)]):
   payload=copy.deepcopy(q);payload['options']['num_predict']=predict;call=dict(phase=phase,payload=payload,status='reserved');row['calls'].append(call);report['actual_provider_calls']+=1;oe.checkpoint(a.output,report)
   response=server.chat(payload);call.update(status='complete',response=response);oe.checkpoint(a.output,report);assert response['final']['prompt_eval_count']+96+256<=2048
  if not a.tokens_only:
   raw=response['text'];a1=packet['answers'];result=local('required-speech' if a.required else 'source-speech',dict(answers=a1,calls=[dict(text=raw,context=dict(scope='campaign_save',revision=1000),available=[dict(kind='state',id='archived_campaign')])]))['results'][0];row['authorized']=result['ok'];row['required']=packet['needle'] in json.loads(raw)['speech'].lower();history[key]=dict(input=packet['request']['messages'][-1]['content'],speech=json.loads(raw)['speech'])
  oe.checkpoint(a.output,report);print(packet['actor'],packet['player'],packet['family'],'$0 API',flush=True)
 report['complete']=True;oe.checkpoint(a.output,report)
if __name__=='__main__':main()
