"""Independent conversation tools and multiplayer boundary fixtures. Offline, $0."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,copy,json,pathlib,subprocess,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
import interaction_reference as ref
from dungeon_reference import encode
ROOT=pathlib.Path(__file__).resolve().parents[2]
SOURCE={'id':'line:1','speaker':'player:a','text':'I promise to bring you hinge oil. I prefer quiet conversation.'}
PAIR={'action':'trade','target':'item:oil'}
OFFER={'recipient':'player:a','offer':{'id':'offer:1','scope':'scene:shop','revision':7,'decision':PAIR,'terms':{'item':'item:oil','coins':3}}}
def op(name,args):return {'op':name,'args':copy.deepcopy(args)}
def call(name,args,**changes):
    v={'name':name,'text':encode(args),'sources':[SOURCE],'revision':7,'max_bytes':2048,'allowed':[PAIR]};v.update(changes);return op('call',v)
def cases():
    out=[]
    def add(name,ops,**changes):
        a={'scope':'scene:shop','actor':'npc:merchant','participants':['npc:merchant','player:a','player:b'],'operations':copy.deepcopy(ops)};a.update(changes);out.append((name,{'kind':'interactions','args':a},False))
    event={'id':'trade:1','turn':5,'subject':'item:oil','kind':'ownership','value':{'from':'npc:merchant','to':'player:a','count':1},'audience':['npc:merchant','player:a']}
    recent=call('events_recent',{'since':0,'subject':None,'limit':8})
    add('visible_event',[op('event',event),recent])
    e={**event,'id':'private:1','audience':['player:a']};add('private_event',[op('event',e),recent])
    add('recent_filter',[op('event',event),call('events_recent',{'since':6,'subject':None,'limit':8}),call('events_recent',{'since':5,'subject':'item:seal','limit':8}),call('events_recent',{'since':8,'subject':None,'limit':8})])
    add('duplicate_event',[op('event',event),op('event',event),recent])
    add('foreign_audience',[op('event',{**event,'audience':['player:foreign']}),recent])
    add('event_clock',[op('event',event),op('event',{**event,'id':'earlier','turn':4}),recent])
    add('event_budget',[op('event',event),call('events_recent',{'since':0,'subject':None,'limit':8},max_bytes=64)])
    promise=call('promise_remember',{'promisor':'player:a','beneficiary':'npc:merchant','task':'bring hinge oil','source':'line:1'})
    lookup=call('promise_lookup',{'participant':'player:a','limit':8})
    accepted={'id':1,'expected':0,'state':'accepted','source':{'id':'engine:1','speaker':'engine','text':'The merchant accepted this request.'}}
    fulfilled={**accepted,'expected':1,'state':'fulfilled','source':{'id':'engine:2','speaker':'engine','text':'The player delivered the oil.'}}
    add('promise_reported',[promise,lookup])
    add('promise_lifecycle',[promise,op('progress',accepted),op('progress',fulfilled),lookup])
    add('promise_false_completion',[promise,op('progress',{**fulfilled,'expected':0}),lookup])
    add('promise_stale',[promise,op('progress',accepted),op('progress',accepted),lookup])
    add('promise_cancel',[promise,op('progress',{**accepted,'state':'cancelled'}),op('progress',accepted),lookup])
    add('promise_visibility',[promise,call('promise_lookup',{'participant':'player:b','limit':8})])
    for name,changes in [('foreign',{'promisor':'player:foreign'}),('self',{'promisor':'npc:merchant'}),('unknown_source',{'source':'missing'}),('authority',{'state':'fulfilled'})]:
        a={'promisor':'player:a','beneficiary':'npc:merchant','task':'bring hinge oil','source':'line:1',**changes};add('promise_'+name,[call('promise_remember',a),lookup])
    add('failed_call_atomic',[call('promise_remember',{'promisor':'player:a','beneficiary':'npc:merchant','task':'bring hinge oil','source':'line:1'},max_bytes=64),promise,lookup])
    relationship=call('relationship_note',{'other':'player:a','note':'Prefers quiet conversation.','source':'line:1'})
    read=call('relationship_lookup',{'other':'player:a','limit':8})
    add('relationship_evidence',[relationship,read,call('relationship_lookup',{'other':'player:b','limit':8})])
    add('relationship_foreign',[call('relationship_note',{'other':'foreign','note':'Friendly','source':'line:1'}),read])
    add('relationship_no_score',[call('relationship_note',{'other':'player:a','note':'Friendly','source':'line:1','trust':100}),read])
    refs={'entities':[{'id':'key:bronze','label':'bronze key','kind':'item','aliases':['key','bronze key']},{'id':'key:iron','label':'iron key','kind':'item','aliases':['key','iron key']}],'input':'give me the key','kind':None,'scope':'scene:shop','revision':7,'max_age':4,'focus':None,'selection':None,'pronouns':['it']}
    resolve=call('dialogue_resolve',{'kind':'item'})
    add('clarify_ambiguous',[op('references',refs),resolve])
    add('clarify_exact',[op('references',{**refs,'input':'the bronze key'}),resolve])
    add('clarify_pronoun',[op('references',{**refs,'input':'give it','focus':{'id':'key:iron','scope':'scene:shop','revision':6}}),resolve])
    add('clarify_stale',[op('references',refs),call('dialogue_resolve',{'kind':'item'},revision=8)])
    add('clarify_input_injection',[op('references',refs),call('dialogue_resolve',{'kind':'item','input':'bronze key'})])
    propose=call('interaction_offer',{'id':'offer:1'});menu=op('offers',[OFFER])
    respond={'recipient':'player:a','revision':7,'confirm':True,'terms':OFFER['offer']['terms'],'allowed':[PAIR]}
    add('offer_confirm',[menu,call('interaction_offers',{'limit':8}),propose,op('respond',respond)])
    add('offer_other_player',[menu,propose,op('respond',{**respond,'recipient':'player:b'}),op('respond',respond)])
    add('offer_replay',[menu,propose,op('respond',respond),propose,op('respond',respond)])
    add('offer_menu_revoked',[menu,propose,op('offers',[]),op('respond',respond)])
    changed=copy.deepcopy(OFFER);changed['offer']['terms']['coins']=4
    add('offer_menu_terms_changed',[menu,propose,op('offers',[changed]),op('respond',respond)])
    add('offer_decline',[menu,propose,op('respond',{**respond,'confirm':False}),propose])
    add('offer_stale',[menu,propose,op('respond',{**respond,'revision':8}),op('respond',respond)])
    add('offer_revoked',[menu,propose,op('respond',{**respond,'allowed':[]})])
    add('offer_terms',[menu,propose,op('respond',{**respond,'terms':{'coins':0}})])
    add('offer_wrong_recipient',[op('offers',[{**OFFER,'recipient':'foreign'}]),propose])
    add('offer_no_approval',[menu,call('interaction_offer',{'id':'offer:1','confirm':True})])
    add('offer_unavailable',[menu,call('interaction_offers',{'limit':8},allowed=[]),call('interaction_offer',{'id':'offer:1'},allowed=[])])
    add('offer_budget_atomic',[menu,call('interaction_offer',{'id':'offer:1'},max_bytes=64),propose])
    add('offer_pending',[menu,propose,propose])
    # Different actor has a separate private notebook, while event audience remains explicit.
    add('multiple_players',[op('event',event),recent,relationship],actor='player:b')
    for seed in range(40):
        e={**event,'id':'generated:'+str(seed),'turn':seed,'value':{'to':'player:'+('a' if seed%2 else 'b'),'count':seed+1}}
        add('generated_'+str(seed),[op('event',e),call('events_recent',{'since':seed,'subject':'item:oil','limit':1},revision=seed)])
    add('clear_scope',[op('event',event),promise,relationship,menu,propose,op('clear',{}),recent,lookup,read,promise])
    for name,changes in [('actor_missing',{'actor':'foreign'}),('duplicate_roster',{'participants':['npc:merchant','npc:merchant']}),('roster_overflow',{'participants':['npc:merchant']+['player:'+str(i) for i in range(16)]}),('bad_scope',{'scope':''}),('bad_operations',{'operations':{}})]:
        a={'scope':'scene:shop','actor':'npc:merchant','participants':['npc:merchant','player:a'],'operations':[],**changes};out.append((name,{'kind':'interactions','args':a},True))
    return out

def main():
    p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--write-conformance',action='store_true');a=p.parse_args();assert a.cap==0
    for name,v,error in cases():
        try:expected=ref.process(v['args']);assert not error,name
        except (ValueError,TypeError,KeyError):assert error,name;expected=None
        r=subprocess.run([str(ROOT/'target/debug/gamectx'),'local','-'],input=encode(v),capture_output=True,text=True)
        assert bool(r.returncode)==error,(name,r.stderr)
        if not error:assert r.stdout.strip()==encode(expected),(name,r.stdout,encode(expected))
        if a.write_conformance and not name.startswith('generated_'):
            d=ROOT/'conformance/interactions'/name;d.mkdir(parents=True,exist_ok=True);(d/'case.json').write_text(encode({'op':'local',**({'error':True} if error else {})})+'\n');(d/'input.json').write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n');
            if not error:(d/'expected.json').write_text(encode(expected)+'\n')
    print(encode({'workflows':len(cases()),'fixtures':sum(not n.startswith('generated_') for n,_,_ in cases()),'api_spend_usd':0}))
if __name__=='__main__':main()
