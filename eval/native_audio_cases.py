"""Independent native-audio request-plan oracle; no engines or network."""
import argparse,json,pathlib,subprocess,copy
ROOT=pathlib.Path(__file__).resolve().parents[1]
def enc(v):return json.dumps(v,ensure_ascii=False,separators=(',',':'))
def oracle(a):
 assert set(a)=={'engine','request'};r=a['request'];engine=a['engine'];assert engine in ['piper','whisper']
 def ident(s):assert isinstance(s,str)and 0<len(s)<=128 and all(c.isascii()and(c.isalnum()or c in '-_.')for c in s)
 ident(r['model'])
 if engine=='whisper':assert set(r)=={'model'};return dict(model=r['model'],threads=4,format='text')
 assert set(r)=={'model','input','voice','speed','response_format'};ident(r['voice']);assert r['response_format']=='wav';assert isinstance(r['input'],str)and 0<len(r['input'])<=240 and not any(ord(c)<32 or 127<=ord(c)<160 for c in r['input']);assert type(r['speed'])in[int,float]and .5<=r['speed']<=2
 return dict(voice=r['voice'],text=r['input'],length_scale=f"{1/r['speed']:.6f}",threads=4)
def cases():
 base=dict(engine='piper',request=dict(model='piper',input='Welcome, traveler.',voice='merchant',speed=1,response_format='wav'));out=[]
 def add(name,a,error=False):out.append((name,a,error))
 add('piper',base);add('unicode',base|dict(request=base['request']|dict(input='Hello, Vela — tea?')));add('fast',base|dict(request=base['request']|dict(speed=2)));add('slow',base|dict(request=base['request']|dict(speed=.5)));add('whisper',dict(engine='whisper',request=dict(model='tiny-en')))
 for name,patch in [('empty',dict(input='')),('long',dict(input='a'*241)),('newline',dict(input='a\nb')),('voice_path',dict(voice='../voice')),('speed',dict(speed=0)),('format',dict(response_format='mp3')),('extra',dict(argv=['bad']))]:add(name,base|dict(request=base['request']|patch),True)
 add('unknown',base|dict(engine='shell'),True)
 return out
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,args,error in cases():
  try:expected=oracle(args);assert not error
  except(AssertionError,KeyError,TypeError):assert error;expected=None
  request=dict(kind='studio-native-audio',args=args)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(request),capture_output=True,text=True);assert(r.returncode!=0)==error,(name,r.stderr)
   if not error:assert r.stdout.strip()==enc(expected),(name,r.stdout,expected)
  if a.write_conformance:
   d=ROOT/'conformance/native-audio'/name;d.mkdir(parents=True,exist_ok=True)
   for key,v in [('case',dict(op='local',**(dict(error=True)if error else{}))),('input',request)]+([]if error else[('expected',expected)]):(d/(key+'.json')).write_text(enc(v)+'\n')
 print(enc(dict(workflows=len(cases()),actual_provider_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
