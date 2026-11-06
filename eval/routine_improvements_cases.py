import argparse,copy,json,pathlib,statistics,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import routines_reference as old
import routine_improvements_reference as ref
from routines_cases import base,observation,enc
ROOT=pathlib.Path(__file__).resolve().parents[1]
def cases():
 out=[]
 for i in range(48):
  a=base();a['frames']=[dict(revision=1,now=i,conditions=['route']+(['customer'] if i%2 else []),allowed=['work','walk','customer'],emergency=False)];a['now']=i
  obs=[]
  for j in range(7):
   o=observation(token=j+1,at=0,expires=100,location='court' if j%2 else 'shop')
   if j>0:o.update(kind='report',source=dict(id='report:'+str(j),speaker='player',text='Unrelated stock tally. '*20),value=dict(count=j),time=None)
   obs.append(o)
  a['observations']=obs
  selection=dict(kinds=[],locations=[],time_sources=[],required=[],limit=i%4)
  if i%3==0:selection.update(time_sources=['sun'],required=[1],limit=1)
  elif i%3==1:selection.update(locations=['court'])
  else:selection.update(kinds=['report'])
  a.update(selection=selection,changed_conditions=['customer'] if i%2 else ['distant'],changed_activities=[])
  out.append(('select_'+str(i),dict(kind='routine-detail',args=a),False))
 for name,mutate in [('missing_required',lambda a:a['selection'].update(required=[99])),('filtered_required',lambda a:a['selection'].update(locations=['elsewhere'])),('over_limit',lambda a:a['selection'].update(limit=33)),('duplicate_required',lambda a:a['selection'].update(required=[1,1],limit=2)),('expired_required',lambda a:a['observations'][0].update(expires=1)),('unknown_selector',lambda a:a['selection'].update(guess=True))]:
  a=copy.deepcopy(out[0][1]['args']);mutate(a)
  if name=='expired_required':a['now']=6
  out.append((name,dict(kind='routine-detail',args=a),True))
 return out
def expected(a):
 cfg=old.config(a['defaults'],a['overrides']);r=old.Runtime(a['definition'],cfg)
 for f in a['frames']:r.update(f)
 selected=ref.select(a['observations'],a['now'],cfg,a['selection']);phase=next((dict(routine=r.d['id'],id=e['id']) for e in r.rows if e['state']=='active'),None)
 return dict(observations=selected,time=old.time_knowledge(selected,a['now'],cfg,r.d['calendar'],phase),next_wake=ref.next_wake(r,a['observations']),affected=ref.affected(r,a['changed_conditions'],a['changed_activities']))
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');args=p.parse_args();assert args.cap==0
 for name,v,error in cases():
  try:wanted=expected(v['args']);assert not error,name
  except AssertionError:
   if not error:raise
   wanted=None
  if not args.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(v),capture_output=True,text=True);assert bool(r.returncode)==error,(name,r.stderr)
   if not error:assert r.stdout.strip()==enc(wanted),(name,r.stdout,enc(wanted))
  if args.write_conformance and (not name.startswith('select_') or int(name[7:])<3):
   d=ROOT/'conformance/routine-detail'/name;d.mkdir(parents=True,exist_ok=True);(d/'case.json').write_text(enc(dict(op='local',**({'error':True} if error else {})))+'\n');(d/'input.json').write_text(json.dumps(v,indent=2)+'\n')
   if not error:(d/'expected.json').write_text(enc(wanted)+'\n')
 a=cases()[0][1]['args'];recent=a['observations'][-3:];chosen=expected(a)['observations']
 performance=json.loads((ROOT/'eval/routine-performance.json').read_text());assert len(performance['pairs'])==7 and performance['model_calls']==0 and performance['api_spend_usd']==0
 assert [r['pair'] for r in performance['pairs']]==list(range(7))
 for key in ['snapshot_clone_ns','preflight_ns']:assert all(type(r[key]) is int and r[key]>0 for r in performance['pairs'])
 assert performance['median_snapshot_clone_ns']==statistics.median(r['snapshot_clone_ns'] for r in performance['pairs'])
 assert performance['median_preflight_ns']==statistics.median(r['preflight_ns'] for r in performance['pairs'])
 report=dict(workflows=len(cases()),accepted=48,rejected=6,recent_observation_bytes=len(enc(recent).encode()),selected_observation_bytes=len(enc(chosen).encode()),required_token_in_recent=any(o['token']==1 for o in recent),required_token_in_selected=any(o['token']==1 for o in chosen),api_spend_usd=0)
 print(enc(report))
if __name__=='__main__':main()
