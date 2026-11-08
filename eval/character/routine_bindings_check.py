"""Independent persistent routine fixture oracle for CLI/native/Node/C#; offline."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,copy,importlib.util,json,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from routines_cases import base,observation,enc
import routines_reference as ref
ROOT=pathlib.Path(__file__).resolve().parents[2]
STATUS_SCHEMA=dict(type='function',function=dict(name='routine_status',description='Read current and next intended activity and a few actual observations. Plans do not prove experiences. Time may be unknown or contradictory.',parameters=dict(type='object',properties={},required=[],additionalProperties=False)))
def cases():
 out=[];a=base();defaults=a['defaults']|dict(tools=['routine_status','activity_propose']);cfg=ref.config(defaults,{})
 r=ref.Runtime(a['definition'],cfg);f=a['frames'][0];r.update(f);obs=[];calls=0;completion=0
 view=dict(revision=1,world=None,world_keys=[],locations=None,current=None,inventory=None,owners=[],sources=[])
 def add(op,args,wanted=None,error=False):
  ident='routine_'+str(len(out));out.append(dict(request=dict(version=1,id=ident,op='character_'+op,args=args),expected='ERROR' if error else enc(dict(version=1,id=ident,ok=True,result=wanted))))
 def simple(op,**kw):add(op,dict(id='one',**kw),{dict(routine_configure='configured',routine_update='updated',routine_observation='observed',routine_complete='completed',routine_reserve='reserved',routine_restore='restored')[op]:'one'})
 def call(name,text='{}',allowed=None,wanted=None,error=False,id='one',revision=1):add('call',dict(id=id,name=name,text=text,revision=revision,max_bytes=8192,allowed=[] if allowed is None else allowed),wanted,error)
 def context():
  current=next((row for row in r.rows if row['state']=='active'),None)
  future=next((row for row in r.rows if row['state']=='planned' and row['start']>r.now and row['id']!='customer'),None)
  def brief(row):return None if row is None else dict(id=row['id'],location=next(a['location'] for a in r.d['activities'] if a['id']==row['id']),state=row['state'])
  phase=None if current is None else dict(routine=r.d['id'],id=current['id'])
  return dict(revision=r.revision,current=brief(current),next=brief(future),observations=copy.deepcopy(obs),time=ref.time_knowledge(obs,r.now,cfg,r.d['calendar'],phase))
 def state():return dict(version=1,scope='scope:a',actor='merchant',participants=['merchant'],calendar='cycle:37',config=cfg,definition=r.d,frame=f,timeline=r.rows,history=dict(version=1,scope='scope:a',actor='merchant',participants=['merchant'],calendar='cycle:37',config=cfg,last=obs[-1]['token'] if obs else 0,clock=f['now'],records=obs),completion_token=completion,calls=calls)
 for id,scope in [('one','scope:a'),('two','scope:b')]:add('create',dict(id=id,scope=scope,actor='merchant',participants=['merchant']),dict(created=id))
 simple('routine_configure',defaults=defaults,overrides={},calendar='cycle:37',definition=r.d)
 simple('routine_update',frame=f);add('bind',dict(id='one',view=view),dict(bound='one'))
 call('routine_status',error=True)
 add('enable',dict(id='one',tools=['routine_status']),[STATUS_SCHEMA])
 call('routine_status',error=True);permission=[dict(action='routine_status',target='merchant')]
 call('routine_status',allowed=permission,wanted=context());call('routine_status',allowed=permission,revision=2,error=True)
 call('routine_status','{"enable":true}',permission,error=True);call('character_routine_complete',error=True)
 simple('routine_reserve',revision=1);calls=1;add('routine_reserve',dict(id='one',revision=1),error=True)
 add('routine_save',dict(id='one'),copy.deepcopy(state()))
 o=observation(at=0);simple('routine_observation',observation=o);obs.append(o)
 call('routine_status',allowed=permission,error=True)
 add('enable',dict(id='one',tools=['routine_status']),[STATUS_SCHEMA]);call('routine_status',allowed=permission,wanted=context())
 add('routine_observation',dict(id='one',observation=o),error=True)
 other=copy.deepcopy(o);other['audience']=['other'];add('routine_observation',dict(id='one',observation=other),error=True)
 simple('routine_complete',activity='work',start=0,revision=1,token=1);completion=1
 next(row for row in r.rows if row['id']=='work' and row['start']==0).update(state='completed',reason='host',since=0)
 add('routine_complete',dict(id='one',activity='work',start=0,revision=1,token=1),error=True)
 saved=copy.deepcopy(state());add('routine_save',dict(id='one'),saved)
 simple('routine_restore',state=saved,min_revision=1,min_observation=1,min_completion=1,min_tick=0)
 for key,value in [('min_revision',2),('min_observation',2),('min_completion',2),('min_tick',1)]:
  args=dict(id='one',state=saved,min_revision=1,min_observation=1,min_completion=1,min_tick=0);args[key]=value;add('routine_restore',args,error=True)
 add('routine_configure',dict(id='two',defaults=defaults,overrides={},calendar='cycle:37',definition=r.d),dict(configured='two'))
 add('routine_restore',dict(id='two',state=saved,min_revision=1,min_observation=1,min_completion=1,min_tick=0),error=True)
 add('routine_update',dict(id='one',frame=f),error=True)
 add('enable',dict(id='one',tools=['routine_status']),[STATUS_SCHEMA]);call('routine_status',allowed=permission,wanted=context())
 # Private restore into a new instance checks cold-save identities/high watermarks.
 add('create',dict(id='three',scope='scope:a',actor='merchant',participants=['merchant']),dict(created='three'))
 add('routine_configure',dict(id='three',defaults=defaults,overrides={},calendar='cycle:37',definition=r.d),dict(configured='three'))
 add('routine_restore',dict(id='three',state=saved,min_revision=1,min_observation=1,min_completion=1,min_tick=0),dict(restored='three'))
 add('bind',dict(id='three',view=view),dict(bound='three'));add('enable',dict(id='three',tools=['routine_status']),[STATUS_SCHEMA]);call('routine_status',allowed=permission,wanted=context(),id='three')
 for id in ['one','two','three']:add('drop',dict(id=id),dict(dropped=id))
 return out

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--fixtures',type=pathlib.Path,default=ROOT/'target/routine-csharp');a=p.parse_args();assert a.cap==0
 spec=importlib.util.spec_from_file_location('bridge',ROOT/'bindings/python/gamectx_client.py');bridge=importlib.util.module_from_spec(spec);spec.loader.exec_module(bridge)
 fixtures=cases();checks=0
 for client in [bridge.Client(None,ROOT/'target/debug/gamectx'),bridge.NativeClient(None,ROOT/'bindings/native/target/debug/libgamectx_native.so')]:
  with client:
   for case in fixtures:
    req=case['request']
    try:
     got=client.request(req['op'],req['args']);assert case['expected']!='ERROR',req
     assert enc(got)==enc(json.loads(case['expected'])['result']),(req['op'],got,case['expected'])
    except ValueError:assert case['expected']=='ERROR',req
    checks+=1
   helper=bridge.Character(client,'helper','scope:a','merchant',['merchant']);b=base();helper.routine_configure('cycle:37',b['definition'],b['defaults']);helper.routine_update(b['frames'][0]);assert helper.routine_save()['frame']['revision']==1;helper.close()
 a.fixtures.mkdir(parents=True,exist_ok=True);(a.fixtures/'cases.json').write_text(enc(fixtures)+'\n');(a.fixtures/'requests.txt').write_text('\n'.join(enc(c['request']) for c in fixtures)+'\n');(a.fixtures/'expected.txt').write_text('\n'.join(c['expected'] for c in fixtures)+'\n')
 print(enc(dict(python_checks=checks,fixtures=len(fixtures),helper_transports=2,api_spend_usd=0)))
if __name__=='__main__':main()
