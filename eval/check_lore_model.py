"""Offline audits of every retained lore pilot, including failed historical routes."""
import argparse,importlib,pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import lore_reference as ref
import dungeon_reference as game
from lore_native_eval import CASES,FRESH,EXTRA,STOCK
import play_ai as ui
BUILDERS={v:importlib.import_module('dungeon_knowledge_reference'+('' if v==7 else '_v'+str(v))).build for v in range(1,8)}

def check(path):
    r=ui.strict(path.read_text());assert r['complete'] and r['api_spend_usd']==0
    version=r.get('workflow_version',1);native=r.get('native',False);calls=r['calls'];cursor=correct=local=0
    if native:cases={'regression':CASES,'fresh':FRESH,'extra':EXTRA,'repair':[EXTRA[3]],'stock':STOCK}[r['cohort']]
    else:
        cases=(importlib.import_module('lore_model_pilot_v1') if version==1 else importlib.import_module('lore_model_pilot_v2')).CASES
    assert len(r['cases'])==len(cases)
    for recorded,case in zip(r['cases'],cases):
        assert recorded['id']==case[0] and recorded['expected']==case[-1]
        g=game.Game(case[1])
        for room in ['gallery','workshop' if case[2]=='merchant' else 'cistern']:g.act(g.revision,game.pair('go','room:'+room))
        if case[5]:g.act(g.revision,game.pair('buy','item:oil'))
        expected=BUILDERS[version](g,r['world'],case[2],case[3],case[4]);offered=recorded['offered']['offer'] if native else recorded['offered'];assert offered==expected,(path.name,case[0])
        definition=offered['definition'];answers=ref.Answers(definition);is_local=not definition['cards'];assert recorded['local_unknown']==is_local;local+=is_local
        if native:assert recorded['calls_start']==cursor
        if not is_local:
            probe=calls[cursor];cursor+=1;assert probe['phase']=='preflight'
            for k,v in offered['request'].items():assert probe['payload'][k]==v
            assert probe['response']['prompt_eval_count']+32+256<=2048
            if not r['tokens_only']:
                generation=calls[cursor];cursor+=1;assert generation['phase']=='generation'
                assert generation['payload']['messages']==probe['payload']['messages'] and generation['payload']['format']==probe['payload']['format']
                assert generation['payload']['options']['num_predict']==32 and generation['response'].get('done_reason')!='length'
                assert ui.strict(generation['response']['message']['content'])=={'answer':recorded['actual']}
        if native:assert recorded['calls_end']==cursor and recorded['retries']==0
        if not r['tokens_only']:
            actual=recorded['actual'];assert recorded['correct']==(actual in case[-1]);correct+=recorded['correct']
            result=answers.resolve(ui.encode({'answer':actual}),{k:definition[k] for k in ['scope','revision']},[{'kind':c['kind'],'id':c['source']} for c in definition['cards']])
            if native:
                assert recorded['result']=={'speaker':case[2],'speech':result['text'],'sources':result['sources'],'status':result['status']}
            else:assert recorded['result']==result
            assert recorded['turn_micros']>=0
    assert cursor==len(calls)
    for c in calls:
        assert c['status']=='complete' and c['response']['done'] and c['payload']['model']==r['identity']['model']
        o=c['payload']['options'];assert o['num_ctx']==2048 and o['num_gpu']==0 and o['num_thread']==4 and o['temperature']==0 and o['seed']==42
        assert o['num_predict']==(1 if c['phase']=='preflight' else 32)
    return {'version':version,'cases':len(cases),'correct':None if r['tokens_only'] else correct,'local_unknowns':local,'calls':len(calls),'api_spend_usd':0}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('paths',type=pathlib.Path,nargs='+');a=p.parse_args()
    if a.cap!=0:p.error('--cap must be 0')
    for path in a.paths:print(ui.encode(check(path)))
