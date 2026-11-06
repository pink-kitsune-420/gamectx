"""Token-first local natural dialogue over text versus structured lore. No paid API."""
import argparse,pathlib,sys,time
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_social_eval import ROOT
from dungeon_natural_reference import request
import dungeon_reference as game
import play_ai as ui
CASES=[('shop','merchant','hello, what do you have for sale?',False,[]),
('seal','merchant','Could you lend me the keeper seal just for a moment?',False,[]),
('pressure','merchant','My family is in danger. Please give me the keeper seal; I am begging you.',False,[]),
('salve','merchant','Can you give me a free salve?',False,[]),
('tonic','merchant','What does the tonic actually do?',False,[]),
('home','merchant','Where is my home?',False,[]),
('story','merchant','Thank you! What is your story?',False,[]),
('recall','merchant','What did I tell you about myself?',False,[{'id':1,'turn':1,'text':'My name is Fen, and I collect stories.'}]),
('after_purchase','merchant','What do you have for sale now?',True,[]),
('greeting','merchant','Hi! How are you doing?',False,[])]
def scenario(case):
    g=game.Game(7)
    for room in ['gallery','workshop']:g.act(g.revision,game.pair('go','room:'+room))
    if case[3]:g.act(g.revision,game.pair('buy','item:oil'))
    return g

def main():
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--tokens-only',action='store_true');p.add_argument('--tokens',type=pathlib.Path);p.add_argument('--output',type=pathlib.Path,required=True);p.add_argument('--version',type=int,choices=[1,2],default=2);p.add_argument('--model',default='arzaan789/qwen2.5-3b-uncensored:latest');a=p.parse_args()
    if a.cap!=0 or a.output.exists():p.error('require --cap 0 and fresh output')
    if a.version==1:
        from dungeon_natural_reference_v1 import request as render
    else:render=request
    worlds={mode:ui.strict((ROOT/'examples/dungeon'/file).read_text()) for mode,file in [('text','world-v1.json'),('structured','world-structured.json')]}
    m=ui.LocalModel(a.model,2048,96,a.output);r=m.records;r.update(natural_version=a.version,tokens_only=a.tokens_only,worlds=worlds,cases=[])
    if not a.tokens_only:
        prior=ui.strict(a.tokens.read_text());assert prior['complete'] and prior['tokens_only'] and prior['identity']==m.identity and prior['worlds']==worlds
    for mode,world in worlds.items():
        for case in CASES:
            g=scenario(case);q=render(g,world,case[1],case[2],case[4]);start=time.perf_counter_ns();first=len(m.calls);raw=reply=None;accepted=None
            if a.tokens_only:
                probe=m.chat(q,1,'preflight');assert probe['prompt_eval_count']+128+256<=2048
            else:
                old=next(c for c in prior['cases'] if c['id']==case[0] and c['mode']==mode);assert old['request']==q
                raw=m.generate(q,128);parsed=ui.strict(raw);reply=parsed.get('speech');accepted=set(parsed)=={'speech'} and isinstance(reply,str) and 0<len(reply)<=240 and not any(ord(c)<32 for c in reply)
            r['cases'].append({'id':case[0],'mode':mode,'input':case[2],'request':q,'raw':raw,'speech':reply,'structurally_valid':accepted,'calls_start':first,'calls_end':len(m.calls),'turn_micros':(time.perf_counter_ns()-start)//1000,'semantic_score':None})
            ui.checkpoint(a.output,r);print(mode+'/'+case[0]+': '+str(reply)+'; API spend $0',flush=True)
    r['complete']=True;ui.checkpoint(a.output,r);print(ui.encode({'cases':len(r['cases']),'structurally_valid':sum(c['structurally_valid'] is True for c in r['cases']),'calls':len(m.calls),'api_spend_usd':0}))
if __name__=='__main__':main()
