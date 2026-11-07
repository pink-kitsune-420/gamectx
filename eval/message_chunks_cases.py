"""Independent bounded long-message slicing and sentence reply prototype."""
import argparse,json,pathlib,subprocess,re
ROOT=pathlib.Path(__file__).resolve().parents[1]
def enc(v):return json.dumps(v,ensure_ascii=False,separators=(',',':'))
def complete(s):
 body=s.rstrip().rstrip('"\'”’)]').rstrip();count=0;in_run=False
 for i,ch in enumerate(s):
  end=ch in '.!?。！？'and not(ch=='.'and i>0 and s[i-1].isdigit()and i+1<len(s)and s[i+1].isdigit())
  if end and not in_run:count+=1
  in_run=end
 return bool(body)and body[-1]in '.!?。！？'and not body.endswith(('...','…'))and count==1
def oracle(a):
 assert set(a)=={'op','args'};v=a['args']
 if a['op']=='player':
  assert set(v)=={'text','max_chars'};s=v['text'];assert isinstance(s,str)and type(v['max_chars'])is int and 512<=v['max_chars']<=4096 and 1<=len(s)<=v['max_chars']and len(s.encode())<=16384 and not any((ord(c)<32 or 127<=ord(c)<=159)and c not in '\n\r\t'for c in s)
  s=s.replace('\r',' ').replace('\n',' ').replace('\t',' ');out=[]
  while s:
   n=0;b=0;last=None
   for c in s:
    if n==384 or b+len(c.encode())>960:break
    n+=1;b+=len(c.encode())
    if c.isspace()or c in '.!?':last=n
   end=last if n<len(s)and last and last>=n//2 else n;out.append(s[:end]);s=s[end:]
  assert len(out)<=32;return dict(chunks=out,characters=sum(len(x)for x in out),summary_authoritative=False)
 assert a['op']=='reply'and set(v)=={'speech','max_chunks'}and type(v['max_chunks'])is int and 1<=v['max_chunks']<=3 and 1<=len(v['speech'])<=v['max_chunks'];seen=[]
 for s in v['speech']:
  assert isinstance(s,str)and 1<=len(s)<=240 and len(s.encode())<=1024 and not any(ord(c)<32 or 127<=ord(c)<=159 for c in s)and complete(s) and s.lower()not in seen;seen.append(s.lower())
 return dict(chunks=v['speech'],characters=sum(len(s)for s in v['speech']),model_calls=1)
def cases():
 rows=[]
 for name,s,cap,error in [('short','Hello.',512,False),('long',('I prefer quiet places. '*100),4096,False),('unicode','😀'*1200,2048,False),('boundary','a'*384+' b.',512,False),('no_space','a'*900,1024,False),('newlines','Hello.\nMy name is Vela, not Vira.\tRemember that.',512,False),('oversize','a'*4097,4096,True),('control','a\x00b',512,True),('empty','',512,True),('invalid_cap','Hello.',100,True)]:rows.append((name,dict(op='player',args=dict(text=s,max_chars=cap)),error))
 for name,s,n,error in [('two_sentences',['I prefer quiet rooms.','I hope we can talk here.'],2,False),('single',['Hello.'],1,False),('quoted',['\"Hello.\"'],1,False),('decimal',['The price is 3.5 coins.'],1,False),('trailing_space',['Hello.  '],1,False),('wall',['I can also show you a '],3,True),('ellipsis',['I wonder...'],3,True),('duplicate',['Hello.','hello.'],3,True),('multiple_in_chunk',['Hello. Goodbye.'],2,True),('too_many',['One.','Two.','Three.','Four.'],3,True),('empty_reply',[],3,True),('oversize_reply',['a'*240+'.'],3,True)]:rows.append((name,dict(op='reply',args=dict(speech=s,max_chunks=n)),error))
 return rows
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,args,error in cases():
  try:expected=oracle(args);assert not error,name
  except(AssertionError,KeyError,TypeError):assert error,name;expected=None
  req=dict(kind='studio-message-chunks',args=args)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(req),capture_output=True,text=True);assert(r.returncode!=0)==error,(name,r.stderr)
   if not error:assert r.stdout.strip()==enc(expected),(name,r.stdout,expected)
  if a.write_conformance:
   d=ROOT/'conformance/message-chunks'/name;d.mkdir(parents=True,exist_ok=True)
   for k,x in [('case',dict(op='local',**(dict(error=True)if error else{}))),('input',req)]+([]if error else[('expected',expected)]):
    f=d/(k+'.json');assert not f.exists();f.write_text(enc(x)+'\n')
 print(enc(dict(workflows=len(cases()),api_spend_usd=0,provider_calls=0)))
