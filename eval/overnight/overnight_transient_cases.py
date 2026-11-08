"""Additive transient world workflow oracle and binding parity, zero network."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,copy,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from living_cases import town,observe,trace
from overnight_transient_reference import trim
ROOT=pathlib.Path(__file__).resolve().parents[2]
def enc(v):return json.dumps(v,ensure_ascii=False,separators=(',',':'))
def cases():
 out=[]
 for name,count in [('one',1),('capacity',132),('invalid_retention',0)]:
  operations=[];active=[];rev=life_rev=0;results=[]
  for n in range(1,count+1):
   e=copy.deepcopy(observe()['event']);e.update(token=n,at=n,retain=False,tag='conversation',text='Confirmed transient counter dialogue.')
   operations.append(('record',dict(actor='merchant',event=e)))
   if len(active)>=128:active=trim(active,64);life_rev+=1
   active.append(e);life_rev+=1;rev+=1;results.append(dict(ok=True,result=dict(revision=rev,life_revision=life_rev)))
  if not count:
   operations.append(('record',dict(actor='merchant',event=observe()['event'])));results.append(dict(ok=False))
  out.append((name,dict(kind='living-world',args=trace(operations)),dict(results=results,revision=rev,model_calls=0)))
 return out
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,req,want in cases():
  if not a.prototype_only:
   cp=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(req),capture_output=True,text=True);assert cp.returncode==0,(name,cp.stderr);assert cp.stdout.strip()==enc(want),(name,cp.stdout[:1000])
  if a.write_conformance:
   d=ROOT/'conformance/deep-transient'/name;d.mkdir(parents=True,exist_ok=True)
   for f,v in [('case',dict(op='local')),('input',req),('expected',want)]: (d/(f+'.json')).write_text(enc(v)+'\n')
 print(enc(dict(workflows=len(cases()),api_spend_usd=0,model_calls=0)))
if __name__=='__main__':main()
