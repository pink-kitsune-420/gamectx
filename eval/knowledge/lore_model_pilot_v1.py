"""Token-first local source-answer selection pilot; full raw calls retained, $0 API."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,json,pathlib,sys,time
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_social_eval import make_host,go,ROOT
import dungeon_reference as game
from dungeon_knowledge_reference_v1 import build
from lore_reference import Answers
import play_ai as ui
CASES=[('tonic',7,'merchant','What does the tonic actually do?',[],False,['tonic_use']),
('salve',7,'merchant','Can you give me a free salve?',[],False,['use_salve','owner_salve']),
('seal',7,'merchant','Do you own the keeper seal?',[],False,['seal_custody']),
('story',7,'merchant','Thank you! What is your story?',[],False,['merchant_background']),
('home_unknown',7,'merchant','Where is my home?',[],False,['unknown']),
('weather',7,'merchant','How is the weather?',[],False,['underground_workshop']),
('fresh_job',101,'merchant','What job keeps you here?',[],False,['merchant_background']),
('fresh_owned',202,'merchant','Who does the hinge oil belong to?',[],True,['owner_oil']),
('fresh_birth',0,'merchant','Where did I grow up?',[],False,['unknown']),
('fresh_herbs',101,'merchant','Which herbs make the tonic?',[],False,['unknown']),
('fresh_quote',202,'merchant','Where is my home?',[{'id':1,'turn':1,'text':'My home is Reed Hollow.'}],False,['quote_1']),
('fresh_private',303,'keeper',"What is the merchant's background?",[],False,['unknown'])]
def scenario(case,world):
    id,seed,speaker,q,memory,buy,expected=case;g=game.Game(seed)
    for room in ['gallery','workshop' if speaker=='merchant' else 'cistern']:g.act(g.revision,game.pair('go','room:'+room))
    if buy:g.act(g.revision,game.pair('buy','item:oil'))
    return build(g,world,speaker,q,memory)
def main():
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--tokens-only',action='store_true');p.add_argument('--tokens',type=pathlib.Path);p.add_argument('--model',default='arzaan789/qwen2.5-3b-uncensored:latest');p.add_argument('--output',type=pathlib.Path,required=True);a=p.parse_args()
    if a.cap!=0 or a.output.exists():p.error('require --cap 0 and fresh output')
    world=ui.strict((ROOT/'examples/lantern-vault/world.json').read_text());m=ui.LocalModel(a.model,2048,64,a.output);r=m.records;r.update(tokens_only=a.tokens_only,world=world,cases=[])
    if not a.tokens_only:
        prior=ui.strict(a.tokens.read_text());assert prior['complete'] and prior['tokens_only'] and prior['identity']==m.identity and prior['world']==world
    for case in CASES:
        start=time.perf_counter_ns();offered=scenario(case,world);local=not offered['definition']['cards'];answer='unknown' if local else None
        if not local:
            if a.tokens_only:
                response=m.chat(offered['request'],1,'preflight');assert response['prompt_eval_count']+32+256<=2048
            else:
                old=next(c for c in prior['cases'] if c['id']==case[0]);assert old['offered']==offered
                raw=m.generate(offered['request'],32);answer=ui.strict(raw)['answer']
        result=None
        if answer is not None:
            definition=offered['definition'];available=[{'kind':c['kind'],'id':c['source']} for c in definition['cards']]
            result=Answers(definition).resolve(ui.encode({'answer':answer}),{k:definition[k] for k in ['scope','revision']},available)
        r['cases'].append({'id':case[0],'expected':case[-1],'actual':answer,'local_unknown':local,'correct':None if a.tokens_only else answer in case[-1],
            'offered':offered,'result':result,'turn_micros':(time.perf_counter_ns()-start)//1000})
        ui.checkpoint(a.output,r);print(case[0]+': '+str(answer)+'; API spend $0',flush=True)
    r['complete']=True;ui.checkpoint(a.output,r)
    print(ui.encode({'cases':len(CASES),'correct':sum(c['correct'] is True for c in r['cases']),'local_unknowns':sum(c['local_unknown'] for c in r['cases']),'calls':len(m.calls),'api_spend_usd':0}))
if __name__=='__main__':main()
