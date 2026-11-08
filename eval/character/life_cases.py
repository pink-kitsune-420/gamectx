"""Stage 1 expectations defined before Rust comparisons. All offline, zero spend."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,copy,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from life_reference import run
ROOT=pathlib.Path(__file__).resolve().parents[2]
def enc(v):return json.dumps(v,ensure_ascii=False,separators=(',',':'))
def event(token=1,at=1,**kw):
 return dict(token=token,at=at,kind='direct',tag='returned',subject='player',source=dict(id='receipt:'+str(token),speaker='merchant',text='The borrowed lamp was returned.'),audience=['merchant','player'],text='A borrowed lamp was returned intact.',causes=[],retain=True)|kw
def base():
 return dict(setting=json.loads((ROOT/'examples/fixtures/characters/setting.json').read_text()),definition=json.loads((ROOT/'examples/fixtures/characters/merchant-life.json').read_text()),defaults=dict(enabled=True,balanced=True),overrides={},scope='merchant:player',actor='merchant',participants=['merchant','player'],operations=[dict(op='experience',args=event()),dict(op='appraise',args=dict(rule='returned_lamp',event=1,revision=1)),dict(op='context',args=dict(tags=['trade'],limit=4)),dict(op='inspect',args={})])
def cases():
 out=[('merchant',base(),False)]
 for i in range(40):
  b=base();b['definition']['attitudes'][0]['level']=i-20;b['definition']['rules'][0]['delta']=1+i%20;out.append(('generated_'+str(i),b,False))
 for name,fn in [('disabled',lambda b:b.update(defaults={})),('report_not_experience',lambda b:b['operations'][0]['args'].update(kind='report')),('foreign_audience',lambda b:b['operations'][0]['args'].update(audience=['merchant'])),('replay',lambda b:b['operations'].insert(1,copy.deepcopy(b['operations'][0]))),('stale',lambda b:b['operations'][1]['args'].update(revision=0)),('private',lambda b:b['definition']['attitudes'][0].update(audience=['merchant'])),('nonhuman',lambda b:b['definition']['facets'][0].update(text='A glass colony that communicates through chimes.'))]:
  b=base();fn(b);out.append((name,b,False))
 for name,fn in [('future_history',lambda b:b['definition'].update(history=['future_player_choice'])),('unknown_field',lambda b:b['definition'].update(execute=True)),('abrupt_delta',lambda b:b['definition']['rules'][0].update(delta=100)),('unknown_option',lambda b:b['overrides'].update(autoapprove=True))]:
  b=base();fn(b);out.append((name,b,True))
 return out
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 for name,v,error in cases():
  try:wanted=run(v);assert not error,name
  except (AssertionError,KeyError,ValueError,TypeError):
   if not error:raise
   wanted=None
  request=dict(kind='life',args=v)
  if not a.prototype_only:
   r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(request),capture_output=True,text=True)
   assert bool(r.returncode)==error,(name,r.stderr)
   if not error:assert r.stdout.strip()==enc(wanted),(name,r.stdout,enc(wanted))
  if a.write_conformance and not name.startswith('generated_'):
   d=ROOT/'conformance/life'/name;d.mkdir(parents=True,exist_ok=True)
   (d/'case.json').write_text(enc(dict(op='local',**({'error':True} if error else {})))+'\n');(d/'input.json').write_text(json.dumps(request,indent=2)+'\n')
   if not error:(d/'expected.json').write_text(enc(wanted)+'\n')
 print(enc(dict(workflows=len(cases()),model_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
