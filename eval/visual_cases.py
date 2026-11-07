"""Predeclared original visual compiler and approval/reuse/repair expectations."""
import argparse,copy,hashlib,importlib.util,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import visual_reference as r
from life_cases import base as life_base
ROOT=pathlib.Path(__file__).resolve().parents[1]
def base():
 l=life_base();l['operations']=[];l['scope']='counter';l['defaults']=dict(enabled=True)
 b=json.loads((ROOT/'examples/visual/chime-art.json').read_text())
 scene=dict(id='counter',revision=1,epoch=1,at=10,participants=l['participants'],facts=[],affordances=[])
 return dict(bible=b,life=l,activity=None,frame=dict(scope='counter',observer='player',entity='merchant',listener='player',scene=scene,audience=['player'],life_revision=0,activity_revision=None))
def fact(id,kind='perceived',**extra):return dict(id=id,revision=1,observed_at=10,source=dict(id='sense_'+id,speaker='player',text='The host confirms this perception.'),kind=kind,audience=['merchant','player'],text='Raw world text is never an image prompt.')|extra
def neutral(a):a['life']['definition']['attitudes'][0]['level']=0;return a
def patch():return dict(replace_tokens=[],add_tokens=[],negative_tokens=[],forbid_props=[],seed_delta=0,asset=None)
def trace(specs,ops,**cfg):return dict(config=dict(enabled=True,capacity=16,queue_limit=2,epoch='preview_run')|cfg,specs=specs,operations=[dict(op=k,args=v) for k,v in ops])
def req(i=0):return ('request',dict(spec=i))
def approve(key=0):return ('approve',dict(key=key,asset_uri='asset://chime/neutral.png',digest='a'*64))
def cases():
 out=[]
 def add(n,k,v,error=False):out.append((n,k,v,error))
 a=base();add('guarded_portrait','visual-spec',a)
 a=neutral(base());add('neutral_portrait','visual-spec',a)
 a=base();a['life']['definition']['attitudes'][0]['level']=15;add('warm_portrait','visual-spec',a)
 a=base();a['frame']['scene']['facts']=[fact('visible_sun','global'),fact('shutters_closed')];add('closed_shutters_no_global_sun','visual-spec',a)
 a=base();a['frame']['scene']['facts']=[fact('visible_sun'),fact('shutters_closed')];add('occlusion_wins','visual-spec',a)
 a=base();a['frame']['scene']['facts']=[fact('visible_sun')];add('perceived_sun','visual-spec',a)
 a=base();a['frame']['scene']['facts']=[fact('visible_lamp')];add('perceived_prop','visual-spec',a)
 for n,f in [('report_omitted',fact('visible_sun','report')),('mutual_omitted',fact('visible_sun','mutual')),('aged_omitted',fact('visible_sun',observed_at=7)),('other_observer_omitted',fact('visible_sun',source=dict(id='sense',speaker='merchant',text='Another NPC saw it.'))),('private_omitted',fact('visible_sun',audience=['merchant'])),('unmapped_omitted',fact('unlisted'))]:
  a=base();a['frame']['scene']['facts']=[f];add(n,'visual-spec',a)
 a=base();a['activity']=dict(scope='counter',actor='merchant',revision=1,source='routine',state='active',id='work');a['frame']['activity_revision']=1;add('working_pose','visual-spec',a)
 a=copy.deepcopy(a);a['activity']['source']='initiative';a['activity']['id']='walk';add('started_initiative_pose','visual-spec',a)
 a=base();a['bible']['format']='scene';a['frame']['entity']='counter';a['life']=None;a['frame']['life_revision']=None;a['frame']['scene']['facts']=[fact('shutters_closed')];add('scene_spec','visual-spec',a)
 a=base();a['bible']['format']='sprite';a['bible']['width']=128;a['bible']['height']=128;add('sprite_spec','visual-spec',a)
 for n,mutate in [('bad_aspect',lambda a:a['bible'].update(width=513)),('unknown_bible_field',lambda a:a['bible'].update(provider_key='forbidden')),('bad_dimensions',lambda a:a['bible'].update(width=0)),('duplicate_anchor',lambda a:a['bible']['anchors'].append(copy.deepcopy(a['bible']['anchors'][0]))),('bad_rule_range',lambda a:a['bible']['expressions'][0].update(min=10,max=0)),('stale_life',lambda a:a['frame'].update(life_revision=1)),('foreign_life',lambda a:a['frame'].update(entity='counter')),('foreign_audience',lambda a:a['frame'].update(audience=['stranger'])),('future_observation',lambda a:a['frame']['scene']['facts'].append(fact('visible_sun',observed_at=11))),('proposal_not_active',lambda a:(a.update(activity=dict(scope='counter',actor='merchant',revision=1,source='initiative',state='proposed',id='walk')),a['frame'].update(activity_revision=1)))]:
  a=base();mutate(a);add(n,'visual-spec',a,True)
 a=base();b=copy.deepcopy(a);b['life']['definition']['attitudes'][0]['level']=-5
 add('same_slot_reuses_key','visual-assets',trace([a,b],[req(),approve(),req(1)]))
 n=neutral(base());warm=base();warm['life']['definition']['attitudes'][0]['level']=20
 add('neutral_fallback_then_exact','visual-assets',trace([n,warm],[req(),approve(),req(1),('submit',dict(key=1,ticket=1,asset_uri='asset://chime/warm.png',digest='b'*64)),('approve',dict(key=1,asset_uri='asset://chime/warm.png',digest='b'*64)),req(1)]))
 add('candidate_never_default_art','visual-assets',trace([n],[req(),('submit',dict(key=0,ticket=0,asset_uri='asset://unapproved.png',digest='b'*64)),req()]))
 add('duplicate_candidate_no_requeue','visual-assets',trace([n],[req(),req(),req()]))
 add('disabled_no_jobs','visual-assets',trace([n],[req()],enabled=False))
 add('queue_limit','visual-assets',trace([n,warm],[req(),req(1)],queue_limit=1))
 add('capacity_fallback','visual-assets',trace([n,warm],[req(),approve(),req(1)],capacity=1))
 add('rejected_no_auto_retry','visual-assets',trace([n],[req(),('reject',dict(key=0,reason='Unwanted prop.')),req(),('submit',dict(key=0,ticket=0,asset_uri='asset://late.png',digest='b'*64))]))
 p=patch();p['seed_delta']=1;add('repair_invalidates_old_job','visual-assets',trace([n],[req(),('repair',dict(key=0,patch=p)),('submit',dict(key=0,ticket=0,asset_uri='asset://stale.png',digest='b'*64)),req(),('inspect',dict(key=1))]))
 p=patch();p['asset']=dict(asset_uri='asset://edited.png',digest='c'*64);add('hand_edit_pinned','visual-assets',trace([n],[req(),('repair',dict(key=0,patch=p)),req(),('approve',dict(key=0,asset_uri='asset://late.png',digest='a'*64))]))
 a=base();a['frame']['scene']['facts']=[fact('visible_lamp')];p=patch();p['forbid_props']=['lamp'];add('repair_forbids_prop_and_cue','visual-assets',trace([a],[req(),('repair',dict(key=0,patch=p)),req(),('inspect',dict(key=1))]))
 a=neutral(base());a['frame']['scene']['facts']=[fact('visible_lamp')];next(x for x in a['bible']['facts'] if x['id']=='visible_lamp')['tokens']=['a work counter','repaired lamp visible on the counter'];p=patch();p['negative_tokens']=['wide-brimmed hat'];p['add_tokens']=['uncovered head'];p['replace_tokens']=[{'from':'repaired lamp visible on the counter','to':'a shaded lamp'}];add('repair_tokens','visual-assets',trace([a],[req(),approve(),('repair',dict(key=0,patch=p)),req(),('inspect',dict(key=1))]))
 p=patch();p['replace_tokens']=[dict(**{'from':n['bible']['style'],'to':'Another style'})];add('locked_style_not_patchable','visual-assets',trace([n],[req(),('repair',dict(key=0,patch=p)),req()]))
 p=patch();p['seed_delta']=1;add('repair_capacity_atomic','visual-assets',trace([n],[req(),('repair',dict(key=0,patch=p)),('submit',dict(key=0,ticket=0,asset_uri='asset://still_current.png',digest='a'*64))],capacity=1))
 a=base();b=copy.deepcopy(a);a['frame']['scene']['facts']=[fact('visible_sun')];b['frame']['scene']['facts']=[fact('shutters_closed')];add('sunny_fallback_not_visible_after_shutters','visual-assets',trace([a,b],[req(),approve(),req(1)]))
 a=neutral(base());b=copy.deepcopy(a);b['frame']['scene']['epoch']=2;add('epoch_isolation','visual-assets',trace([a,b],[req(),approve(),req(1)]))
 a=neutral(base());b=copy.deepcopy(a);b['bible']['palette'].append('dark red');add('bible_content_not_just_revision','visual-assets',trace([a,b],[req(),approve(),req(1)]))
 a=neutral(base());b=copy.deepcopy(a);b['bible']['palette'].reverse();b['bible']['anchors'].reverse();b['bible']['expressions'].reverse();add('normalized_order_reuse','visual-assets',trace([a,b],[req(),approve(),req(1)]))
 a=base();b=copy.deepcopy(a);a['frame']['scene']['facts']=[fact('shutters_closed'),fact('visible_lamp')];b['frame']['scene']['facts']=list(reversed(a['frame']['scene']['facts']));add('fact_order_stable','visual-assets',trace([a,b],[req(),approve(),req(1)]))
 a=base();a['frame']['scene']['facts']=[fact('visible_lamp',revision=2)];add('future_fact_revision','visual-spec',a,True)
 a=base();next(x for x in a['bible']['anchors'] if x['entity']=='merchant')['props']=[];a['frame']['scene']['facts']=[fact('visible_lamp')];add('foreign_prop_withheld','visual-spec',a)
 a=base();a['bible']['expressions'][0]['subject']='self';a['bible']['expressions'][0]['kind']='emotion';a['bible']['expressions'][0]['min']=0;a['bible']['expressions'][0]['max']=100;add('missing_emotion_neutral','visual-spec',a)
 a=base();a['bible']['style']='Original copper ink — soft edges';a['bible']['palette']=['銅色','deep blue'];add('unicode_digest','visual-spec',a)
 p=patch();p['seed_delta']=1;q=patch();q['seed_delta']=2**32-1;add('repair_cycle_stale_ticket','visual-assets',trace([n],[req(),('repair',dict(key=0,patch=p)),req(),('repair',dict(key=1,patch=q)),req(),('submit',dict(key=0,ticket=0,asset_uri='asset://stale.png',digest='a'*64)),('submit',dict(key=0,ticket=4,asset_uri='asset://current.png',digest='a'*64))]))
 p=patch();p['replace_tokens']=[{'from':'neutral expression','to':'composed expression'}];add('repair_expression_token','visual-assets',trace([n],[req(),approve(),('repair',dict(key=0,patch=p)),req(),('inspect',dict(key=1))]))
 add('candidate_approval_mismatch','visual-assets',trace([n],[req(),('submit',dict(key=0,ticket=0,asset_uri='asset://submitted.png',digest='a'*64)),approve(),req()]))
 add('duplicate_submission_rejected','visual-assets',trace([n],[req(),('submit',dict(key=0,ticket=0,asset_uri='asset://submitted.png',digest='a'*64)),('submit',dict(key=0,ticket=0,asset_uri='asset://submitted.png',digest='a'*64))]))
 add('bad_asset_digest','visual-assets',trace([n],[req(),('approve',dict(key=0,asset_uri='asset://bad.png',digest='not-a-digest')),req()]))
 add('bad_asset_uri','visual-assets',trace([n],[req(),('approve',dict(key=0,asset_uri='javascript:bad',digest='a'*64)),req()]))
 p=patch();p['seed_delta']=1;add('queue_recovery_after_repair','visual-assets',trace([n,warm],[req(),req(1),('repair',dict(key=0,patch=p)),req(1)],queue_limit=1))
 p=patch();p['forbid_props']=['x'*128];add('negative_constraint_bounds','visual-assets',trace([n],[req(),('repair',dict(key=0,patch=p)),req()]))
 a=base();a['bible']['expressions'][0]['tokens']=['guarded expression']*2;add('duplicate_tokens','visual-spec',a,True)
 a=base();a['bible']['revision']=0;add('zero_bible_revision','visual-spec',a,True)
 a=base();e=copy.deepcopy(a['life']['definition']['attitudes'][0]);e.update(id='strain',kind='emotion',subject='merchant',level=50);a['life']['definition']['attitudes'].append(e);add('live_self_emotion','visual-spec',a)
 a=base();a['life']['definition']['attitudes'][0]['audience']=['merchant'];add('private_attitude_neutral','visual-spec',a)
 a=base();e=copy.deepcopy(a['life']['definition']['attitudes'][0]);e['id']='other_trust';a['life']['definition']['attitudes'].append(e);add('ambiguous_attitude_neutral','visual-spec',a)
 a=base();a['life']['definition']['attitudes'][0]['level']=10;a['life']['operations']=[dict(op='experience',args=dict(token=1,at=1,kind='outcome',tag='returned',subject='player',source=dict(id='receipt:1',speaker='merchant',text='Lamp returned.'),audience=['merchant','player'],text='Lamp returned.',causes=[],retain=True)),dict(op='appraise',args=dict(rule='returned_lamp',event=1,revision=1))];a['frame']['life_revision']=2;add('appraised_warm','visual-spec',a)
 return out
def expected(k,a):return r.compile(a) if k=='visual-spec' else r.workflow(a)
def binding_fixtures(outputs):
 fixtures=[]
 def add(op,a,result=None,error=False):fixtures.append(dict(request=dict(op=op,args=a),expected='ERROR' if error else r.encode(dict(result=result))))
 for n,k,a,error in cases():add('local',dict(kind=k,args=a),outputs[n],error)
 a=neutral(base());l=a['life'];cfg=dict(enabled=True,capacity=16,queue_limit=2,epoch='binding_run');registry=r.Registry(cfg);spec=r.compile(a);id='visual_demo';cid='visual_character'
 add('character_create',dict(id=cid,scope=l['scope'],actor=l['actor'],participants=l['participants']),dict(created=cid))
 add('character_life_configure',dict(id=cid,**{k:l[k] for k in ['setting','definition','defaults','overrides']}),dict(configured=cid))
 add('visual_create',dict(id=id,bible=a['bible'],config=cfg),dict(created=id))
 add('visual_create',dict(id=id,bible=a['bible'],config=cfg),error=True)
 def prepare():
  selection=registry.request(spec);add('visual_prepare',dict(id=id,frame=a['frame'],character=cid,activity=None),dict(compiled=spec,selection=selection));return selection
 selection=prepare();key=selection['key'];ticket=selection['generation']['ticket']
 add('visual_inspect',dict(id=id,key=key),copy.deepcopy(registry.rows[key]))
 add('visual_submit',dict(id=id,key=key,ticket=ticket,asset_uri='asset://draft.png',digest='a'*64),registry.submit(key,ticket,'asset://draft.png','a'*64))
 prepare()
 add('visual_approve',dict(id=id,key=key,asset_uri='asset://draft.png',digest='a'*64),registry.approve(key,'asset://draft.png','a'*64));prepare()
 p=patch();p['seed_delta']=1;result=registry.repair(key,p);add('visual_repair',dict(id=id,key=key,patch=p),result);new=result['key'];prepare()
 add('visual_submit',dict(id=id,key=key,ticket=ticket,asset_uri='asset://stale.png',digest='a'*64),error=True)
 p=patch();p['asset']=dict(asset_uri='asset://edited.png',digest='b'*64);add('visual_repair',dict(id=id,key=new,patch=p),registry.repair(new,p));prepare()
 add('visual_inspect',dict(id=id,key=new),copy.deepcopy(registry.rows[new]))
 add('visual_reject',dict(id=id,key=new,reason='Host revoked approval.'),registry.reject(new,'Host revoked approval.'));prepare()
 bad=copy.deepcopy(a['frame']);bad['life_revision']=99;add('visual_prepare',dict(id=id,frame=bad,character=cid,activity=None),error=True)
 add('visual_drop',dict(id=id),dict(dropped=id));add('visual_inspect',dict(id=id,key=key),error=True)
 for i,f in enumerate(fixtures):
  f['request']=dict(version=1,id='visual_'+str(i),**f['request'])
  if f['expected']!='ERROR':f['expected']=r.encode(dict(version=1,id='visual_'+str(i),ok=True,**json.loads(f['expected'])))
 return fixtures
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--prototype-only',action='store_true');p.add_argument('--tokens-only',action='store_true');p.add_argument('--write-conformance',action='store_true');p.add_argument('--check-bindings',action='store_true');p.add_argument('--report',type=pathlib.Path);p.add_argument('--check',type=pathlib.Path);args=p.parse_args();assert args.cap==0;outputs={}
 for n,k,a,error in cases():
  rejected=False
  try:wanted=expected(k,a)
  except (AssertionError,KeyError,ValueError,StopIteration,TypeError):wanted=None;rejected=True
  assert rejected==error,(n,'prototype rejection mismatch');outputs[n]=wanted
  req=dict(kind=k,args=a)
  if not(args.prototype_only or args.tokens_only):
   result=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=r.encode(req),text=True,capture_output=True);assert bool(result.returncode)==error,(n,result.stderr)
   if not error:assert result.stdout.strip()==r.encode(wanted),(n,result.stdout,r.encode(wanted))
  if args.write_conformance:
   d=ROOT/'conformance/visual'/n;d.mkdir(parents=True,exist_ok=True)
   for f,v in [('case',dict(op='local',**({'error':True} if error else {}))),('input',req)]+([] if error else [('expected',wanted)]):(d/(f+'.json')).write_text(r.encode(v)+'\n')
 assert outputs['repair_expression_token']['results'][3]['result']['asset'] is None
 assert outputs['foreign_prop_withheld']['spec']['visible_props']==[]
 assert outputs['live_self_emotion']['spec']['expression']['slot']=='tense'
 assert outputs['private_attitude_neutral']['spec']['expression']['slot']=='neutral'
 assert outputs['ambiguous_attitude_neutral']['spec']['expression']['slot']=='neutral'
 assert outputs['appraised_warm']['spec']['expression']['slot']=='warm'
 assert 'sunlight' not in r.encode(outputs['closed_shutters_no_global_sun']['spec'])
 assert outputs['same_slot_reuses_key']['results'][-1]['result']['exact']
 assert outputs['neutral_fallback_then_exact']['results'][2]['result']['asset'] is not None
 assert outputs['candidate_never_default_art']['results'][-1]['result']['asset'] is None
 assert outputs['sunny_fallback_not_visible_after_shutters']['results'][-1]['result']['asset'] is None
 report=dict(version=1,workflows=len(outputs),malformed_cases=sum(x[3] for x in cases()),actual_model_calls=0,api_spend_usd=0,provider_tokens=None,queued_generation_requests=sum(v.get('queued_generation_requests',0) for v in outputs.values() if v),exact_reuse=sum(x.get('result',{}).get('exact',False) for v in outputs.values() if v and 'results' in v for x in v['results']),approved_fallbacks=sum(x.get('result',{}).get('asset') is not None and not x['result'].get('exact',False) for v in outputs.values() if v and 'results' in v for x in v['results']),closed_shutters_excludes_sunlight='sunlight' not in r.encode(outputs['occlusion_wins']['spec']),stale_worker_rejected=not outputs['repair_invalidates_old_job']['results'][2]['ok'],frozen_life_model_sha256={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((ROOT/'eval').glob('life-model-*.json'))},limits='Offline deterministic metadata only; queued jobs are not dispatched. No image model, image-quality score, provider token count or measured frame-rate claim.')
 if args.report:assert not args.report.exists();args.report.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
 if args.check:assert json.loads(args.check.read_text())==report
 if args.check_bindings:
  module=importlib.util.spec_from_file_location('visual_bridge',ROOT/'bindings/python/gamectx_client.py');bridge=importlib.util.module_from_spec(module);sys.modules[module.name]=bridge;module.loader.exec_module(bridge);fixtures=binding_fixtures(outputs);checked=0
  for client in [bridge.Client(None,ROOT/'target/debug/gamectx'),bridge.NativeClient(None,ROOT/'bindings/native/target/debug/libgamectx_native.so')]:
   with client:
    for f in fixtures:
     try:got=client.request(f['request']['op'],f['request']['args']);assert f['expected']!='ERROR';assert r.encode(got)==r.encode(json.loads(f['expected'])['result'])
     except ValueError:assert f['expected']=='ERROR'
     checked+=1
  d=ROOT/'target/visual-csharp';d.mkdir(parents=True,exist_ok=True)
  for filename,rows in [('cases.json',[r.encode(fixtures)]),('requests.txt',[r.encode(f['request']) for f in fixtures]),('expected.txt',[f['expected'] for f in fixtures])]:(d/filename).write_text('\n'.join(rows)+'\n')
  print(r.encode(dict(binding_checks=checked,actual_model_calls=0)))
 print(r.encode(report))
if __name__=='__main__':main()
