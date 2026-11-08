"""Exact byte/selection measurements of optional conversation tools. No model calls."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from interaction_cases import ROOT,op,call
from interaction_reference import process
from dungeon_reference import encode

def main():
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--output',type=pathlib.Path);p.add_argument('--check',type=pathlib.Path);a=p.parse_args();assert a.cap==0
    events=[{'id':'event:'+str(i),'turn':i,'subject':'item:oil','kind':'ownership','value':{'owner':'player:'+('a' if i%2 else 'b'),'count':1},'audience':['npc:merchant','player:a'] if i%3 else ['player:a']} for i in range(120)]
    v={'scope':'scene:shop','actor':'npc:merchant','participants':['npc:merchant','player:a','player:b'],'operations':[op('event',e) for e in events]+[call('events_recent',{'since':0,'subject':None,'limit':2},revision=120)]}
    r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=encode({'kind':'interactions','args':v}),capture_output=True,text=True,check=True);expected=process(v);assert r.stdout.strip()==encode(expected)
    selected=expected['results'][-1]['result'];authorized=[{k:e[k] for k in ['id','turn','subject','kind','value']} for e in events if 'npc:merchant' in e['audience']]
    demo=subprocess.run([str(ROOT/'target/debug/examples/conversation_tools')],capture_output=True,text=True,check=True);schemas=json.loads(demo.stdout.splitlines()[-1])
    report={'events':120,'authorized':len(authorized),'selected':len(selected['records']),'all_authorized_bytes':len(encode({'records':authorized}).encode()),'selected_bytes':len(encode(selected).encode()),'all_schema_bytes':len(encode(schemas).encode()),'schema_bytes':{t['function']['name']:len(encode(t).encode()) for t in schemas},'api_spend_usd':0}
    if a.check:assert json.loads(a.check.read_text())==report
    if a.output:a.output.write_text(json.dumps(report,indent=2)+'\n')
    print(encode(report))
if __name__=='__main__':main()
