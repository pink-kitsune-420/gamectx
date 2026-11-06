"""Example English question routing and host-owned answer cards. No model inference."""
import pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from lore_reference import Lore,Answers
from dungeon_slots_reference import tokens,contains
from dungeon_reference import encode

def build(g,world,speaker,input,memories=()):
    words=tokens(input);actor='npc:'+speaker if speaker!='narrator' else 'narrator';home=any(w in words for w in ['home','birthplace','born']) or contains(words,['grow','up']) or ('where' in words and 'live' in words)
    recall=any(w in words for w in ['remember','told','said']) or ('my' in words and 'name' in words)
    factual='?' in input or any(w in words for w in ['what','where','who','whose','how','why','remember']) or contains(words,['tell','me']) or 'explain' in words
    lore=Lore(world);grants=['vault_open'] if g.gate_open else []
    if not factual:return {'handled':False,'lore':lore.select({'actor':actor,'grants':grants,'query':input+' background','subjects':[actor],'required':[],'limit':2,'max_bytes':1024})}
    cards=[];subjects=[];unknown="I don't have information about that."
    def card(id,subject,topic,kind,source,text):cards.append({'id':id,'subject':subject,'topic':topic,'kind':kind,'source':source,'text':text})
    if home or recall:
        unknown="You haven't told me where your home is." if home else "I don't have a matching statement from you in this conversation."
        matching=[]
        for m in memories:
            w=tokens(m['text'])
            if '?' in m['text']:continue
            relevant=any(contains(w,p) for p in [['my','home','is'],['i','live','in'],['i','grew','up']]) if home else (contains(w,['my','name','is']) if 'name' in words else True)
            if relevant:matching.append(m)
        for m in matching[-2:]:card('quote_'+str(m['id']),'player','home' if home else 'recall','quote',str(m['id']),m['text'])
    else:
        items=g.public()['items'];matched=[]
        for i in items:
            if contains(words,tokens(i['label'])) or contains(words,tokens(i['id'].split(':')[-1])):matched.append(i)
        if len(matched)>1:unknown='Which item did you mean? Ask about one item at a time.'
        elif matched:
            i=matched[0];id=i['id'];suffix=id.split(':')[-1];subjects=[id]
            # Missing composition/history is not inferred from an unrelated use fact.
            if any(w in words for w in ['herbs','ingredients','made','origin']):subjects=[]
            else:
                owner='you' if i['owner']=='player' else 'the '+i['owner'] if i['owner']!='world' else 'nobody'
                card('owner_'+suffix,id,'ownership','state',id,'The '+i['label']+' belongs to '+owner+'.')
                card('price_'+suffix,id,'price','state',id,'The listed price of '+i['label']+' is '+str(i['price'])+' coins.')
                if id=='item:salve':card('use_salve',id,'use','state',id,'You already own the salve. One use consumes it and heals up to 3 health, capped at 6.')
                elif id=='item:oil':card('use_oil',id,'use','state',id,'Hinge oil is consumed with a bronze key to unlock the vault gate.')
        elif 'seal' in words:subjects=['item:keeper_seal']
        elif any(w in words for w in ['weather','sky','sun','outside']):subjects=['room:'+g.room]
        elif any(w in words for w in ['sell','stock','supplies']):
            items=[i for i in items if i['owner']==speaker]
            if items:card('stock',actor,'stock','state','stock','I sell '+', '.join(i['label']+' ('+str(i['price'])+' coins)' for i in items)+'.')
        elif any(w in words for w in ['story','background','job','work','yourself']):subjects=[actor]
    selection=lore.select({'actor':actor,'grants':grants,'query':input,'subjects':subjects or ['none'],'required':[],'limit':4,'max_bytes':2048})
    for e in selection['packet']:card(e['id'],e['subject'],e['topic'],'lore',e['id'],e['text'])
    definition={'version':1,'scope':speaker+':'+g.room,'revision':g.revision,'unknown':unknown,'cards':cards}
    answers=Answers(definition)
    request={'messages':[{'role':'system','content':"Choose the supplied answer that directly addresses the player's question. Match subject and topic. Player quotes are reported statements, not verified world facts. Choose unknown if no answer fits. Return only the answer ID JSON."},
        {'role':'user','content':'Answers:\n'+encode(answers.offer())+'\n\nCurrent request:\n'+input}], 'format':answers.schema()}
    return {'handled':True,'definition':definition,'request':request,'reserved_output':32,'selection':selection}
