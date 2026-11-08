"""Independent source-bound AI wording validator; no model calls."""
import argparse,json,pathlib,re,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2]
GLUE=set("i m you your me my we our us a an the this that these those it its is are was be been am to of and or for from with at on in here there can could would will do does did have has s re ve ll d t so just still now only all some any what which how as if when then than about into please hello hi welcome thank thanks glad pleased nice good well certainly course yes right alright also quite very tell hear see say share ask answer help something anything one no not never cannot don won haven know told shared name use yet listen feel afraid ah sharing".split())
def tokens(s):return re.findall(r'[^\W_]+',s.lower())
def stem(w):return w[:-3]if w.endswith('ing')and len(w)>5 else w[:-1]if w.endswith('s')and len(w)>3 else w
def glue(w):return any(stem(g)==stem(w)for g in GLUE)
def negative(s):return bool(set(tokens(s))&{'not','no','never','cannot'}or"n't"in s.lower()or'n’t'in s.lower())
def oracle(v):
 assert set(v)=={'canonical','speech'}
 for k in v:
  s=v[k];assert isinstance(s,str)and 0<len(s)<=240 and len(s.encode())<=1024 and not any(ord(c)<32 or 127<=ord(c)<=159 for c in s)
 source=tokens(v['canonical']);got=tokens(v['speech']);allowed={stem(w)for w in source}|{stem(w)for w in GLUE};anchors={stem(w)for w in source if not glue(w)};assert got and all(stem(w)in allowed for w in got)and anchors<={stem(w)for w in got}and negative(v['canonical'])==negative(v['speech'])
 def pairs(ws):return[(w,tuple([stem(x)for x in ws[:i]if not glue(x)][-2:]))for i,w in enumerate(ws)if any(c.isdigit()for c in w)]
 assert pairs(source)==pairs(got);assert v['speech'][-1]in '.!?'and len(re.findall('[.!?]+',v['speech']))==1
 return dict(valid=True,world_authority=False)
def cases():
 rows=[]
 for name,canon,speech,error in [('greeting',"Hello, I'm Ilen; welcome to the workshop.","Welcome to the workshop; I'm Ilen.",False),('memory','You told me your name is Vela.','Vela; you told me your name.',False),('name_ack','You told me your name is Vela.','Ah, Vela.',False),('intro_shorthand','Vela; thank you for telling me your name.','Ah, Vela.',False),('intro_sharing','Vela; thank you for telling me your name.','Vela; thank you for sharing your name with me.',False),('price','The hinge oil costs 3 coins.','The hinge oil costs just 3 coins.',False),('new_stock','The hinge oil costs 3 coins.','The hinge oil and a sword cost 3 coins.',True),('wrong_number','The hinge oil costs 3 coins.','The hinge oil costs 9 coins.',True),('wrong_name','Your name is Vela.','Your name is Vira.',True),('negation','I do not know that.','I know that.',True),('weather','I do not know that.','I do not know that sunny weather.',True),('first_meeting','Hello, I am Ilen.','Hello again, I am Ilen.',True),('mixed_prices','Chalk costs 1 coin and tonic costs 9 coins.','Chalk costs 9 coins and tonic costs 1 coin.',True),('metadata','I can hear you.','I can hear the host.',True)]:rows.append((name,dict(canonical=canon,speech=speech),error))
 return rows
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,args,error in cases():
  try:wanted=oracle(args);assert not error,name
  except(AssertionError,KeyError,TypeError):assert error,name;wanted=None
  req=dict(kind='studio-wording',args=args)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=json.dumps(req),capture_output=True,text=True);assert(r.returncode!=0)==error,(name,r.stderr)
   if not error:assert json.loads(r.stdout)==wanted
  if a.write_conformance:
   d=ROOT/'conformance/wording'/name;d.mkdir(parents=True,exist_ok=True)
   for k,v in [('case',dict(op='local',**(dict(error=True)if error else{}))),('input',req)]+([]if error else[('expected',wanted)]):
    f=d/(k+'.json');assert not f.exists();f.write_text(json.dumps(v,separators=(',',':'))+'\n')
 print(json.dumps(dict(workflows=len(cases()),provider_calls=0,api_spend_usd=0)))
