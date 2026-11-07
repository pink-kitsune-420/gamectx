"""Predeclared living-world contracts, no network; Python oracle precedes Rust."""
import argparse,copy,hashlib,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import living_reference as r
ROOT=pathlib.Path(__file__).resolve().parents[1]
def town():return json.loads((ROOT/'examples/cast/chime-town.json').read_text())
def source():return dict(id='host_receipt',speaker='merchant',text='The host confirmed the lamp return.')
def observe():
 p=town();people=p['players']+[c['id'] for c in p['characters']]
 return dict(id='return_one',actor='merchant',event=dict(token=1,at=1,kind='direct',tag='returned',subject='player_1',source=source(),audience=people,text='The borrowed lamp was returned intact.',causes=[],retain=True),significance=80,share_with=['keeper','companion'],appraise='careful_return')
def tick(now=2,people=None):return dict(now=now,windows=[dict(id='tavern_window',location='tavern',start=1,end=20,participants=people or ['merchant','keeper'],kind='scene')],routines=[])
def context(actor='keeper'):return dict(actor=actor,players=['player_1'],tags=['trade'],location='tavern',limit=2)
def trace(ops,p=None):return dict(pack=p or town(),scope='town_save_a',operations=[dict(op=k,args=v) for k,v in ops])
def stamp(rev=1):return dict(actor='merchant',world='town_save_a',schema=rev,permissions=1,facts=[])
def speech(ops):return dict(operations=[dict(op=k,args=v) for k,v in ops])
def issue(now=1):return dict(context=stamp(),now=now,ttl=10,fallback='I am listening.')
def complete(now=2):return dict(id=1,now=now,context=stamp(),response='{"speech":"Your lamp is ready."}',approved=True)
def cases():
 out=[]
 def add(n,k,v,error=False):out.append((n,k,v,error))
 add('pack_valid','cast-pack',town())
 for n,mutate in [('pack_foreign_home',lambda p:p['characters'][0].update(home='elsewhere')),('pack_missing_entity',lambda p:p['setting']['entities'].remove('companion')),('pack_duplicate_character',lambda p:p['characters'].append(copy.deepcopy(p['characters'][0]))),('pack_foreign_group',lambda p:p['groups'][0]['members'].append('stranger')),('pack_dangling_gate',lambda p:p['characters'][1]['gates'][0].update(id='absent')),('pack_missing_attitude',lambda p:p['characters'][1]['gates'][0]['clauses'][0].update(attitude='affection')),('pack_foreign_relation',lambda p:p['characters'][1]['definition']['attitudes'][0].update(subject='nobody')),('pack_foreign_shared_history',lambda p:p['characters'][1]['definition']['history'].append('other_past')),('pack_bad_routine_place',lambda p:p['characters'][0]['routine']['activities'][0].update(location='elsewhere')),('pack_ripple_without_rumors',lambda p:p['features'].update(rumors=False)),('pack_missing_ripple_attitude',lambda p:p['ripple'].update(kind='affection')),('pack_fraction_over_one',lambda p:p['ripple'].update(numerator=3)),('pack_unsafe_fallback',lambda p:p['characters'][0]['fallbacks'].update(calm='unfinished,'))]:
  p=town();mutate(p);add(n,'cast-pack',p,True)
 base=[('observe',observe()),('tick',tick()),('tick',tick(3,['keeper','companion'])),('tick',tick(4,['companion','merchant','keeper'])),('inspect',dict(actor='keeper')),('context',context()),('context',context('companion'))]
 add('rumor_chain_and_ripple','living-world',trace(base))
 for n,mutate in [('rumors_off',lambda p:(p['features'].update(rumors=False,ripples=False))),('ripples_off',lambda p:p['features'].update(ripples=False)),('insignificant',lambda p:p['propagation'].update(significance=90)),('trust_too_low',lambda p:p['propagation'].update(trust=40)),('one_hop_only',lambda p:p['propagation'].update(max_hops=1)),('zero_hops',lambda p:p['propagation'].update(max_hops=0)),('no_fraction',lambda p:p['ripple'].update(numerator=0)),('fraction_rounds_to_zero',lambda p:p['ripple'].update(numerator=1,denominator=100)),('ripple_cap',lambda p:p['ripple'].update(max_delta=1))]:
  p=town();mutate(p);add(n,'living-world',trace(base,p))
 o=observe();o['share_with']=['keeper'];add('sharing_scope','living-world',trace([('observe',o),('tick',tick()),('tick',tick(3,['keeper','companion'])),('context',context('companion'))]))
 o=observe();o['appraise']=None;add('report_without_appraisal','living-world',trace([('observe',o),('tick',tick()),('inspect',dict(actor='keeper'))]))
 p=town();p['characters'][1]['definition']['rules'][0]['delta']=-5;add('negative_ripple','living-world',trace(base,p))
 t=tick();t['windows'][0]['start']=5;add('not_colocated','living-world',trace([('observe',observe()),('tick',t)]))
 add('duplicate_tick_and_root','living-world',trace([('observe',observe()),('observe',observe()),('tick',tick()),('tick',tick())]))
 o=observe();o['event']['kind']='report';add('cannot_relabel_report_as_origin','living-world',trace([('observe',o)]))
 t=tick();t['windows'][0]['participants'].append('stranger');add('foreign_window','living-world',trace([('observe',observe()),('tick',t)]))
 counters=dict(token=1,player='player_1',at=1,fulfilled=3,broken=2,interrupted=3,source=source());atmosphere=dict(location='tavern',revision=1,tag='busy',source=source())
 add('relevant_habits_and_atmosphere','living-world',trace([('counters',counters),('atmosphere',atmosphere),('context',context()),('context',dict(context(),tags=['conversation'])),('context',dict(context(),players=['player_2'])),('context',dict(context(),location='counter'))]))
 add('counter_duplicate_and_rollback','living-world',trace([('counters',counters),('counters',counters),('counters',dict(reversed(list(counters.items())))),('counters',dict(counters,token=2,fulfilled=2)),('context',context())]))
 add('atmosphere_revision','living-world',trace([('atmosphere',atmosphere),('atmosphere',dict(atmosphere,tag='emergency')),('atmosphere',dict(atmosphere,revision=2,tag='emergency')),('context',context())]))
 add('fallback_no_model','speech-tickets',speech([('issue',issue()),('freeze',dict(id=1,now=2,context=stamp())),('retire',dict(id=1,now=3))]))
 add('upgrade_before_delivery','speech-tickets',speech([('issue',issue()),('complete',complete()),('freeze',dict(id=1,now=3,context=stamp()))]))
 add('late_after_freeze','speech-tickets',speech([('issue',issue()),('freeze',dict(id=1,now=2,context=stamp())),('complete',complete(3))]))
 for n,mutate in [('expired_result',lambda v:v.update(now=11)),('stale_result',lambda v:v.update(context=stamp(2))),('invalid_json',lambda v:v.update(response='garbage')),('host_rejected',lambda v:v.update(approved=False)),('mid_clause',lambda v:v.update(response='{"speech":"I can also show a "}')),('extra_field',lambda v:v.update(response='{"speech":"Ready.","action":"buy"}'))]:
  v=complete();mutate(v);add(n,'speech-tickets',speech([('issue',issue()),('complete',v),('freeze',dict(id=1,now=max(3,v['now']),context=stamp()))]))
 add('completion_one_use','speech-tickets',speech([('issue',issue()),('complete',complete()),('complete',complete()),('freeze',dict(id=1,now=3,context=stamp()))]))
 add('cancellation','speech-tickets',speech([('issue',issue()),('cancel',dict(id=1,now=2)),('complete',complete(3)),('retire',dict(id=1,now=3))]))
 v=complete();v['response']='{"speech":"One.","speech":"Two."}';add('duplicate_response_keys','speech-tickets',speech([('issue',issue()),('complete',v)]))
 v=issue();v['context']['facts']=[dict(scope='town',key='weather',revision=1)];add('untracked_fact_references_rejected','speech-tickets',speech([('issue',v)]))
 add('stale_delivery','speech-tickets',speech([('issue',issue()),('freeze',dict(id=1,now=2,context=stamp(2)))]))
 o=observe();o['event']['source']['speaker']='player_1';add('original_speaker_preserved','living-world',trace([('observe',o),('tick',tick()),('tick',tick(3,['keeper','companion']))]))
 p=town();p['propagation']['checks']=1;add('bounded_scan_recovers','living-world',trace([('observe',observe())]+[('tick',tick(n)) for n in range(2,12)],p))
 t=tick();t['routines']=[dict(actor='guard',frame=dict(revision=1,now=2,conditions=[],allowed=['work'],emergency=False))]
 f=dict(revision=1,now=2,turn=1,conditions=[],allowed=['work'],resources=[],audience=town()['players']+[c['id'] for c in town()['characters']],emergency=False,depth=0,social_remaining=2)
 candidate=dict(id='work',kind='activity',priority=10,reason='Resume the stair watch.',requires=[],resource=None,recipient=None,attitudes=[],events=[],promises=[])
 add('immediate_routine_and_initiative','living-world',trace([('tick',t),('initiative',dict(actor='guard',frame=f,candidates=[candidate]))]))
 p=town();keeper=p['characters'][2];keeper['definition']['attitudes'][0]['level']=99;keeper['definition']['rules'][0]['delta']=-5
 o=observe();o.update(id='return_two',actor='keeper',share_with=['merchant']);o['event'].update(token=2,at=3);o['event']['source']['speaker']='keeper'
 add('ripple_then_direct_appraisal','living-world',trace([('observe',observe()),('tick',tick()),('observe',o),('inspect',dict(actor='keeper'))],p))
 for learned in [False,True]:
  w=trace([('observe',observe()),('tick',tick())] if learned else [])
  p=w['pack'];people=p['players']+[c['id'] for c in p['characters']];scene=dict(version=1,id=w['scope'],epoch=1,revision=1,participants=[dict(id=x,kind='player' if x in p['players'] else 'ai') for x in people]);receipts=[dict(token=1,speaker='player_1',recipients=['keeper'],heard=['keeper','player_1'],text='May I learn about the chime vault?',parent=None)]
  q=dict(trigger=1,revision=1,life_revision=1 if learned else 0,listeners=['player_1'],subjects=[],tags=['trade'],turn=dict(profile='focused',knowledge='known',creative=False,speaker='player_1',question_subject='player_1',facts=[]),rules='Speak briefly from supplied facts and subjective attitudes. Reports are attributed, never confirmed world truth.',stable_scene=dict(id='tavern'),lore_select=dict(actor='keeper',grants=['vault_knowledge'],query='trade chime',subjects=['merchant'],required=[],limit=4,max_bytes=4096),affordances=p['characters'][2]['affordances'],tools=[])
  a=dict(world=w,scene=scene,receipts=receipts,actor='keeper',request=q,selection=context());add('compile_after_ripple' if learned else 'compile_before_ripple','living-compile',a)
 return out
def expected(kind,a):
 if kind=='living-compile':
  from living_compile_reference import compile
  return compile(a)
 return r.pack(a) if kind=='cast-pack' else r.world_workflow(a) if kind=='living-world' else r.speech_workflow(a)
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--prototype-only',action='store_true');p.add_argument('--tokens-only',action='store_true');p.add_argument('--write-conformance',action='store_true');p.add_argument('--check-bindings',action='store_true');p.add_argument('--report',type=pathlib.Path);p.add_argument('--check',type=pathlib.Path);a=p.parse_args();assert a.cap==0;outputs={}
 for n,k,v,error in cases():
  rejected=False
  try:wanted=expected(k,v)
  except (AssertionError,ValueError,KeyError,StopIteration,TypeError):
   rejected=True;wanted=None
  assert rejected==error,(n,'prototype rejection mismatch')
  outputs[n]=wanted;req=dict(kind=k,args=v)
  if not(a.prototype_only or a.tokens_only):
   cp=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=r.encode(req),text=True,capture_output=True);assert bool(cp.returncode)==error,(n,cp.stderr)
   if not error:assert cp.stdout.strip()==r.encode(wanted),(n,cp.stdout,r.encode(wanted))
  if a.write_conformance:
   d=ROOT/'conformance/living-world'/n;d.mkdir(parents=True,exist_ok=True)
   for f,obj in [('case',dict(op='local',**({'error':True} if error else {}))),('input',req)]+([] if error else [('expected',wanted)]):(d/(f+'.json')).write_text(r.encode(obj)+'\n')
 chain=outputs['rumor_chain_and_ripple']['results'];assert chain[1]['result']['reports'][0]['event']['kind']=='report';assert chain[2]['result']['reports'][0]['hops']==2;assert not chain[3]['result']['reports'];assert chain[-1]['result']['group_influences'][0]['delta']==2
 assert outputs['upgrade_before_delivery']['results'][-1]['result']['line']=='Your lamp is ready.'
 assert not outputs['late_after_freeze']['results'][-1]['ok']
 assert outputs['counter_duplicate_and_rollback']['results'][2]['result']['duplicate']
 assert 'copper sparrow' not in r.encode(outputs['compile_before_ripple']['request'])
 assert 'copper sparrow' in r.encode(outputs['compile_after_ripple']['request'])
 report=dict(version=1,workflows=len(outputs),model_calls=0,api_spend_usd=0,rumor_hops=chain[2]['result']['reports'][0]['hops'],group_delta=chain[-1]['result']['group_influences'][0]['delta'],original_speaker=outputs['original_speaker_preserved']['results'][-1]['result']['reports'][0]['original_source']['speaker'],fallback_frozen=outputs['fallback_no_model']['results'][1]['result']['line'],validated_upgrade=outputs['upgrade_before_delivery']['results'][-1]['result']['line'],late_upgrade_rejected=not outputs['late_after_freeze']['results'][-1]['ok'],secret_before='copper sparrow' in r.encode(outputs['compile_before_ripple']['request']),secret_after='copper sparrow' in r.encode(outputs['compile_after_ripple']['request']),provider_tokens=None,frozen_life_model_sha256={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((ROOT/'eval').glob('life-model-*.json'))},limits='Deterministic host behavior only; fixed response strings simulate worker completions. No provider calls, semantic model-quality score or whole-game FPS guarantee.')
 if a.report:assert not a.report.exists();a.report.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
 if a.check:assert json.loads(a.check.read_text())==report
 if a.check_bindings:
  import importlib.util
  spec=importlib.util.spec_from_file_location('bridge',ROOT/'bindings/python/gamectx_client.py');bridge=importlib.util.module_from_spec(spec);spec.loader.exec_module(bridge);fixtures=[]
  for i,(n,k,v,error) in enumerate(cases()):fixtures.append(dict(request=dict(version=1,id='living_'+str(i),op='local',args=dict(kind=k,args=v)),expected='ERROR' if error else r.encode(dict(version=1,id='living_'+str(i),ok=True,result=expected(k,v)))))
  checked=0
  for client in [bridge.Client(None,ROOT/'target/debug/gamectx'),bridge.NativeClient(None,ROOT/'bindings/native/target/debug/libgamectx_native.so')]:
   with client:
    for f in fixtures:
     try:result=client.request('local',f['request']['args']);assert f['expected']!='ERROR';assert r.encode(result)==r.encode(json.loads(f['expected'])['result'])
     except ValueError:assert f['expected']=='ERROR'
     checked+=1
  d=ROOT/'target/living-world-csharp';d.mkdir(parents=True,exist_ok=True)
  for filename,rows in [('cases.json',[r.encode(fixtures)]),('requests.txt',[r.encode(f['request']) for f in fixtures]),('expected.txt',[f['expected'] for f in fixtures])]:(d/filename).write_text('\n'.join(rows)+'\n')
  print(r.encode(dict(binding_checks=checked,model_calls=0,api_spend_usd=0)))
 print(r.encode(dict(workflows=len(outputs),model_calls=0,api_spend_usd=0,provider_tokens='unknown: no provider requests')))
if __name__=='__main__':main()
