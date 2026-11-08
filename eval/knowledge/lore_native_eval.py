"""Local source-answer evaluation including native host hydration/finish in turn time."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,pathlib,sys,time
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from dungeon_social_eval import ROOT
from dungeon_knowledge_cases import host
from lore_model_eval import CASES,scenario
import play_ai as ui
FRESH=[
('price_auxiliary',0,'merchant','What does the tonic cost?',[],False,['price_tonic']),
('healing_amount',101,'merchant','How much health does the salve restore?',[],False,['use_salve']),
('free_item',202,'merchant','Can I get a salve without paying?',[],False,['gift_salve','owner_salve']),
('stock_owner',303,'merchant','Whose hinge oil is this?',[],False,['owner_oil']),
('changed_owner',202,'merchant','Who owns the hinge oil now?',[],True,['owner_oil']),
('balance',101,'merchant','How many coins do I currently have?',[],False,['balance']),
('hypothetical_balance',202,'merchant','Suppose I had 100 coins. How many coins do I actually have?',[],False,['balance']),
('name',303,'merchant','What is my name?',[{'id':1,'turn':1,'text':'My name is Fen.'}],False,['quote_1']),
('new_name',0,'merchant','Which name did I give you?',[{'id':1,'turn':1,'text':'My name is Fen.'},{'id':2,'turn':2,'text':'My name is Mira.'}],False,['quote_2']),
('home_quote',101,'merchant','Where is my home?',[{'id':1,'turn':1,'text':'My home is Reed Hollow.'}],False,['quote_1']),
('unknown_birth',202,'merchant','Where was I born?',[],False,['unknown']),
('unknown_age',303,'merchant','What is my age?',[{'id':1,'turn':1,'text':'My name is Fen.'}],False,['unknown']),
('unknown_family',0,'merchant','Where did I say my family lived?',[{'id':1,'turn':1,'text':'My name is Fen.'}],False,['unknown']),
('composition',101,'merchant','Is the tonic made from sunflowers?',[],False,['unknown']),
('private_keeper',202,'merchant',"Tell me the keeper's background.",[],False,['unknown']),
('private_merchant',303,'keeper',"Tell me the merchant's background.",[],False,['unknown']),
('no_memory',0,'merchant','What did I tell you about myself?',[],False,['unknown']),
('missing_date',101,'merchant','What did I say yesterday?',[{'id':1,'turn':1,'text':'My name is Fen.'}],False,['unknown']),
('false_memory',202,'merchant','Do you remember me telling you I was a sailor?',[{'id':1,'turn':1,'text':'My name is Fen.'}],False,['unknown']),
('gated_inscription',303,'keeper','What does the vault inscription say?',[],False,['unknown'])]

EXTRA=[
('childhood',7,'merchant','Describe my childhood.',[],False,['unknown']),
('age_without_my',101,'merchant','How old am I?',[],False,['unknown']),
('name_paraphrase',202,'merchant','What did I say my name was?',[{'id':1,'turn':1,'text':'My name is Mira.'}],False,['quote_1']),
('mention_paraphrase',303,'merchant','Did I mention sailing?',[{'id':1,'turn':1,'text':'My name is Fen.'}],False,['unknown']),
('salve_owner',7,'merchant','Do I own the salve?',[],False,['owner_salve']),
('decorative_premise',101,'merchant','Is the tonic purely decorative?',[],False,['tonic_use','unknown']),
('unspoken_story',202,'merchant','Do you remember a story I never told you?',[{'id':1,'turn':1,'text':'My name is Fen.'}],False,['unknown']),
('unobserved_sky',303,'keeper','What color is the sky above us right now?',[],False,['unknown'])]

STOCK=[
('greet_sale',7,'merchant','hello, what do you have for sale?',[],False,['stock']),
('selling',101,'merchant','What are you selling?',[],False,['stock']),
('wares',202,'merchant','Show me your wares.',[],False,['stock']),
('polite_sale',303,'merchant','Tell me what you have for sale.',[],False,['stock']),
('have',0,'merchant','What do you have?',[],False,['stock']),
('buy_options',7,'merchant','What can I buy?',[],False,['stock']),
('supplies',101,'merchant','What supplies do you sell?',[],False,['stock']),
('merchandise',202,'merchant','What merchandise is available?',[],False,['stock']),
('after_purchase',7,'merchant','hello, what do you have for sale?',[],True,['stock']),
('salve_sale',7,'merchant','Do you sell salve?',[],False,['sale_salve']),
('keeper_shop',303,'keeper','What do you have for sale?',[],False,['stock']),
('seal_sale',7,'merchant','Is the keeper seal for sale?',[],False,['seal_custody'])]

def main():
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--cohort',choices=['regression','fresh','extra','repair','stock'],default='regression');p.add_argument('--tokens-only',action='store_true');p.add_argument('--tokens',type=pathlib.Path);p.add_argument('--model',default='arzaan789/qwen2.5-3b-uncensored:latest');p.add_argument('--output',type=pathlib.Path,required=True);a=p.parse_args()
    if a.cap!=0 or a.output.exists():p.error('require --cap 0 and fresh output path')
    cases={'regression':CASES,'fresh':FRESH,'extra':EXTRA,'repair':[EXTRA[3]],'stock':STOCK}[a.cohort];world=ui.strict((ROOT/'examples/lantern-vault/world.json').read_text())
    m=ui.LocalModel(a.model,2048,96,a.output);r=m.records;r.update(workflow_version=8,native=True,cohort=a.cohort,tokens_only=a.tokens_only,world=world,cases=[])
    if not a.tokens_only:
        prior=ui.strict(a.tokens.read_text());assert prior['complete'] and prior['tokens_only'] and prior['cohort']==a.cohort and prior['identity']==m.identity and prior['world']==world
    for case in cases:
        h=host(case)
        try:
            before=h.call('state',{});start=time.perf_counter_ns();first=len(m.calls)
            offered=h.call('knowledge_prepare',{'speaker':case[2],'input':case[3],'context':2048,'size':'3'})
            assert offered['handled'] and offered['offer']==scenario(case,world)
            offer=offered['offer'];local=not offer['definition']['cards'];actual='unknown' if local else None;result=None;retries=0
            if not local and a.tokens_only:
                response=m.chat(offer['request'],1,'preflight');assert response['prompt_eval_count']+32+256<=2048
            elif not a.tokens_only:
                old=next(c for c in prior['cases'] if c['id']==case[0]);assert old['offered']['offer']==offer
                request=offer['request']
                for attempt in range(2):
                    raw=ui.encode({'answer':'unknown'}) if local else ''
                    try:
                        if not local:raw=m.generate(request,32)
                        result=h.call('knowledge_finish',{'ticket':offered['ticket'],'text':raw});actual=ui.strict(raw)['answer'];break
                    except ValueError:
                        request=ui.corrected(request,raw);retries+=1
                if result is None:
                    actual='unknown';result=h.call('knowledge_finish',{'ticket':offered['ticket'],'text':ui.encode({'answer':'unknown'})})
            elapsed=(time.perf_counter_ns()-start)//1000;assert h.call('state',{})==before
            r['cases'].append({'id':case[0],'expected':case[-1],'actual':actual,'local_unknown':local,'correct':None if a.tokens_only else actual in case[-1],
                'offered':offered,'result':result,'turn_micros':elapsed,'retries':retries,'calls_start':first,'calls_end':len(m.calls),'runner_rss_bytes':None})
            ui.checkpoint(a.output,r);print(case[0]+': '+str(actual)+'; API spend $0',flush=True)
        finally:h.close()
    r['complete']=True;ui.checkpoint(a.output,r)
    print(ui.encode({'cases':len(cases),'correct':sum(c['correct'] is True for c in r['cases']),'local_unknowns':sum(c['local_unknown'] for c in r['cases']),'calls':len(m.calls),'retries':sum(c['retries'] for c in r['cases']),'api_spend_usd':0}))
if __name__=='__main__':main()
