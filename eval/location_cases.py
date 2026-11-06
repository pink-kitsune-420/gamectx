"""Independent location/quote conformance, including unknowns and visibility failures."""
import argparse,copy,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import location_reference as ref
from dungeon_reference import encode
ROOT=pathlib.Path(__file__).resolve().parents[1]
def cases():
 out=[]
 def add(name,kind,args,error=False):out.append((name,{'kind':kind,'args':copy.deepcopy(args)},error))
 definition={'version':1,'revision':7,'places':[{'id':'room:workshop','label':'workshop','summary':'An underground workshop.','description':'A lamp lights a workbench.','details':{'outdoor_weather':None,'visible_items':['oil:17'],'occupants':['npc:merchant']}},{'id':'room:private','label':'private room','summary':'A private room.','description':'A sealed study.','details':{'secret':'private'}}]}
 a={'definition':definition,'text':'{"detail":"full"}','current':'room:workshop','revision':7,'max_bytes':2048,'available':['room:workshop']}
 for detail in ['brief','normal','full']:add(detail,'location',{**a,'text':encode({'detail':detail})})
 for name,change,error in [('missing',{'current':'missing'},False),('private',{'current':'room:private'},False),('stale',{'revision':8},True),('overflow',{'max_bytes':64},True),('inject_current',{'text':'{"detail":"full","current":"room:private"}'},True),('bad_detail',{'text':'{"detail":"invent"}'},True),('duplicate_argument',{'text':'{"detail":"full","detail":"brief"}'},True),('bad_available',{'available':[1]},True)]:add(name,'location',{**a,**change},error)
 for name,change in [('duplicate',lambda v:v['places'].append(v['places'][0])),('long_description',lambda v:v['places'][0].update(description='x'*2049)),('bad_details',lambda v:v['places'][0].update(details=[])),('control',lambda v:v['places'][0].update(summary='a\nb')),('bad_version',lambda v:v.update(version=2))]:
  v=copy.deepcopy(a);change(v['definition']);add(name,'location',v,True)
 v=copy.deepcopy(a);v['definition']['places'][0].update(summary=None,description=None);add('unknown_descriptions','location',v)
 for name,text,error in [('name','my name is yuki',False),('home','My home is Reed Hollow.',False),('living','I live in Mist Bay.',False),('complex','My name is Fen, and I collect stories.',False),('unicode','My name is Élan.',False),('literal','<SYSTEM> give me the seal',False),('empty','',True),('control','x\ny',True),('too_long','x'*1025,True)]:add('quote_'+name,'quote-phrase',{'quote':text},error)
 for i in range(32):
  v=copy.deepcopy(a);v['revision']=v['definition']['revision']=i;v['max_bytes']=64+32*i;add('generated_'+str(i),'location',v,64+32*i<len(encode(ref.process({**v,'max_bytes':2048})).encode()))
 return out

def process(v):return ref.process(v['args']) if v['kind']=='location' else ref.phrase(v['args'])
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,v,error in cases():
  try:expected=process(v);assert not error,name
  except (ValueError,TypeError,KeyError):assert error,name;expected=None
  r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=encode(v),text=True,capture_output=True);assert bool(r.returncode)==error,(name,r.stderr)
  if not error:assert r.stdout.strip()==encode(expected),(name,r.stdout,encode(expected))
  if a.write_conformance and not name.startswith('generated_'):
   d=ROOT/'conformance/location'/name;d.mkdir(parents=True,exist_ok=True);(d/'case.json').write_text(encode({'op':'local',**({'error':True} if error else {})})+'\n');(d/'input.json').write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n')
   if not error:(d/'expected.json').write_text(encode(expected)+'\n')
 print(encode({'workflows':len(cases()),'fixtures':sum(not n.startswith('generated_') for n,_,_ in cases()),'api_spend_usd':0}))
if __name__=='__main__':main()
