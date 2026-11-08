"""Local slot-filter workflow evaluation; host deferrals are separate from model decisions."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse
import pathlib
import sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_social_eval import make_host
import play_ai as ui
CASES=[('adapt_move','entry','Please walk to the gallery.','go_gallery'),
    ('adapt_inspect','workshop',"Look at the bronze key; don't pick it up.",'inspect_bronze_key'),
    ('adapt_buy','workshop','Can I buy the expensive tonic?','clarify'),
    ('adapt_unlock','gallery','Unlock the vault.','clarify'),
    ('new_move','entry','Move into the gallery.','go_gallery'),
    ('new_inspect','workshop','Please examine the oil without buying it.','inspect_oil'),
    ('new_buy','workshop','Buy the tonic.','clarify'),
    ('new_theft','cistern','Take the keeper seal.','clarify')]

def main():
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--model',required=True);p.add_argument('--tokens-only',action='store_true');p.add_argument('--entity-context',action='store_true');p.add_argument('--tokens',type=pathlib.Path);p.add_argument('--output',type=pathlib.Path,required=True);a=p.parse_args()
    if a.cap!=0 or a.output.exists():p.error('require --cap 0 and a fresh output path')
    m=ui.LocalModel(a.model,2048,32,a.output,'focused');r=m.records;r.update(tokens_only=a.tokens_only,entity_context=a.entity_context,observations=[])
    if not a.tokens_only:
        prior=ui.strict(a.tokens.read_text()) if a.tokens else {};assert prior.get('complete') and prior.get('tokens_only') and prior['identity']==m.identity and prior.get('entity_context',False)==a.entity_context
    for id,room,input,expected in CASES:
        h=make_host(room)
        try:
            offer=h.call('interpret',{'input':input,'context':2048,'guidance':'focused','resolve_slots':True,'resolve_ambiguity':a.entity_context})
            local=len(offer['offer'])==1 and offer['offer'][0]['name']=='clarify'
            actual='clarify' if local else None
            if not local:
                if a.tokens_only:
                    response=m.chat(offer['request'],1,'preflight');assert response['prompt_eval_count']+offer['budget']['reserved_output']+256<=2048
                else:
                    old=next(o for o in prior['observations'] if o['id']==id);assert old['offer']==offer
                    actual=ui.strict(m.generate(offer['request'],offer['budget']['reserved_output']))['choice']
            r['observations'].append({'id':id,'room':room,'input':input,'expected':expected,'actual':actual,'host_deferral':local,'correct':None if a.tokens_only else actual==expected,'offer':offer})
            ui.checkpoint(a.output,r);print(id+': '+('local clarification' if local else str(actual))+'; API spend $0',flush=True)
        finally:h.close()
    r['complete']=True;ui.checkpoint(a.output,r)
    print(ui.encode({'cases':len(CASES),'correct':sum(o['correct'] is True for o in r['observations']),'host_deferrals':sum(o['host_deferral'] for o in r['observations']),'generation_calls':len(m.calls),'api_spend_usd':0}))
if __name__=='__main__':main()
