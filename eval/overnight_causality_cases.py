"""Fresh deterministic betrayal/neutral/forgiveness and cyclic-rumor stress trace."""
import argparse,copy,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from living_cases import town,observe,trace
from living_reference import world_workflow
ROOT=pathlib.Path(__file__).resolve().parents[1]
def enc(x):return json.dumps(x,ensure_ascii=False,separators=(',',':'))
def case():
 p=town();merchant=next(c for c in p['characters'] if c['id']=='merchant')
 template=merchant['definition']['rules'][0]
 for id,tag,delta in [('betray','broken',-20),('forgive','repaired',10)]:merchant['definition']['rules'].append(dict(template,id=id,event=tag,delta=delta,cooldown=1))
 ops=[]
 for n in range(1,7):
  e=observe();e.update(id='arc_'+str(n),appraise='betray' if n<3 else 'forgive',share_with=['keeper','companion']);e['event'].update(token=n,at=n*5,tag='broken' if n<3 else 'repaired',kind='outcome');e['event']['source']['speaker']='player_1';ops.append(('observe',e))
  for delay,actors in [(1,['merchant','keeper']),(2,['keeper','companion']),(3,['merchant','keeper','companion'])]:ops.append(('tick',dict(now=n*5+delay,windows=[dict(id='arc_window',location='tavern',start=n*5,end=n*5+4,participants=actors,kind='scene')],routines=[])))
  ops.append(('inspect',dict(actor='merchant')))
 for now in range(40,140):ops.append(('tick',dict(now=now,windows=[dict(id='noise_window',location='tavern',start=40,end=140,participants=['merchant','keeper','companion'],kind='routine')],routines=[dict(actor='guard',frame=dict(revision=now,now=now,conditions=[],allowed=['work'],emergency=now%10==0))])))
 for actor in ['keeper','companion']:ops.append(('inspect',dict(actor=actor)))
 return dict(kind='living-world',args=trace(ops,p))
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--prototype-only',action='store_true');p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
 req=case();expected=world_workflow(req['args']);rows=expected['results'];assert all(x['ok'] for x in rows)
 def level(inspection,player):return next(a['level'] for a in inspection['effective_attitudes'] if a['definition']['subject']==player and a['definition']['kind']=='trust')
 arcs=[level(rows[n*5+4]['result'],'player_1') for n in range(6)];assert arcs==[0,-20,-10,0,10,20],arcs
 assert all(level(rows[n*5+4]['result'],'player_2')==0 for n in range(6))
 reports=[r for x in rows if 'reports' in x.get('result',{}) for r in x['result']['reports']]
 assert len(reports)==12 and max(x['hops'] for x in reports)==2
 assert all(x['event']['kind']=='report' and x['original_source']['speaker']=='player_1' for x in reports)
 assert all(not x['result']['reports'] for x in rows[30:130])
 assert all(level(x['result'],'player_1')==21 and level(x['result'],'player_2')==0 for x in rows[-2:])
 if not a.prototype_only:
  cp=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=enc(req),text=True,capture_output=True);assert cp.returncode==0,cp.stderr;assert cp.stdout.strip()==enc(expected),'Rust/Python causality drift'
 if a.write_conformance:
  d=ROOT/'conformance/deep-causality/forgiveness_and_rumor_cycles';d.mkdir(parents=True,exist_ok=True)
  for f,v in [('case',dict(op='local')),('input',req),('expected',expected)]: (d/(f+'.json')).write_text(enc(v)+'\n')
 print(enc(dict(operations=len(rows),arc=arcs,propagated_reports=len(reports),noise_ticks=100,emergency_frames=10,player_2_delta=0,api_spend_usd=0,model_calls=0)))
if __name__=='__main__':main()
