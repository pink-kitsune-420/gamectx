"""Independent read-only time/weather fixtures and exact Rust byte comparisons."""
import argparse,copy,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import world_reference as ref
from dungeon_reference import encode
ROOT=pathlib.Path(__file__).resolve().parents[1]
def base():
 return {'definition':{'version':1,'id':'world:test','revision':7,'states':[{'key':'time','value':{'tick':42,'label':'third bell','calendar':{'watch':3}}},{'key':'weather','value':None},{'key':'secret','value':'hidden storm schedule'}]},'text':'{"keys":["time","weather"]}','revision':7,'max_bytes':1024,'available':['time','weather']}
def cases():
 out=[]
 def add(name,a,error=False):out.append((name,{'kind':'world','args':a},error))
 add('clock_and_unknown_weather',base())
 for name,changes,error in [('private',{'text':'{"keys":["secret"]}'},False),('missing',{'text':'{"keys":["missing"]}'},False),('stale',{'revision':8},True),('overflow',{'max_bytes':64},True),('extra',{'text':'{"keys":["time"],"revision":8}'},True),('duplicate_key',{'text':'{"keys":["time","time"]}'},True),('duplicate_argument',{'text':'{"keys":["time"],"keys":["weather"]}'},True),('empty',{'text':'{"keys":[]}'},True),('too_many',{'text':encode({'keys':['k'+str(i) for i in range(9)]})},True),('bad_available',{'available':[False]},True)]:
  add(name,{**base(),**changes},error)
 for name,change in [('duplicate_state',lambda v:v['states'].append(v['states'][0])),('long_state',lambda v:v['states'][0].update(value='x'*4097)),('bad_version',lambda v:v.update(version=2)),('bad_revision',lambda v:v.update(revision=True))]:
  a=base();change(a['definition']);add(name,a,True)
 for i in range(48):
  a=base();a['revision']=a['definition']['revision']=i;a['text']=encode({'keys':[['time'],['weather'],['time','weather','secret','absent']][i%3]});a['available']=['time','weather'] if i%2 else [];add('generated_'+str(i),a)
 return out
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,v,error in cases():
  try:wanted=ref.process(v['args']);assert not error,name
  except (ValueError,TypeError,KeyError):assert error,name;wanted=None
  r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=encode(v),text=True,capture_output=True);assert bool(r.returncode)==error,(name,r.stderr)
  if not error:assert r.stdout.strip()==encode(wanted),(name,r.stdout,encode(wanted))
  if a.write_conformance and not name.startswith('generated_'):
   d=ROOT/'conformance/world'/name;d.mkdir(parents=True,exist_ok=True);(d/'case.json').write_text(encode({'op':'local',**({'error':True} if error else {})})+'\n');(d/'input.json').write_text(json.dumps(v,indent=2)+'\n')
   if not error:(d/'expected.json').write_text(encode(wanted)+'\n')
 print(encode({'workflows':len(cases()),'fixtures':sum(not n.startswith('generated_') for n,_,_ in cases()),'api_spend_usd':0}))
if __name__=='__main__':main()
