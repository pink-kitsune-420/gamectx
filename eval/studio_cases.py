"""Predeclared local UI routing/voice boundaries; no transport/provider calls."""
import argparse,copy,importlib.util,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import studio_reference as r
ROOT=pathlib.Path(__file__).resolve().parents[1]
def op(k,**v):return dict(op=k,args=v)
def reserve(m,b,c=0,local=False):return op('reserve',modality=m,backend=b,ceiling=c,local_fallback=local)
def cases():
 out=[]
 for m,bs in r.BACKENDS.items():
  for b in bs:out.append((m+'_'+b,'studio-routing',dict(cap=0,operations=[reserve(m,b,10),op('inspect')]),False))
 for m in r.BACKENDS:
  out.append(('paid_'+m,'studio-routing',dict(cap=10,operations=[reserve(m,r.BACKENDS[m][-1],10),op('dispatch',id=1),op('finish',id=1),reserve(m,r.BACKENDS[m][-1],10),op('inspect')]),False))
  out.append(('local_cap_fallback_'+m,'studio-routing',dict(cap=0,operations=[reserve(m,r.BACKENDS[m][-1],10,True),op('inspect')]),False))
 out += [('shared_cap','studio-routing',dict(cap=20,operations=[reserve('text','online_openai_compatible',10),reserve('image','online_image_api',10),reserve('stt','online_stt_api',1),op('dispatch',id=1),op('cancel',id=1),op('finish',id=1),op('finish',id=1),op('cancel',id=2),op('limit',cap=9),op('limit',cap=10),op('limit',cap=21),op('inspect')]),False),('unpriced_online','studio-routing',dict(cap=20,operations=[reserve('text','online_openai_compatible'),op('inspect')]),False),('unknown_backend','studio-routing',dict(cap=0,operations=[reserve('tts','unlisted')]),False),('negative_cap','studio-routing',dict(cap=-1,operations=[]),True)]
 text='A😀 lamp is ready. The hidden tail.'
 for name,events in [('complete',[op('queue'),op('finish',through=len(text),heard=len(text))]),('barge_in',[op('queue'),op('progress',through=8,heard=5),op('interrupt',through=8,heard=5),op('finish',through=len(text),heard=len(text))]),('before_play',[op('interrupt',through=1,heard=0),op('interrupt',through=0,heard=0)]),('bad_offsets',[op('queue'),op('progress',through=4,heard=5),op('progress',through=1000,heard=0),op('progress',through=8,heard=5),op('progress',through=7,heard=5),op('interrupt',through=8,heard=4),op('finish',through=9,heard=9)]),('interim_no_actions',[op('queue'),op('recognition',text='buy oil',final=False,uncertainty='still listening'),op('recognition',text='buy oil',final=True,uncertainty=None)]),('duplicate_end',[op('queue'),op('finish',through=len(text),heard=len(text)),op('interrupt',through=len(text),heard=len(text))])]:out.append((name,'studio-voice',dict(text=text,operations=events+[op('inspect')]),False))
 return out
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--prototype-only',action='store_true');p.add_argument('--tokens-only',action='store_true');p.add_argument('--write-conformance',action='store_true');p.add_argument('--report',type=pathlib.Path);p.add_argument('--check',type=pathlib.Path);p.add_argument('--check-bindings',action='store_true');a=p.parse_args();assert a.cap==0;outputs={}
 for n,k,v,error in cases():
  failed=False
  try:w=(r.routing if k=='studio-routing' else r.voice)(v)
  except (AssertionError,ValueError,KeyError,TypeError):w=None;failed=True
  assert failed==error,n;outputs[n]=w;request=dict(kind=k,args=v)
  if not(a.prototype_only or a.tokens_only):
   got=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=r.enc(request),capture_output=True,text=True);assert bool(got.returncode)==error,(n,got.stderr)
   if not error:assert got.stdout.strip()==r.enc(w),(n,got.stdout,r.enc(w))
  if a.write_conformance:
   d=ROOT/'conformance/studio'/n;d.mkdir(parents=True,exist_ok=True)
   for f,x in [('case',dict(op='local',**({'error':True} if error else {}))),('input',request)]+([] if error else [('expected',w)]):(d/(f+'.json')).write_text(r.enc(x)+'\n')
 assert outputs['barge_in']['results'][-1]['result']['heard_text']=='A😀 la'
 assert outputs['interim_no_actions']['results'][1]['result']['actions']==[]
 report=dict(version=1,workflows=len(outputs),actual_provider_calls=0,api_spend_usd=0,provider_tokens=None,barge_in_heard_text=outputs['barge_in']['results'][-1]['result']['heard_text'],limits='Offline routing and receipt metadata only. No installed provider/browser audio quality, physical heard-character accuracy or frame-rate measurement.')
 if a.report:assert not a.report.exists();a.report.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
 if a.check:assert json.loads(a.check.read_text())==report
 if a.check_bindings:
  module=importlib.util.spec_from_file_location('studio_bridge',ROOT/'bindings/python/gamectx_client.py');bridge=importlib.util.module_from_spec(module);module.loader.exec_module(bridge);fixtures=[]
  for i,(n,k,v,error) in enumerate(cases()):
   id='studio_'+str(i);fixtures.append(dict(request=dict(version=1,id=id,op='local',args=dict(kind=k,args=v)),expected='ERROR' if error else r.enc(dict(version=1,id=id,ok=True,result=outputs[n]))))
  for client in [bridge.Client(None,ROOT/'target/debug/gamectx'),bridge.NativeClient(None,ROOT/'bindings/native/target/debug/libgamectx_native.so')]:
   with client:
    for f in fixtures:
     try:got=client.request('local',f['request']['args']);assert f['expected']!='ERROR';assert r.enc(got)==r.enc(json.loads(f['expected'])['result'])
     except ValueError:assert f['expected']=='ERROR'
  d=ROOT/'target/studio-csharp';d.mkdir(parents=True,exist_ok=True)
  for name,rows in [('cases.json',[r.enc(fixtures)]),('requests.txt',[r.enc(f['request']) for f in fixtures]),('expected.txt',[f['expected'] for f in fixtures])]:(d/name).write_text('\n'.join(rows)+'\n')
  print(r.enc(dict(binding_fixtures=len(fixtures),actual_provider_calls=0)))
 print(r.enc(report))
if __name__=='__main__':main()
