"""Independent stage 2 expectations, no generation and no network."""
import argparse,copy,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from continuity_reference import run
ROOT=pathlib.Path(__file__).resolve().parents[1]
def base():
 people=['merchant','player'];scene=dict(id='counter',revision=1,epoch=1,at=1,participants=people,facts=[dict(id='window',revision=1,observed_at=1,source=dict(id='sensor',speaker='merchant',text='Sun is visible through the window.'),kind='perceived',audience=people,text='Sun visible through the window.')],affordances=['serve'])
 thread=dict(id='trade',topic='lamp repairs',questions=['Which wick?'],promises=['repair'],unfinished=['polish'],listener='player',participants=people,shared=['window'],referents=[])
 def op(k,**args):return dict(op=k,args=args)
 return dict(scope='counter:player',actor='merchant',participants=people,operations=[op('scene',**scene),op('thread',**thread),op('generate',thread='trade',text='I can replace that wick.',dependencies=['window'],life_revision=0),op('validate',id=1,life_revision=0),op('delivery',id=1,event='queued',through=0,recipient=None),op('delivery',id=1,event='played',through=8,recipient=None),op('delivery',id=1,event='interrupted',through=8,recipient=None),op('delivery',id=1,event='heard',through=5,recipient='player'),op('inspect')])
def cases():
 out=[('interrupted',base())]
 for i in range(32):
  b=base();s=copy.deepcopy(b['operations'][0]);s['args']['revision']=2;s['args']['at']=2
  if i%4==1:s['args']['epoch']=2
  if i%4==2:s['args']['facts'][0]['revision']=2
  if i%4==3:s['args']['id']='gallery'
  b['operations'].insert(3,s);out.append(('freshness_'+str(i),b))
 for name,fn in [('unheard',lambda b:b['operations'].__setitem__(slice(4,None),[dict(op='inspect',args={})])),('replay',lambda b:b['operations'].insert(4,copy.deepcopy(b['operations'][3]))),('private',lambda b:b['operations'][0]['args']['facts'][0].update(audience=['merchant'])),('global_is_not_perceived',lambda b:b['operations'][0]['args']['facts'][0].update(kind='global')),('overheard',lambda b:b['operations'][0]['args']['facts'][0].update(kind='report')),('missing_dependency',lambda b:b['operations'][2]['args'].update(dependencies=['missing'])),('heard_too_much',lambda b:b['operations'][7]['args'].update(through=10)),('stale_personality',lambda b:b['operations'][3]['args'].update(life_revision=2))]:
  b=base();fn(b);out.append((name,b))
 return out
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,v in cases():
  wanted=run(v);request=dict(kind='continuity',args=v)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=json.dumps(request),capture_output=True,text=True);assert r.returncode==0,(name,r.stderr)
   assert r.stdout.strip()==json.dumps(wanted,ensure_ascii=False,separators=(',',':')),(name,r.stdout,wanted)
  if a.write_conformance and not name.startswith('freshness_'):
   d=ROOT/'conformance/continuity'/name;d.mkdir(parents=True,exist_ok=True)
   for f,o in [('case',dict(op='local')),('input',request),('expected',wanted)]: (d/(f+'.json')).write_text(json.dumps(o,ensure_ascii=False,separators=(',',':'))+'\n')
 print(json.dumps(dict(workflows=len(cases()),model_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
