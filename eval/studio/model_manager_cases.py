"""Independent deterministic residency/escalation policy oracle; no network or models."""
import argparse,copy,json,pathlib,subprocess,unicodedata
ROOT=pathlib.Path(__file__).resolve().parents[2]
def enc(x):return json.dumps(x,ensure_ascii=False,separators=(',',':'))
def config():return json.loads((ROOT/'examples/lantern-vault/web/models.json').read_text())
def signal(**kw):return dict(turn=1,health=6,combat=False,atmosphere='calm',significance=0,attitude_delta=0,conflict=False,next=[],available=['text','dramatic_text','image','dramatic_image','tts','stt'])|kw
def oracle(a):
 c=a['config'];assert set(c)==set('version enabled mode text_escalation image_escalation residency idle_seconds prewarm ram_limit_mib reserve_mib rules profiles'.split())
 assert c['version']==1 and type(c['enabled'])is bool and c['mode'] in ['auto','normal','dramatic'] and c['residency'] in ['on_demand','warm']
 for k in ['text_escalation','image_escalation','prewarm']:assert type(c[k])is bool
 for k,lo,hi in [('idle_seconds',1,3600),('ram_limit_mib',256,65536),('reserve_mib',128,8192)]:assert type(c[k])is int and lo<=c[k]<=hi
 r=c['rules'];assert set(r)==set('combat emergency low_health significance attitude_change conflict hold_turns cooldown_turns'.split())
 for k in ['combat','emergency','conflict']:assert type(r[k])is bool
 for k,hi in [('low_health',100),('significance',100),('attitude_change',200),('hold_turns',100),('cooldown_turns',100)]:assert type(r[k])is int and 0<=r[k]<=hi
 profiles=c['profiles'];assert set(profiles)==set('text dramatic_text image dramatic_image tts stt'.split())
 for k,p in profiles.items():
  fields=['model','ram_mib','backend']+(['size'] if 'text' in k else ['recipe','selection','steps'] if 'image' in k else [])
  assert set(p)==set(fields) and isinstance(p['model'],str) and p['model'] and len(p['model'])<=128 and all(x.isascii() and (x.isalnum() or x in '-_:./') for x in p['model'])
  assert p['backend'] in (['inherit','local_ollama','online_openai_compatible'] if 'text' in k else ['inherit','local_sd_cpp','online_image_api'] if 'image' in k else ['inherit','off','browser_web_speech','local_tts_http','online_tts_api'] if k=='tts' else ['inherit','off','browser_speech_recognition','local_stt_http','online_stt_api'])
  assert type(p['ram_mib'])is int and 64<=p['ram_mib']<=65536
  if 'text' in k:assert p['size'] in ['1','1.5','3','8','30','70']
  if 'image' in k:
   assert p['recipe'] and len(p['recipe'])<=128 and all(x.isascii()and(x.isalnum()or x in '-_.') for x in p['recipe'])
   assert type(p['steps'])is int and 1<=p['steps']<=64 and isinstance(p['selection'],dict) and 1<=len(p['selection'])<=8
   for slot,v in p['selection'].items():assert slot and len(slot)<=128 and all(x.isascii()and(x.isalnum()or x in '-_')for x in slot) and isinstance(v,str) and len(v.encode())<=256 and not v.startswith('/') and '..' not in v.split('/') and not any(unicodedata.category(x)=='Cc' for x in v)
 out=[];last=0;active=False;until=0;after=0
 assert len(a['signals'])<=64
 for s in a['signals']:
  assert set(s)==set('turn health combat atmosphere significance attitude_delta conflict next available'.split())
  assert type(s['turn'])is int and last<=s['turn']<=2**32-201 and s['turn']>0
  for k,hi in [('health',100),('significance',100),('attitude_delta',200)]:assert type(s[k])is int and 0<=s[k]<=hi
  for k in ['combat','conflict']:assert type(s[k])is bool
  assert s['atmosphere'] in ['calm','busy','tense','emergency'] and len(s['next'])<=4 and len(set(s['next']))==len(s['next']) and set(s['next'])<=set(['text','image','tts','stt'])
  assert len(s['available'])<=6 and len(set(s['available']))==len(s['available']) and set(s['available'])<=set(profiles)
  reasons=[]
  for key,yes in [('combat',r['combat']and s['combat']),('emergency',r['emergency']and s['atmosphere']=='emergency'),('low_health',r['low_health']>0 and s['health']<=r['low_health']),('significance',r['significance']>0 and s['significance']>=r['significance']),('attitude_change',r['attitude_change']>0 and s['attitude_delta']>=r['attitude_change']),('conflict',r['conflict']and s['conflict'])]:
   if yes:reasons.append(key)
  if not c['enabled'] or c['mode']=='normal':active=False;reasons=[];until=0;after=0
  elif c['mode']=='dramatic':active=True;reasons=['manual'];until=s['turn']+r['hold_turns']
  elif reasons and (active or s['turn']>=after or 'emergency' in reasons):active=True;until=s['turn']+r['hold_turns']
  elif active and s['turn']<=until:reasons=['hold']
  else:
   if active:after=s['turn']+r['cooldown_turns']
   active=False;reasons=['cooldown'] if reasons else []
  fallbacks=[]
  def choose(channel):
   desired='dramatic_'+channel if active and c[channel+'_escalation'] else channel
   if not c['enabled']:return channel
   for key in dict.fromkeys([desired,channel]):
    if key in s['available'] and (profiles[key]['backend'].startswith('online_') or profiles[key]['ram_mib']<=c['ram_limit_mib']):return key
    fallbacks.append(key+(':unavailable' if key not in s['available'] else ':ram_limit'))
   return 'fallback' if channel=='text' else 'cached'
  text=choose('text');image=choose('image');warm=[]
  if c['enabled'] and c['prewarm']:
   for channel in s['next']:
    key=text if channel=='text' else image if channel=='image' else channel
    if key in profiles and key in s['available'] and (profiles[key]['backend']=='inherit' or profiles[key]['backend'].startswith('local_')) and profiles[key]['ram_mib']<=c['ram_limit_mib'] and key not in warm:warm.append(key)
  out.append(dict(dramatic=active,reasons=reasons,text=text,image=image,art='detailed' if image=='dramatic_image' else 'standard',prewarm=warm,held_until=until,retry_after=after,fallbacks=fallbacks));last=s['turn']
 return dict(turns=out)
def cases():
 c=config();c.update(enabled=True,text_escalation=True,image_escalation=True,ram_limit_mib=6144,prewarm=True)
 out=[]
 def add(name,signals,cfg=c,error=False):out.append((name,dict(config=copy.deepcopy(cfg),signals=signals),error))
 add('normal',[signal()]);add('combat',[signal(combat=True)]);add('emergency',[signal(atmosphere='emergency')]);add('low_health',[signal(health=2)]);add('significant',[signal(significance=95)]);add('attitude_crossing',[signal(attitude_delta=30)]);add('conflict',[signal(conflict=True)])
 add('manual',[signal()],c|dict(mode='dramatic'));add('normal_override',[signal(combat=True)],c|dict(mode='normal'));add('disabled',[signal(combat=True)],c|dict(enabled=False));add('missing_heavy',[signal(combat=True,available=['text','image'])]);add('ram_limit',[signal(combat=True)],c|dict(ram_limit_mib=4096));add('text_only',[signal(combat=True)],c|dict(image_escalation=False));add('prewarm',[signal(next=['text','tts'])]);add('no_prewarm',[signal(next=['text'])],c|dict(prewarm=False));add('fallback',[signal(available=[])]);add('hold_cooldown',[signal(combat=True),signal(turn=2),signal(turn=5),signal(turn=6,combat=True),signal(turn=7,atmosphere='emergency')]);add('duplicate_available',[signal(available=['text','text'])],error=True);add('clock_rollback',[signal(turn=5),signal(turn=4)],error=True);add('unknown_mode',[signal()],c|dict(mode='anything'),True);add('unknown_rule',[signal()],c|dict(rules=c['rules']|dict(invented=True)),True)
 online=copy.deepcopy(c);online['ram_limit_mib']=4096
 for key in ['dramatic_text','dramatic_image']:online['profiles'][key]['backend']='online_openai_compatible' if 'text' in key else 'online_image_api'
 add('online_dramatic',[signal(combat=True,next=['text','image'])],online);add('online_unavailable',[signal(combat=True,available=['text','image'])],online)
 invalid=copy.deepcopy(c);invalid['profiles']['text']['backend']='shell';add('unknown_backend',[signal()],invalid,True)
 browser=copy.deepcopy(c);browser['profiles']['tts']['backend']='browser_web_speech';add('browser_audio',[signal(next=['tts'])],browser)
 empty=copy.deepcopy(c);empty['profiles']['text']['model']='';add('empty_model',[signal()],empty,True)
 spaced=copy.deepcopy(c);spaced['profiles']['image']['selection']['model']='checkpoints/normal model.gguf';add('file_id_with_space',[signal()],spaced)
 traversal=copy.deepcopy(c);traversal['profiles']['image']['selection']['model']='../secret.gguf';add('file_id_traversal',[signal()],traversal,True)
 add('bad_residency',[signal()],c|dict(residency='forever'),True);add('invalid_prediction',[signal(next=['anything'])],error=True);add('too_many_turns',[signal()for _ in range(65)],error=True);add('image_only',[signal(combat=True)],c|dict(text_escalation=False))
 return out
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,args,error in cases():
  try:wanted=oracle(args);assert not error,name
  except (AssertionError,KeyError,TypeError):assert error,name;wanted=None
  request=dict(kind='model-manager',args=args)
  if not a.prototype_only:
   run=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(request),capture_output=True,text=True);assert (run.returncode!=0)==error,(name,run.stderr)
   if not error:assert run.stdout.strip()==enc(wanted),(name,run.stdout,enc(wanted))
  if a.write_conformance:
   d=ROOT/'conformance/model-manager'/name;d.mkdir(parents=True,exist_ok=True)
   for k,v in [('case',dict(op='local',**({'error':True}if error else{}))),('input',request)]+([] if error else [('expected',wanted)]):(d/(k+'.json')).write_text(enc(v)+'\n')
 print(enc(dict(workflows=len(cases()),actual_provider_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
