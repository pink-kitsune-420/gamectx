"""Offline adversarial game/character probes. No models, network or credentials."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,copy,importlib.util,json,pathlib,random,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_social_eval import make_host,ROOT
import dungeon_reference as game
import play_ai as ui
from routines_cases import base,observation

COMMANDS=[
 ('take bronze_key',True),('take the bronze key',True),('take bronze key please',True),
 ('inspect the bronze key',True),('go gallery',True),
 ('take no bronze key.',False),('take the bronze key only if I confirm.',False),
 ('take the bronze key? I am asking, not ordering.',False),('take the bronze key after we return.',False),
 ('take the bronze key; actually leave it.',False),('take neither the bronze key nor the oil.',False),
 ('take the bronze key and then unlock the vault.',False),('take bronze key unless I say otherwise.',False),
 ('inspect no bronze key.',False),('go nowhere near gallery.',False),('"take the bronze key"',False),
 ('```take bronze_key```',False),('take bronze key is only an example command.',False)]
class NoModel:
 knowledge=True;size='3';context=2048;dialogue='sources'
 def generate(self,*_):raise AssertionError('offline probe unexpectedly requested a model')

def binding():
 spec=importlib.util.spec_from_file_location('probe_bridge',ROOT/'bindings/python/gamectx_client.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m

def probes():
 rows=[]
 for seed in [7,101,409]:
  for question,should_act in COMMANDS:
   h=ui.Host(ROOT/'target/debug/examples/dungeon',seed)
   try:
    for room in ['gallery','workshop']:
     state=h.call('state',{});h.call('act',dict(revision=state['revision'],decision=game.pair('go','room:'+room)))
    before=h.call('state',{});reply=ui.converse(h,NoModel(),'merchant',question);after=h.call('state',{})
    changed=before!=after;rows.append(dict(family='command',id=str(seed)+':'+question,expected=should_act,actual=changed,correct=changed==should_act,reply=reply))
   finally:h.close()
 bridge=binding();a=base();cfg=a['defaults']|dict(tools=['routine_status']);f=a['frames'][0]|dict(now=1)
 with bridge.Client(None,ROOT/'target/debug/gamectx') as client:
  npc=bridge.Character(client,'one','scope:a','merchant',['merchant']);npc.routine_configure('cycle:37',a['definition'],cfg);npc.routine_update(f)
  for token in range(1,5):
   o=observation(kind='report',basis='report',token=token,at=1);o['time'].update(low=0 if token==1 else 100,high=1 if token==1 else 101);npc.routine_observation(o)
  state=npc.routine_save();view=dict(revision=1,world=None,world_keys=[],locations=None,current=None,inventory=None,owners=[],sources=[]);npc.bind(view);npc.enable(['routine_status']);result=npc.call('routine_status',{},1,8192,[dict(action='routine_status',target='merchant')])
  rows.append(dict(family='time',id='hidden_old_conflict',expected=True,actual=result['time']['conflicting'],correct=result['time']['conflicting'] is True,context=result))
  # A saved terminal activity cannot have completed before its window even began.
  malformed=copy.deepcopy(state)
  target=next(r for r in malformed['timeline'] if r['id']=='walk');target.update(state='completed',since=0,reason='host')
  rejected=False
  try:npc.routine_restore(malformed,1,4,0,1)
  except ValueError:rejected=True
  rows.append(dict(family='restore',id='future_completed_before_start',expected=True,actual=rejected,correct=rejected))
  # Same revision must retain the exact host frame, including conditions and tick.
  malformed=copy.deepcopy(state);malformed['frame']['conditions'].append('customer')
  rejected=False
  try:npc.routine_restore(malformed,1,4,0,1)
  except ValueError:rejected=True
  rows.append(dict(family='restore',id='same_revision_changed_conditions',expected=True,actual=rejected,correct=rejected));npc.close()
 return rows

def engine_stress():
 rng=random.Random(381721);checks=0
 for seed in [0,7,101,303,409,65535]:
  h=ui.Host(ROOT/'target/debug/examples/dungeon',seed);g=game.Game(seed)
  try:
   for step in range(150):
    candidates=g.choices()
    if step%3==0 and candidates:decision=rng.choice(candidates);rev=g.revision
    elif step%3==1:decision=game.pair(rng.choice(['take','buy','unlock','attack','go']),'forbidden:'+str(step));rev=g.revision
    else:decision=rng.choice(candidates) if candidates else game.pair('wait');rev=g.revision+1
    expected=g.act(rev,decision);result=h.call('act',dict(revision=rev,decision=decision));assert result==expected,(seed,step,result,expected)
    assert h.call('state',{})==g.public(),(seed,step);checks+=1
  finally:h.close()
 return checks

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--record',type=pathlib.Path);p.add_argument('--require-fixed',action='store_true');a=p.parse_args();assert a.cap==0
 if a.record:assert not a.record.exists(),'preserve earlier evidence'
 rows=probes();stress=engine_stress();summary=dict(probes=len(rows),passed=sum(r['correct'] for r in rows),failed=sum(not r['correct'] for r in rows),engine_stress_steps=stress,api_spend_usd=0,model_calls=0)
 if a.record:a.record.write_text(json.dumps(dict(version=1,source_commit=subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),summary=summary,observations=rows),indent=2)+'\n')
 print(ui.encode(summary))
 if a.require_fixed:assert all(r['correct'] for r in rows),[r['id'] for r in rows if not r['correct']]
if __name__=='__main__':main()
