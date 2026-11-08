"""Independent v2 compiler and boundary cases. No model calls, old outputs stay frozen."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,copy,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import life_speech_reference as ref
from life_model_v2_eval import compiler_args,life,suite
ROOT=pathlib.Path(__file__).resolve().parents[2]
def cases():
 out=[]
 for c in suite():out.append((c['id'],dict(life=life(),request=compiler_args(c)),False))
 base=dict(life=life(),request=compiler_args(suite()[1]))
 for profile in ('focused','expanded'):
  for knowledge in ('known','partial','unknown'):
   for creative in (False,True):
    a=copy.deepcopy(base);t=a['request']['turn'];t.update(profile=profile,knowledge=knowledge,creative=creative)
    if knowledge!='known':t['facts'].append(dict(key='player_birthplace',subject='player',status='unknown',value=None))
    out.append((profile+'_'+knowledge+'_'+str(creative).lower(),a,False))
 a=copy.deepcopy(base);a['life']['defaults']['balanced']=False;a['request']['turn'].update(profile='expanded');out.append(('balanced_off',a,False))
 a=copy.deepcopy(base);a['request']['dialogue']=[dict(user='Earlier question.',assistant='An earlier answer.')];out.append(('history_then_current',a,False))
 for name,mutate in [('known_with_unknown',lambda a:a['request']['turn']['facts'].append(dict(key='player_birthplace',subject='player',status='unknown',value=None))),('unknown_without_unknown',lambda a:a['request']['turn'].update(knowledge='unknown')),('foreign_subject',lambda a:a['request']['turn'].update(question_subject='outsider')),('foreign_speaker',lambda a:a['request']['turn'].update(speaker='outsider')),('duplicate_key',lambda a:a['request']['turn']['facts'].append(copy.deepcopy(a['request']['turn']['facts'][0]))),('reserved_key',lambda a:a['request']['turn']['facts'][0].update(key='question_subject')),('facts_over_cap',lambda a:a['request']['caps'].update(facts=0)),('unknown_with_value',lambda a:a['request']['turn']['facts'][0].update(status='unknown')),('extra_turn_field',lambda a:a['request']['turn'].update(approve=True)),('history_over_cap',lambda a:a['request'].update(dialogue=[dict(user='Old.',assistant='Old.')] * 2))]:
  a=copy.deepcopy(base);mutate(a);out.append((name,a,True))
 return out
def expected(a):return dict(character=ref.character(a['life'],a['request']['turn'],a['request']['caps'],a['request']['tags']),request=ref.compile_request(a['life'],a['request']))
def sentences():
 return [('complete','The lamp is ready.',240),('comma_wall','x'*239+',',240),('space_wall','x'*239+' ',240),('period_wall','x'*239+'.',240),('unknown_subject',"I don't know your birthplace.",240),('two_sentences','It is ready. I cannot see the sky.',240),('quoted','“Your lamp is ready.”',240),('decimal','It costs 3.5 coins.',240),('ellipsis','The lamp...',240),('cjk','準備できました。',240),('empty','',240),('over_cap','x'*240+'.',240),('no_punctuation','Your lamp is ready',240),('abbreviation','Ask Dr. Ilen.',240)]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');p.add_argument('--check-bindings',action='store_true');a=p.parse_args();assert a.cap==0
 for name,v,error in cases():
  try:wanted=expected(v);assert not error
  except (AssertionError,ValueError,KeyError,TypeError):
   if not error:raise
   wanted=None
  request=dict(kind='life-speech-v2',args=v)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=json.dumps(request),text=True,capture_output=True);assert bool(r.returncode)==error,(name,r.stderr)
   if not error:assert r.stdout.strip()==ref.encode(wanted),(name,r.stdout,ref.encode(wanted))
  if a.write_conformance and name in ('unknown','focused_known_false','expanded_known_false','expanded_unknown_false','foreign_subject','known_with_unknown','reserved_key','history_then_current'):
   d=ROOT/'conformance/life-speech-v2'/name;d.mkdir(parents=True,exist_ok=True)
   for f,o in [('case',dict(op='local',**({'error':True} if error else {}))),('input',request)]+([] if error else [('expected',wanted)]):(d/(f+'.json')).write_text(ref.encode(o)+'\n')
 for name,speech,cap in sentences():
  wanted=ref.assess_sentence(speech,cap)
  request=dict(kind='sentence-boundary',args=dict(text=speech,max_chars=cap))
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=ref.encode(request),text=True,capture_output=True);assert r.returncode==0,(name,r.stderr);assert r.stdout.strip()==ref.encode(wanted),(name,r.stdout,wanted)
  if a.write_conformance and name in ('complete','comma_wall','period_wall','two_sentences','over_cap'):
   d=ROOT/'conformance/sentence-boundary'/name;d.mkdir(parents=True,exist_ok=True)
   for f,o in [('case',dict(op='local')),('input',request),('expected',wanted)]:(d/(f+'.json')).write_text(ref.encode(o)+'\n')
 for name,speech,cap in [('zero_cap','Ready.',0),('oversized','x'*1025,240)]:
  try:ref.assess_sentence(speech,cap);raise RuntimeError('expected boundary error')
  except AssertionError:pass
  request=dict(kind='sentence-boundary',args=dict(text=speech,max_chars=cap))
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=ref.encode(request),text=True,capture_output=True);assert r.returncode!=0,name
  if a.write_conformance:
   d=ROOT/'conformance/sentence-boundary'/name;d.mkdir(parents=True,exist_ok=True)
   for f,o in [('case',dict(op='local',error=True)),('input',request)]:(d/(f+'.json')).write_text(ref.encode(o)+'\n')
 if a.check_bindings:
  import importlib.util
  spec=importlib.util.spec_from_file_location('bridge',ROOT/'bindings/python/gamectx_client.py');bridge=importlib.util.module_from_spec(spec);spec.loader.exec_module(bridge)
  checks=0
  for client in [bridge.Client(None,ROOT/'target/debug/gamectx'),bridge.NativeClient(None,ROOT/'bindings/native/target/debug/libgamectx_native.so')]:
   with client:
    for i,(name,v,error) in enumerate(cases()):
     l=v['life'];npc=bridge.Character(client,'speech:'+str(i),l['scope'],l['actor'],l['participants']);npc.life_configure(l['setting'],l['definition'],l['defaults'],l['overrides'])
     for op in l['operations']:
      if op['op']=='experience':npc.life_experience(op['args'])
      elif op['op']=='appraise':npc.life_appraise(op['args']['rule'],op['args']['event'],op['args']['revision'])
     try:got=npc.life('compile_speech',request=v['request']);assert not error,(name,got);assert ref.encode(got)==ref.encode(expected(v)['request']),name
     except ValueError:assert error,name
     npc.close();checks+=1
  print(ref.encode(dict(binding_checks=checks,api_spend_usd=0,model_calls=0)))
 print(ref.encode(dict(compiler_workflows=len(cases()),sentence_checks=len(sentences())+2,api_spend_usd=0,model_calls=0)))
if __name__=='__main__':main()
