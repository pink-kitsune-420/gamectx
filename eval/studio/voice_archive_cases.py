"""Independent bounded voice-recording retention oracle; no files or inference."""
import argparse,json,pathlib,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];CAP=32*1024*1024
enc=lambda v:json.dumps(v,separators=(',',':'))
def oracle(a):
 assert set(a)=={'config','ops'};records=[];cfg=a['config'];assert len(a['ops'])<=256
 def validate(c):assert set(c)=={'version','enabled','limit'}and c['version']==1 and type(c['enabled'])is bool and type(c['limit'])is int and 1<=c['limit']<=128
 validate(cfg);out=[]
 for op in a['ops']:
  assert set(op)=={'op','args'};v=op['args'];removed=[]
  if op['op']=='configure':validate(v);cfg=v
  elif op['op']=='clear':assert v=={};removed=[r['key']for r in records];records=[]
  else:
   assert op['op']=='remember'and set(v)=={'key','bytes'};assert isinstance(v['key'],str)and len(v['key'])==64 and all(c in '0123456789abcdef'for c in v['key']);assert type(v['bytes'])is int and 1<=v['bytes']<=6*1024*1024
   old=next((r for r in records if r['key']==v['key']),None)
   if old is not None:assert old['bytes']==v['bytes']
   elif cfg['enabled']:records.append(v)
  while records and(not cfg['enabled']or len(records)>cfg['limit']or sum(r['bytes']for r in records)>CAP):removed.append(records.pop(0)['key'])
  out.append(dict(kept=records.copy(),removed=removed,bytes=sum(r['bytes']for r in records)))
 return dict(results=out,config=cfg)
def cases():
 c=dict(version=1,enabled=True,limit=2);key=lambda n:format(n,'064x');remember=lambda n,size=100:dict(op='remember',args=dict(key=key(n),bytes=size));conf=lambda x:dict(op='configure',args=x)
 return [('off',dict(config=c|dict(enabled=False),ops=[remember(1)]),False),('fifo',dict(config=c,ops=[remember(1),remember(2),remember(3)]),False),('reuse',dict(config=c,ops=[remember(1),remember(1)]),False),('disable',dict(config=c,ops=[remember(1),conf(c|dict(enabled=False))]),False),('resize',dict(config=c,ops=[remember(1),remember(2),conf(c|dict(limit=1))]),False),('clear',dict(config=c,ops=[remember(1),dict(op='clear',args={})]),False),('byte_cap',dict(config=c|dict(limit=128),ops=[remember(n,6*1024*1024)for n in range(7)]),False),('bad_key',dict(config=c,ops=[dict(op='remember',args=dict(key='../audio',bytes=1))]),True),('bad_limit',dict(config=c|dict(limit=0),ops=[]),True),('bad_size',dict(config=c,ops=[remember(1,0)]),True),('changed_identity',dict(config=c,ops=[remember(1),remember(1,200)]),True),('too_many_ops',dict(config=c,ops=[remember(1)]*257),True)]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,args,error in cases():
  try:wanted=oracle(args);assert not error
  except(AssertionError,KeyError,TypeError):assert error;wanted=None
  request=dict(kind='studio-voice-archive',args=args)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(request),capture_output=True,text=True);assert(r.returncode!=0)==error,(name,r.stderr)
   if not error:assert r.stdout.strip()==enc(wanted),(name,r.stdout,wanted)
  if a.write_conformance:
   d=ROOT/'conformance/voice-archive'/name;d.mkdir(parents=True,exist_ok=True)
   for k,v in [('case',dict(op='local',**(dict(error=True)if error else{}))),('input',request)]+([]if error else[('expected',wanted)]):(d/(k+'.json')).write_text(enc(v)+'\n')
 print(enc(dict(workflows=len(cases()),actual_provider_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
