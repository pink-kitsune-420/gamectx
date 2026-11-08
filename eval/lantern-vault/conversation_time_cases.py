"""Time knowledge depends on authored access and attributed reports, not global clock access."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,copy,json,pathlib,sys,tempfile
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_knowledge_cases import host
from dungeon_knowledge_reference import build
from dungeon_reference import Game,pair,encode
from test_dungeon_ai import Replies
import play_ai as ui
ROOT=pathlib.Path(__file__).resolve().parents[2]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);a=p.parse_args();assert a.cap==0;count=0
 for file in ['world.json','world-structured.json']:
  source=ui.strict((ROOT/'examples/lantern-vault'/file).read_text())
  for speaker,mode in [('merchant','none'),('keeper','schedule'),('merchant','clock'),('merchant','watch'),('merchant','schedule'),('merchant','hidden_clock')]:
   world=copy.deepcopy(source)
   if mode not in ['none'] and speaker=='merchant':
    e=copy.deepcopy(next(e for e in world['entries'] if e['id']=='keeper_timekeeping'));e.update(id='merchant_timekeeping',subject='npc:merchant',known_by=['npc:keeper'] if mode=='hidden_clock' else ['npc:merchant'])
    if world['version']==1:e['text']='clock' if mode=='hidden_clock' else mode
    else:e['value']={'mode':'clock' if mode=='hidden_clock' else mode}
    world['entries'].append(e)
   with tempfile.TemporaryDirectory() as d:
    path=pathlib.Path(d)/'world.json';path.write_text(encode(world));h=host(('time',7,speaker,'',[],False,None),path)
    try:
     g=Game(7)
     for r in ['gallery','workshop' if speaker=='merchant' else 'cistern']:g.act(g.revision,pair('go','room:'+r))
     actual=h.call('knowledge_prepare',{'speaker':speaker,'input':'What time is it?','context':2048,'size':'3'})['offer'];expected=build(g,world,speaker,'What time is it?')
     assert encode(actual)==encode(expected),(mode,actual,expected)
     reply=ui.converse(h,Replies(),speaker,'What time is it?')['speech']
     if mode in ['clock','watch']:assert '08:20' in reply and mode in reply
     elif mode=='schedule':assert 'morning' in reply and '08:20' not in reply
     else:assert 'clock to check' in reply and '08:20' not in reply
     count+=1
    finally:h.close()
 # A player's report is retained with attribution and a freshness caveat, never clock authority.
 memory=[{'id':1,'text':'My watch says noon.'}]
 h=host(('report',7,'merchant','',memory,False,None))
 try:
  reply=ui.converse(h,Replies(),'merchant','What time is it?')
  assert 'My watch says noon.' in reply['speech'] and "still current" in reply['speech'] and '08:20' not in reply['speech']
  assert reply['sources']==[{'kind':'quote','id':'1'}],reply
  h.call('speech_forget',{'speaker':'merchant'})
  reply=ui.converse(h,Replies(),'merchant','What time is it?');assert 'clock to check' in reply['speech']
  count+=2
 finally:h.close()
 print(encode({'time_knowledge_workflows':count,'api_spend_usd':0}))
if __name__=='__main__':main()
