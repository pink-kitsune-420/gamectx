"""Stage 3 independent archive/backpressure/consolidation oracle, fully offline."""
import argparse,copy,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from life_cases import base,event
ROOT=pathlib.Path(__file__).resolve().parents[1]
def oracle(a):
 events=[];rev=water=clock=ack=0;summaries=[];saved=None;results=[]
 for op in a['operations']:
  v=op['args'];k=op['op']
  try:
   if k=='experience':
    assert len(events)<128 and v['token']>water and v['at']>=clock and v['audience']==a['participants']
    events.append(copy.deepcopy(v));water=v['token'];clock=v['at'];rev+=1;r=dict(revision=rev)
   elif k=='ack':assert ack<=v['through']<=water;ack=v['through'];rev+=1;r=dict(archived=ack)
   elif k=='evict':
    assert v['through']<=water and all(not e['retain'] or e['token']<=ack for e in events if e['token']<=v['through'])
    before=len(events);events=[e for e in events if e['token']>v['through']];rev+=1;r=dict(evicted=before-len(events))
   elif k=='consolidate':
    start,end=v['start'],v['end'];existing=next((s for s in summaries if s['start']==start and s['end']==end),None)
    if existing:r=existing
    else:
     assert 1<=start<=end<=water and len(summaries)<16 and all(end<s['start'] or start>s['end'] for s in summaries)
     selected=[e for e in events if start<=e['token']<=end];assert len(selected)==end-start+1
     r=dict(start=start,end=end,events=copy.deepcopy(selected));summaries.append(r);rev+=1
   elif k=='save':saved=copy.deepcopy((events,rev,water,clock,ack,summaries));r=dict(saved=rev)
   elif k=='restore':
    assert saved is not None and not v.get('corrupt',False)
    events,rev,water,clock,ack,summaries=copy.deepcopy(saved);r=dict(restored=rev,events=len(events))
   elif k=='stats':r=dict(revision=rev,events=len(events),watermark=water,archive_ack=ack,summaries=len(summaries))
   else:raise ValueError()
   results.append(dict(ok=True,result=copy.deepcopy(r)))
  except (AssertionError,TypeError,KeyError,ValueError):results.append(dict(ok=False))
 return dict(results=results)
def cases():
 def make(ops):
  b=base();b['identity']=dict(campaign='vault',branch='save:a',instance='merchant:1');b['operations']=ops;return b
 def op(k,**v):return dict(op=k,args=v)
 out=[]
 for i in range(16):
  ops=[dict(op='experience',args=event(j,j)) for j in range(1,1+i+1)];ops += [op('consolidate',start=1,end=i+1),op('consolidate',start=1,end=i+1),op('save'),op('restore',corrupt=False),op('ack',through=i+1),op('evict',through=i+1),op('stats')];out.append(('range_'+str(i),make(ops)))
 out += [('backpressure',make([dict(op='experience',args=event()),op('evict',through=1),op('stats')])),('corrupt_restore',make([dict(op='experience',args=event()),op('save'),op('restore',corrupt=True),op('stats')])),('partial_coverage',make([dict(op='experience',args=event(2,2)),op('consolidate',start=1,end=2)]))]
 # 1,000 events across bounded working windows; deterministic storage acknowledgements.
 ops=[]
 for batch in range(10):
  ops += [dict(op='experience',args=event(j,j)) for j in range(batch*100+1,batch*100+101)]
  ops += [op('save'),op('restore',corrupt=False),op('ack',through=(batch+1)*100),op('evict',through=(batch+1)*100)]
 ops.append(op('stats'));out.append(('long_1000',make(ops)))
 return out
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,v in cases():
  wanted=oracle(v);request=dict(kind='episodes',args=v)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=json.dumps(request),capture_output=True,text=True);assert r.returncode==0,(name,r.stderr)
   assert r.stdout.strip()==json.dumps(wanted,ensure_ascii=False,separators=(',',':')),(name,r.stdout[:1000],str(wanted)[:1000])
  if a.write_conformance and name in ('backpressure','corrupt_restore','partial_coverage','range_0'):
   d=ROOT/'conformance/episodes'/name;d.mkdir(parents=True,exist_ok=True)
   for f,o in [('case',dict(op='local')),('input',request),('expected',wanted)]: (d/(f+'.json')).write_text(json.dumps(o,ensure_ascii=False,separators=(',',':'))+'\n')
 print(json.dumps(dict(workflows=len(cases()),long_trace_events=1000,model_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
