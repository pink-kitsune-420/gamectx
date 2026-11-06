"""Offline independent routine/time parity and additive conformance cases; no network."""
import argparse,copy,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import routines_reference as ref
ROOT=pathlib.Path(__file__).resolve().parents[1]
def enc(v):return json.dumps(v,ensure_ascii=False,separators=(',',':'))
def activity(id='work',priority=10,**kw):
 return dict(id=id,location='shop',start=0,end=12,repeat=37,priority=priority,requirements=[],interrupt='allow',resume='resume',missed='cancel',grace=0,**{})|kw
def base():
 return dict(definition=dict(version=1,id='merchant',revision=1,calendar='cycle:37',activities=[activity(),activity('customer',20,requirements=['customer']),activity('walk',1,location='court',start=12,end=16,requirements=['route'])]),defaults=dict(routines=True,history=True,time=True,time_sources=['sun','instrument','report','routine']),overrides={},frames=[dict(revision=1,now=0,conditions=['route'],allowed=['work','customer','walk'],emergency=False),dict(revision=2,now=3,conditions=['route','customer'],allowed=['work','customer','walk'],emergency=False),dict(revision=3,now=5,conditions=['route'],allowed=['work','customer','walk'],emergency=False)],observations=[],now=6)
def observation(kind='direct',basis='sun',**kw):
 return dict(token=1,observer='merchant',location='court',at=1,expires=50,kind=kind,source=dict(id='sensor:1',speaker='merchant',text='Sun visible through the window.'),audience=['merchant'],value=dict(visible=True),uncertainty='Host angular estimate.',time=dict(source=basis,calendar='cycle:37',low=3,high=5,rate_low=0,rate_high=2,usable=True))|kw
def cases():
 out=[]
 def add(n,v,error=False):out.append((n,dict(kind='routine-fixture',args=v),error))
 add('interruption_resume',base())
 for name,now,conditions,allowed in [('blocked_travel',13,[],['work','customer','walk']),('missed_windows',18,['route'],['work','customer','walk']),('nonstandard_rollover',38,['route'],['work','customer','walk']),('unavailable_resource',7,[],[])]:
  b=base();b['frames'].append(dict(revision=4,now=now,conditions=conditions,allowed=allowed,emergency=False));b['now']=now;add(name,b)
 b=base();b['observations']=[observation()];add('remembered_sunlight',b)
 b=base();b['observations']=[observation(value=dict(visible=False),time=observation()['time']|dict(usable=False))];add('blocked_or_cloudy',b)
 b=base();b['observations']=[observation(kind='instrument',basis='instrument',time=observation()['time']|dict(source='instrument',usable=False))];add('broken_watch',b)
 b=base();b['observations']=[observation(expires=4)];add('stale_observation',b)
 b=base();o=observation(kind='report',basis='report');b['observations']=[o,o|dict(token=2,source=dict(id='report:2',speaker='player',text='It is much later.'),time=o['time']|dict(low=80,high=90))];add('conflicting_reports',b)
 b=base();b['defaults']['time_sources']=[];add('no_sources',b)
 b=base();b['definition']['activities'][0]['interrupt']='locked';add('locked_activity',b)
 b=base();b['definition']['activities'][0]['interrupt']='locked';b['frames'][1]['emergency']=True;add('emergency',b)
 b=base();b['definition']['activities'][0]['resume']='cancel';add('cancel_on_interrupt',b)
 b=base();b['definition']['activities'][0].update(missed='defer',grace=8);b['frames'].append(dict(revision=4,now=18,conditions=[],allowed=['work'],emergency=False));b['now']=18;add('postponed_work',b)
 for i in range(48):
  b=base();b['frames']=[dict(revision=1,now=i*7,conditions=['route']+(['customer'] if i%2 else []),allowed=['work','customer','walk'],emergency=False)];b['now']=i*7;b['definition']['activities'][0]['priority']=i%21;add('generated_'+str(i),b)
 for name,fn in [('unknown_definition_field',lambda b:b['definition'].update(code='execute()')),('duplicate_activity',lambda b:b['definition']['activities'].append(copy.deepcopy(b['definition']['activities'][0]))),('bad_repeat',lambda b:b['definition']['activities'][0].update(repeat=1)),('timeline_overflow',lambda b:b['overrides'].update(timeline=1)),('backward_tick',lambda b:b['frames'][1].update(now=0)),('unknown_config',lambda b:b['overrides'].update(enable_all=True))]:
  b=base();fn(b);add(name,b,True)
 return out
def expected(a):
 cfg=ref.config(a['defaults'],a['overrides']);r=ref.Runtime(a['definition'],cfg)
 for f in a['frames']:r.update(f)
 phase=next((dict(routine=a['definition']['id'],id=x['id']) for x in r.rows if x['state']=='active'),None)
 return dict(timeline=r.rows,time=ref.time_knowledge(a['observations'],a['now'],cfg,a['definition']['calendar'],phase))
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 accepted=rejected=0
 for name,v,error in cases():
  try:wanted=expected(v['args']);assert not error,name;accepted+=1
  except (AssertionError,KeyError,TypeError,ValueError):
   if not error:raise
   wanted=None;rejected+=1
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(v),capture_output=True,text=True)
   assert bool(r.returncode)==error,(name,r.stderr,r.stdout)
   if not error:assert r.stdout.strip()==enc(wanted),(name,r.stdout,enc(wanted))
  if a.write_conformance and not name.startswith('generated_'):
   d=ROOT/'conformance/routines'/name;d.mkdir(parents=True,exist_ok=True);(d/'case.json').write_text(enc(dict(op='local',**({'error':True} if error else {})))+'\n');(d/'input.json').write_text(json.dumps(v,indent=2)+'\n')
   if not error:(d/'expected.json').write_text(enc(wanted)+'\n')
 report=dict(workflows=accepted+rejected,accepted=accepted,rejected=rejected,api_spend_usd=0,model_calls=0,measurement='deterministic offline contract only')
 print(enc(report))
if __name__=='__main__':main()
