"""Independent bounded utterance-buffer plan; no engines or network."""
import argparse,json,pathlib,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[1]
def enc(v):return json.dumps(v,ensure_ascii=False,separators=(',',':'))
def oracle(a):
 assert set(a)=={'text','start'};text=a['text'];start=a['start'];assert isinstance(text,str)and 0<len(text)<=240 and not any(ord(c)<32 or 127<=ord(c)<160 for c in text);assert type(start)is int and 0<=start<len(text)
 parts=[];pos=start
 while pos<len(text):
  end=min(pos+80,len(text));cut=None
  for i in range(pos,end):
   if text[i]in'.!?'and(i+1==len(text)or text[i+1].isspace()):cut=i+1;break
  if cut is None:
   cut=end
   if end<len(text):
    spaces=[i+1 for i in range(pos,end)if text[i].isspace()]
    if spaces:cut=spaces[-1]
  while cut<len(text)and cut-pos<80 and text[cut].isspace():cut+=1
  parts.append(dict(text=text[pos:cut],start=pos,through=cut));pos=cut
 return dict(parts=parts,through=len(text))
def cases():
 out=[]
 def add(n,text,start=0,error=False):out.append((n,dict(text=text,start=start),error))
 add('sentence','Welcome, traveler.');add('two_sentences','Oil is on the shelf. Chalk is beside it.');add('unicode','Hello, Vela 😀. Your lamp is ready.');add('resume','One finished sentence. One unheard sentence.',23);add('max','a'*240);add('phrases','A long quiet corridor with brass lamps and folded maps leads toward the old courtyard, where Vela waits beside the well.');add('spaces','  A lamp.   Chalk! Tea?  ');add('decimal','It costs 3.5 coins.');add('empty','',0,True);add('too_long','a'*241,0,True);add('control','Hidden\ntext',0,True);add('past_end','A lamp.',7,True);add('negative','A lamp.',-1,True);add('non_integer','A lamp.',True,True)
 return out
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,args,error in cases():
  try:expected=oracle(args);assert not error
  except(AssertionError,KeyError,TypeError):assert error;expected=None
  request=dict(kind='studio-speech-buffer',args=args)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(request),capture_output=True,text=True);assert(r.returncode!=0)==error,(name,r.stderr)
   if not error:assert r.stdout.strip()==enc(expected),(name,r.stdout,expected)
  if a.write_conformance:
   d=ROOT/'conformance/speech-buffer'/name;d.mkdir(parents=True,exist_ok=True)
   for key,v in [('case',dict(op='local',**(dict(error=True)if error else{}))),('input',request)]+([]if error else[('expected',expected)]):(d/(key+'.json')).write_text(enc(v)+'\n')
 print(enc(dict(workflows=len(cases()),actual_provider_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
