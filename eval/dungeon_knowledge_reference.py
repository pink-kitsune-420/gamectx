"""Example English question routing and host-owned answer cards. No model inference."""
import pathlib,sys
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent))
from lore_reference import Lore,Answers
from dungeon_slots_reference import tokens,contains
from dungeon_reference import encode
import conversation_policy_reference as policy
from dungeon_slots_reference import resolve

def stock_question(input):
    w=tokens(input)
    if policy.location(input) or policy.world(input) or ('information' in w):return False
    historical=contains(w,['did','i']) or contains(w,['did','you']) or any(x in w for x in ['remember','recall','told','said','yesterday','previously','tomorrow'])
    negative=any(contains(w,p) for p in [['not','for','sale'],['isn','t','for','sale'],['not','sell'],['don','t','sell']])
    return not historical and not negative and (any(x in w for x in ['sell','selling','stock','supplies','wares','merchandise']) or any(contains(w,p) for p in [['for','sale'],['your','inventory'],['what','do','you','have'],['do','you','have'],['what','have','you','got'],['what','can','i','buy'],['what','could','i','buy'],['what','is','available'],['items','available']]))

def build(g,world,speaker,input,memories=()):
    words=tokens(input);actor='npc:'+speaker if speaker!='narrator' else 'narrator';home=any(w in words for w in ['home','birthplace','born']) or contains(words,['grow','up']) or ('where' in words and any(w in words for w in ['live','lived','living']))
    literal=policy.letter_request(input)
    if literal is None and len(words)==1 and input.strip().isalpha() and memories and policy.letter_request(memories[-1]['text']) is not None:literal=input.strip()
    listing=stock_question(input);identity=policy.identity(input);bio=policy.biography(input);setting=policy.world(input);overview=policy.overview(input) and not listing and not setting;here=policy.location(input);time=policy.time_question(input);weather=policy.weather_report(input);sky_opinion=policy.opinion(input) and any(w in words for w in ['sky','sun','weather','outside'])
    recall=(contains(words,['did','i']) or any(w in words for w in ['remember','recall','tell','told','say','said']) or ('name' in words and any(w in words for w in ['my','i']))) and not (listing or identity or bio or setting or overview or here or time or weather)
    factual=literal is not None or listing or identity or bio or setting or overview or here or time or weather or '?' in input or any(w in words for w in ['what','where','who','whose','how','why','remember']) or contains(words,['tell','me']) or 'explain' in words or ('describe' in words and 'my' in words)
    lore=Lore(world);grants=['vault_open'] if g.gate_open else []
    if policy.opinion(input) and not weather and not sky_opinion:factual=False
    if not factual:return {'handled':False,'lore':lore.select({'actor':actor,'grants':grants,'query':input+' background','subjects':[actor],'required':[],'limit':2,'max_bytes':1024})}
    cards=[];subjects=[];general=False;item_question=False;unknown=policy.unknown(speaker)
    def card(id,subject,topic,kind,source,text):cards.append({'id':id,'subject':subject,'topic':topic,'kind':kind,'source':source,'text':text})
    profile=not recall and 'my' in words and any(w in words for w in ['age','birthday','siblings','food','surname','family','childhood','occupation','parents','past','background','story','job'])
    temporal=any(w in words for w in ['yesterday','today','tomorrow']) and recall
    if sky_opinion:card('sky_opinion',actor,'observation','state','weather',"I can't see the sky from here, but a little daylight sounds welcome. What is it like where you are?")
    elif literal is not None:card('letter_count','text:request','count','state','text:request',literal+' has '+str(sum(c.isalpha() for c in literal))+' letters.')
    elif identity:subjects=[actor]
    elif setting:subjects=['world:lantern_vault']
    elif overview:
        card('topics',actor,'topics','state','topics','Ask me about this place, current stock and prices, visible item ownership, or how the key, oil and salve work.')
    elif time:
        selected=lore.select({'actor':actor,'grants':grants,'query':'timekeeping','subjects':[actor],'required':[],'limit':4,'max_bytes':4096})
        e=next((e for e in selected['packet'] if e['topic']=='timekeeping'),{})
        mode=e.get('value',{}).get('mode',e.get('text','none'))
        reported=next((m for m in reversed(memories) if policy.time_report(m['text'])),None)
        if mode in ['clock','watch']:
            minutes=8*60+g.revision*10;label='day '+str(1+minutes//1440)+', '+str((minutes//60)%24).zfill(2)+':'+str(minutes%60).zfill(2)
            card('world_time',actor,'time','state',mode,'My '+mode+' reads '+label+'.')
        elif mode=='schedule':
            hour=(8*60+g.revision*10)//60%24;phase='morning' if 5<=hour<=11 else 'afternoon' if 12<=hour<=16 else 'evening' if 17<=hour<=20 else 'night'
            card('schedule_time',actor,'time','state','schedule',"By my routine, it should be around "+phase+". I can't give you an exact time.")
        elif reported:card('reported_time','player','time','quote',str(reported['id']),reported['text'])
        else:card('time_unknown',actor,'time','state','perception',"I don't have a clock to check, and I can't see the sun from here. Do you know what time it is?")
    elif here:
        room=g.public()['room'];name=room.split(':')[-1].replace('_',' ')
        card('location',room,'location','state',room,'We are in the '+name+', inside the underground Lantern Vault.')
    elif weather:
        card('weather_report',actor,'observation','state','weather',"If you're feeling warm or cold, we can take a moment. I can't judge the outdoor weather from here.")
    elif profile or temporal:
        unknown="You haven't supplied that information in this conversation."
    elif home or recall:
        unknown="You haven't told me where your home is." if home else "I don't have a matching statement from you in this conversation."
        matching=[]
        stop={'what','which','did','i','my','me','myself','you','your','we','us','a','an','the','do','does','was','were','is','are','am','have','has','had','that','to','of','about','tell','told','say','said','remember','recall','telling','saying','previously','earlier','anything','something'}
        content=set(words)-stop
        for m in memories:
            w=tokens(m['text'])
            if '?' in m['text']:continue
            relevant=any(contains(w,p) for p in [['my','home','is'],['i','live','in'],['i','grew','up']]) if home else (contains(w,['my','name','is']) if 'name' in words else True)
            if relevant and (home or 'name' in words or not content or content.intersection(w)):matching.append(m)
        for m in matching[-1:] if home or 'name' in words else matching[-2:]:card('quote_'+str(m['id']),'player','home' if home else 'recall','quote',str(m['id']),m['text'])
    else:
        items=g.public()['items'];matched=[]
        for i in items:
            if contains(words,tokens(i['label'])) or contains(words,tokens(i['id'].split(':')[-1])):matched.append(i)
        if not matched:
            r=resolve([],items,input)
            if r['reason'] in ['phrase','ambiguous']:matched=[i for i in items if any(c['id']==i['id'] for c in r['candidates'])]
        if len(matched)>1:unknown='Which item did you mean? Ask about one item at a time.'
        elif matched:
            i=matched[0];id=i['id'];suffix=id.split(':')[-1];subjects=[id];item_question=True
            # Missing composition/history is not inferred from an unrelated use fact.
            if any(w in words for w in ['herbs','ingredients','made','origin']):subjects=[]
            else:
                owner='you' if i['owner']=='player' else 'the '+i['owner'] if i['owner']!='world' else 'nobody'
                card('owner_'+suffix,id,'ownership','state',id,'The '+i['label']+' belongs to '+owner+'.')
                card('price_'+suffix,id,'price','state',id,'The listed price of '+i['label']+' is '+str(i['price'])+' coins.')
                if listing:
                    selling=speaker=='merchant' and i['owner']=='merchant'
                    text='The '+i['label']+' is for sale for '+str(i['price'])+' coins.' if selling else 'The '+i['label']+' is not part of my stock. It belongs to '+owner+'.'
                    card('sale_'+suffix,id,'sale','state',id,text)
                if id=='item:salve':card('gift_salve',id,'permission','state',id,'The salve already belongs to you. It is not merchant stock to give away.')
                if id=='item:salve':card('use_salve',id,'use','state',id,'You already own the salve. One use consumes it and heals up to 3 health, capped at 6.')
                elif id=='item:bronze_key':card('use_bronze_key',id,'use','state',id,'Use the bronze key with hinge oil to unlock the vault gate in the gallery.')
                elif id=='item:oil':card('use_oil',id,'use','state',id,'Hinge oil is consumed with a bronze key to unlock the vault gate.')
        elif 'seal' in words:subjects=['item:keeper_seal'];item_question=True
        elif any(w in words for w in ['weather','sky','sun','outside']):
            unknown="I can't observe the current outdoor weather or sky from here."
        elif listing:
            items=[i for i in items if speaker=='merchant' and i['owner']=='merchant']
            text=('That is all my current stock: ' if policy.stock_extra(input) else 'I sell ')+', '.join(i['label']+' ('+str(i['price'])+(' coin)' if i['price']==1 else ' coins)') for i in items)+'.' if items else "I don't have any items for sale."
            card('stock',actor,'stock','state','stock',text)
        elif any(w in words for w in ['story','background','job','work','yourself']):subjects=['npc:merchant' if 'merchant' in words else 'npc:keeper' if 'keeper' in words else actor]
        elif 'coins' in words:
            card('balance','player','balance','state','wallet','You have '+str(g.coins)+' coins.')
        else:general=True
    selection=lore.select({'actor':actor,'grants':grants,'query':policy.meaningful(input) if general else input,'subjects':subjects,'required':[],'limit':4,'max_bytes':2048}) if subjects or general else {'packet':[],'receipt':{'bytes':2,'selected':[],'omitted':[]}}
    for e in selection['packet']:
        value=e.get('value');body=e['text'] if 'text' in e else encode(value)
        if e['topic']=='identity' and isinstance(value,dict) and isinstance(value.get('name'),str):body="I'm "+value['name']+(', the '+value['role'] if isinstance(value.get('role'),str) else '')+'.'
        elif e['topic']=='setting' and isinstance(value,dict) and isinstance(value.get('summary'),str):body=value['summary']
        card(e['id'],e['subject'],e['topic'],'lore',e['id'],body)
    # Exclude irrelevant answer topics; ownership and implemented-use facts remain
    # in the host state rather than being replaced by text from lore.
    if identity:
        cards=[c for c in cards if c['topic']=='identity']
        if not cards:card('identity',actor,'identity','state','identity',"I'm the "+speaker+". You can call me "+speaker.capitalize()+'.')
    elif setting:cards=[c for c in cards if c['topic']=='setting']
    elif bio:cards=[c for c in cards if c['topic']=='background']
    elif item_question and listing:cards=[c for c in cards if c['topic'] in (['sale'] if any(c['topic']=='sale' for c in cards) else ['ownership','permission'])]
    elif item_question and any(w in words for w in ['own','owner','belongs','belong','whose']):cards=[c for c in cards if c['topic']=='ownership']
    elif item_question and any(w in words for w in ['give','free']):cards=[c for c in cards if c['topic'] in ['permission','ownership']]
    elif item_question and any(w in words for w in ['price','cost']):cards=[c for c in cards if c['topic']=='price']
    elif item_question and any(w in words for w in ['use','effect','purpose','do','does','heal']):cards=[c for c in cards if c['topic']=='use']
    definition={'version':1,'scope':speaker+':'+g.room,'revision':g.revision,'unknown':unknown,'cards':cards}
    answers=Answers(definition)
    request={'messages':[{'role':'system','content':"Choose the supplied answer that directly addresses the player's question. Match subject and topic. Player quotes are reported statements, not verified world facts. Choose unknown if no answer fits. Return only the answer ID JSON."},
        {'role':'user','content':'Answers:\n'+encode(answers.offer())+'\n\nCurrent request:\n'+input}], 'format':answers.schema()}
    out={'handled':True,'definition':definition,'request':request,'reserved_output':32,'selection':selection}
    if len(cards)==1 and (literal is not None or identity or setting or overview or here or time or weather or sky_opinion or listing or item_question or home or recall):out['local_answer']=cards[0]['id']
    return out
