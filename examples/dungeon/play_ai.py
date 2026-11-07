#!/usr/bin/env python3
"""Human player / local AI characters. Python standard library; no paid API or model pulls."""
import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import sys
import time
import urllib.request

class HostError(ValueError):pass

def encode(v):return json.dumps(v,ensure_ascii=False,separators=(',',':'),allow_nan=False)
def strict(s):
    def unique(rows):
        out={}
        for k,v in rows:
            if k in out:raise ValueError('duplicate JSON key')
            out[k]=v
        return out
    return json.loads(s,object_pairs_hook=unique,parse_constant=lambda _:(_ for _ in ()).throw(ValueError('nonfinite JSON')))

def checkpoint(path,value):
    if path is None:return
    path.parent.mkdir(parents=True,exist_ok=True);temp=path.with_suffix(path.suffix+'.tmp')
    temp.write_text(json.dumps(value,ensure_ascii=False,indent=2,allow_nan=False)+'\n');temp.replace(path)

class Host:
    def __init__(self,binary,seed,world=None):
        command=[str(binary),'--serve','--seed',str(seed)]+(['--world',str(world)] if world is not None else [])
        self.process=subprocess.Popen(command,stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True,encoding='utf-8');self.next=0
    def call(self,op,args):
        self.next+=1;ident=str(self.next);body=encode({'version':1,'id':ident,'op':op,'args':args})+'\n'
        if len(body.encode())>16384:raise HostError('host request too large')
        self.process.stdin.write(body);self.process.stdin.flush();line=self.process.stdout.readline(1048577)
        if not line.endswith('\n') or len(line.encode())>1048576:raise RuntimeError('dungeon host closed or returned an oversized reply')
        r=strict(line)
        if r.get('version')!=1 or r.get('id')!=ident:raise RuntimeError('dungeon reply ID mismatch')
        if not r.get('ok'):raise HostError(r.get('error','host rejected request'))
        return r['result']
    def close(self):
        if self.process.poll() is None:
            self.process.stdin.close()
            try:self.process.wait(timeout=2)
            except subprocess.TimeoutExpired:self.process.kill();self.process.wait()
        self.process.stdout.close()

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self,*args,**kwargs):raise ValueError('HTTP redirects are disabled')
class LocalModel:
    def __init__(self,model,context=2048,max_calls=128,log=None,guidance="focused",size="3",count_cache=0):
        if type(count_cache) is not int or not 0<=count_cache<=64:raise ValueError('count cache must be 0..64')
        self.count_cache=count_cache;self._counts={};self.count_receipts=[]
        self.choices_limit=None;self.size=size;self.guidance=guidance;self.model=model;self.context=context;self.maximum=max_calls;self.log=log;self.calls=[]
        self.opener=urllib.request.build_opener(urllib.request.ProxyHandler({}),NoRedirect())
        self.identity=self.identify();self.records={'version':1,'identity':self.identity,'guidance':guidance,'size':size,'api_spend_usd':0,'calls':self.calls}
        if count_cache:self.records.update(count_cache=count_cache,count_receipts=self.count_receipts)
        checkpoint(log,self.records)
    def data(self,path,payload=None):
        request=urllib.request.Request('http://127.0.0.1:11434'+path,data=encode(payload).encode() if payload is not None else None,headers={'Content-Type':'application/json'})
        with self.opener.open(request,timeout=60) as response:
            body=response.read(1048577)
            if len(body)>1048576:raise ValueError('model response exceeds byte limit')
        v=strict(body.decode())
        if 'error' in v:raise ValueError(str(v['error'])[:240])
        return v
    def identify(self):
        models=self.data('/api/tags')['models'];found=next((m for m in models if m['name']==self.model),None)
        if found is None:raise ValueError('Model is not installed locally. Select an installed tag with --model. Installed: '+', '.join(m['name'] for m in models))
        show=self.data('/api/show',{'model':self.model})
        if found.get('details',{}).get('format')!='gguf' or type(found.get('size')) is not int or found['size']<=0 or any(v.get(k) for v in [found,show] for k in ['remote_host','remote_model']):raise ValueError('Only installed local GGUF models are supported')
        return {'model':self.model,'digest':found['digest'],'quantization':found['details'].get('quantization_level'),
                'template_sha256':hashlib.sha256(show.get('template','').encode()).hexdigest(),'runner':self.data('/api/version')['version']}
    def chat(self,request,output,phase):
        if not isinstance(request,dict) or set(request)&{'model','options','stream','keep_alive'}:raise ValueError('model and transport settings are host-owned')
        if type(output) is not int or not 1<=output<=self.context:raise ValueError('invalid output reservation')
        if len(self.calls)>=self.maximum:raise RuntimeError('Local generation-call limit reached; restart or choose a larger --max-calls')
        payload={'model':self.model,**request,'stream':False,'keep_alive':'10m',
            'options':{'num_ctx':self.context,'num_predict':output,'num_thread':4,'num_gpu':0,'temperature':0,'seed':42}}
        entry={'phase':phase,'payload':payload,'status':'reserved'};self.calls.append(entry);checkpoint(self.log,self.records)
        start=time.perf_counter_ns()
        try:
            response=self.data('/api/chat',payload)
            if not response.get('done'):raise ValueError('model response did not complete')
            entry.update(status='complete',response=response,elapsed_micros=(time.perf_counter_ns()-start)//1000)
            checkpoint(self.log,self.records);return response
        except Exception as e:
            entry.update(status='failed',error=type(e).__name__);checkpoint(self.log,self.records);raise
    def generate(self,request,output):
        if type(output) is not int or not 1<=output<=self.context:raise ValueError('invalid output reservation')
        capacity=getattr(self,'count_cache',0)
        key=hashlib.sha256(encode({'identity':self.identity,'context':self.context,'output':output,'request':request}).encode()).hexdigest() if capacity else None
        cached=self._counts.get(key) if capacity else None
        if len(self.calls)+(1 if cached else 2)>self.maximum:raise RuntimeError('Not enough local calls for a complete turn')
        if self.identify()!=self.identity:
            if capacity:self._counts.clear()
            raise ValueError('Model/template/runner identity changed; restart this AI session')
        receipt=None
        if capacity:
            receipt={'key':key,'reused':cached is not None,'input_tokens':None,'source_preflight':None,'source_generation':None,'status':'reserved'}
            self.count_receipts.append(receipt)
        try:
            if cached:count,first,second=cached
            else:
                first=len(self.calls);probe=self.chat(request,1,'preflight');count=probe.get('prompt_eval_count');second=None
            if type(count) is not int or count<0 or count+output+256>self.context:raise ValueError('Full model prompt does not fit the output/safety reservation')
            generated=len(self.calls);response=self.chat(request,output,'generation')
            if type(response.get('prompt_eval_count')) is not int or response['prompt_eval_count']!=count:
                if capacity:self._counts.pop(key,None)
                raise ValueError('Full request token count changed; cached count discarded')
            if response.get('done_reason')=='length':raise ValueError('reply reached its generation limit')
            value=response.get('message',{}).get('content')
            if not isinstance(value,str) or len(value.encode())>4096:raise ValueError('invalid or oversized model text')
            if capacity and not cached:
                second=generated
                if len(self._counts)>=capacity:self._counts.pop(next(iter(self._counts)))
                self._counts[key]=(count,first,second)
            if receipt:receipt.update(input_tokens=count,source_preflight=first,source_generation=second,status='complete')
            checkpoint(self.log,self.records);return value
        except Exception:
            if receipt:receipt['status']='failed';checkpoint(self.log,self.records)
            raise

def corrected(request,text):
    return {**request,'messages':[*request['messages'],{'role':'assistant','content':text[:4096]},
        {'role':'user','content':'The host rejected that response. Preserve the original request and current facts. Return only the required JSON with supplied IDs; no actions in speech.'}]}

def speak(host,model,speaker,utterance,lore=False,natural=False):
    offered=host.call('natural_speech_prepare' if natural else 'lore_speech_prepare' if lore else 'speech_prepare',{'speaker':speaker,'input':utterance,'context':model.context,'guidance':getattr(model,'guidance','standard'),'size':getattr(model,'size','3')});request=offered['request'];last=None
    for _ in range(2):
        text=''
        try:
            text=model.generate(request,offered['budget']['reserved_output'])
            submitted=text
            if natural or getattr(model,'guidance','standard')=='focused':
                parsed=strict(text)
                if not isinstance(parsed,dict) or set(parsed)!=set(['speech']):raise HostError('expected speech only')
                submitted=encode({'speech':parsed['speech'],'tone':'neutral','fact_ids':[],'memory_ids':[]})
            return host.call('speech_finish',{'ticket':offered['ticket'],'text':submitted})
        except (ValueError,HostError) as e:
            last=e;request=corrected(request,text)
    raise HostError('Could not obtain a valid character reply: '+str(last))

def interpret(host,model,utterance,confirm,choose_target=None):
    args={'input':utterance,'context':model.context,'guidance':getattr(model,'guidance','standard'),'size':getattr(model,'size','3'),
        'resolve_slots':getattr(model,'guidance','standard')=='focused','resolve_ambiguity':getattr(model,'guidance','standard')=='focused'}
    offered=host.call('interpret',args)
    resolution=offered.get('resolution',{})
    if resolution.get('reason') in ['ambiguous','unresolved_reference']:
        candidates=resolution['candidates']
        if choose_target is None:return {'status':'clarify','candidates':candidates,'message':'Which did you mean: '+', '.join(c['label'] for c in candidates)+'? Choose a listed action with its target.'}
        target=choose_target(candidates)
        if target is None:return {'status':'cancelled'}
        if target not in [c['id'] for c in candidates]:raise HostError('unknown clarification target')
        offered=host.call('interpret',{**args,'target_hint':target,'expected_revision':offered['revision']})
    if offered.get('finished'):return {'status':'finished'}
    request=offered['request'];choices={n['name']:n['pair'] for n in offered['offer']}
    if len(choices)==1 and choices.get('clarify','missing') is None:return {'status':'clarify'}
    limit=getattr(model,'choices_limit',None)
    if limit is not None and sum(pair is not None for pair in choices.values())>limit:return {'status':'clarify'}
    value=None
    for _ in range(2):
        raw=''
        try:
            raw=model.generate(request,offered["budget"]["reserved_output"]);value=strict(raw)
            if not isinstance(value,dict) or set(value)!=set(['choice']) or not isinstance(value['choice'],str) or value['choice'] not in choices:raise ValueError('unknown interpreted choice')
            break
        except ValueError:value=None;request=corrected(request,raw)
    if value is None:raise HostError('Could not interpret that request')
    pair=choices[value['choice']]
    if pair is None:return {'status':'clarify'}
    # Interpretation can be legal but wrong. The human approves every world-changing
    # proposal; explicit listed commands remain the deterministic, one-step path.
    if pair['action'] not in ['look','inspect','wait','talk'] and not confirm(pair):return {'status':'cancelled'}
    if pair['action']=='talk':return {'status':'conversation','speaker':pair['target'].split(':')[-1]}
    return host.call('decide',{'revision':offered['revision'],'text':encode(value),'format':'named'})

def exact(state,input):
    parts=input.split()
    for pair in state['choices']:
        if not parts or parts[0]!=pair['action']:continue
        target=pair['target']
        if target is None and len(parts)==1:return pair
        if target and ((len(parts)==2 and parts[1] in [target,target.split(':')[-1]]) or len(parts)==1 and parts[0]=='look'):return pair
    return None

def show(state):
    print('\n'+state['room'].split(':')[-1]+' | health '+str(state['hp'])+'/6 | coins '+str(state['coins']))
    print(state['description'])
    for item in state['items']:print('  '+item['label']+' — '+item['owner']+'; price '+str(item['price']))
    print('Actions: '+' | '.join(p['action']+(' '+p['target'].split(':')[-1] if p['target'] else '') for p in state['choices']))

def nearby(state):return {'room:workshop':'merchant','room:cistern':'keeper'}.get(state['room'])
def display(reply):print(reply['speaker'].capitalize()+': '+reply['speech'])
def literal_action(state, utterance):
    """Small explicit command grammar. Extra clauses ask for clarification.

    This is an example English policy, not a natural-language intent classifier.
    """
    if '?' in utterance or any(ord(c) < 32 for c in utterance):return None
    words=re.findall(r'[^\W_]+',utterance.lower(),re.UNICODE)
    if not words:return None
    action=words[0]
    if not re.match(r'^\s*'+re.escape(action)+r'(?:\s|$)',utterance,re.IGNORECASE):return None
    rest=words[1:]
    if rest and rest[-1]=='please':rest.pop()
    preposition={'go':'to','inspect':'at','look':'at','heal':'with'}.get(action)
    if rest and rest[0]==preposition:rest.pop(0)
    if rest and rest[0] in ['a','an','the']:rest.pop(0)
    candidates=[]
    for pair in state['choices']:
        if pair['action']!=action:continue
        target=pair['target']
        if target is None:
            if not rest:candidates.append(pair)
            continue
        aliases=[target,target.split(':')[-1]]+[item[k] for item in state['items'] if item['id']==target for k in ['label','kind']]
        if any(rest==re.findall(r'[^\W_]+',alias.lower(),re.UNICODE) for alias in aliases):candidates.append(pair)
    return candidates[0] if len(candidates)==1 else None

def converse(host,model,speaker,utterance):
    local=host.call('trade',{'speaker':speaker,'input':utterance})
    if local['handled']:return {'speaker':speaker,'speech':local['reply']}
    words=re.findall(r'[^\W_]+',utterance.lower(),re.UNICODE)
    if words and words[0] in ['take','inspect','go','look','wait','unlock','attack','heal']:
        state=host.call('state',{});pair=literal_action(state,utterance)
        if pair is None:return {'speaker':'game','speech':'Which available action and item did you mean? Nothing has changed.'}
        result=host.call('act',{'revision':state['revision'],'decision':pair})
        return {'speaker':'game','speech':result.get('message','That action is not available right now.')}
    if not getattr(model,'knowledge',True):return speak(host,model,speaker,utterance)
    args={'speaker':speaker,'input':utterance,'context':model.context,'size':getattr(model,'size','3')}
    if getattr(model,'choices_limit',None) is not None:args['choices_limit']=model.choices_limit
    offered=host.call('knowledge_prepare',args)
    if not offered['handled']:return speak(host,model,speaker,utterance,lore=getattr(model,'dialogue',None)!='natural',natural=getattr(model,'dialogue',None)=='natural')
    offer=offered['offer'];request=offer['request'];empty=not offer['definition']['cards'];local_answer=offer.get('local_answer')
    if getattr(model,'dialogue',None)=='natural' and not empty and not local_answer:return speak(host,model,speaker,utterance,natural=True)
    for attempt in range(2):
        raw=encode({'answer':local_answer or 'unknown'}) if empty or local_answer else ''
        try:
            if not empty and not local_answer:raw=model.generate(request,offer['reserved_output'])
            return host.call('knowledge_finish',{'ticket':offered['ticket'],'text':raw})
        except ValueError:request=corrected(request,raw)
    return host.call('knowledge_finish',{'ticket':offered['ticket'],'text':encode({'answer':'unknown'})})

def conversation(host,model,speaker):
    host.call('speech_state',{'speaker':speaker})
    print(speaker.capitalize()+': Speak freely. /back returns to exploring; /memory shows remembered quotes; /forget clears this conversation. Purchases use a local offer; say yes or no to confirm or cancel.')
    while True:
        utterance=input('you> ').strip()
        if utterance in ['/back','/leave']:host.call('trade_cancel',{});return
        if not utterance:continue
        try:
            if utterance=='/forget':host.call('trade_cancel',{});host.call('speech_forget',{'speaker':speaker});print('Your earlier conversation with this character has been cleared.');continue
            if utterance=='/memory':
                notes=host.call('speech_state',{'speaker':speaker})['memories']
                for note in notes:print('You said: '+note['text'])
                if not notes:print('No earlier words are remembered yet.')
                continue
            display(converse(host,model,speaker,utterance))
        except (ValueError,OSError,RuntimeError) as e:print('Conversation paused: '+str(e))

def play(host,model):
    print('LANTERN VAULT — AI characters\nYou play; the keeper and merchant are local AI characters.\nTalk with them using talk keeper / talk merchant, or say <your words> when nearby.\nUse listed actions or natural language. describe asks the narrator. help and quit are available.')
    show(host.call('state',{}))
    while True:
        state=host.call('state',{})
        if state['won']:print('You escaped with the glass star!');return
        if not state['choices']:print('This run has ended.');return
        utterance=input('> ').strip()
        if not utterance:continue
        if utterance=='quit':return
        try:
            if utterance=='help':print('Find the key and oil, unlock the vault, defeat the sentinel and recover the star.\nTalk to a character to ask questions or introduce yourself. Type /back to resume exploring.\nNatural-language world changes ask for your confirmation. /stats shows local call usage.');continue
            if utterance=='/stats':print(str(len(model.calls))+'/'+str(model.maximum)+' local generation calls used; API spend $0.');continue
            if utterance=='state':print(encode(state));continue
            if utterance=='describe':display(speak(host,model,'narrator','Describe the current surroundings in a short paragraph.'));continue
            if utterance.startswith('say '):
                speaker=nearby(state)
                if speaker is None:print('There is nobody nearby to answer.');continue
                display(converse(host,model,speaker,utterance[4:]));continue
            pair=exact(state,utterance)
            if pair is not None:
                if pair['action']=='talk':conversation(host,model,pair['target'].split(':')[-1]);continue
                result=host.call('act',{'revision':state['revision'],'decision':pair})
            else:
                if nearby(state)=='merchant':
                    local=host.call('trade',{'speaker':'merchant','input':utterance})
                    if local['handled']:
                        display({'speaker':'merchant','speech':local['reply']})
                        if 'result' in local:show(host.call('state',{}))
                        continue
                def confirm(pair):
                    label=str(pair['target']).split(':')[-1].replace('_',' ')
                    current=host.call('state',{});item=next((i for i in current['items'] if i['id']==pair['target']),None)
                    terms=(' for '+str(item['price'])+' coins') if pair['action']=='buy' and item else ''
                    return input('I understood: '+pair['action']+' '+label+terms+'. Proceed? [y/N] ').strip().lower() in ['y','yes']
                def choose_target(candidates):
                    print('Which did you mean?')
                    for index,candidate in enumerate(candidates,1):print('  '+str(index)+'. '+candidate['label'])
                    choice=input('Number, or Enter to cancel: ').strip()
                    return candidates[int(choice)-1]['id'] if choice.isdigit() and 1<=int(choice)<=len(candidates) else None
                result=interpret(host,model,utterance,confirm,choose_target)
                if result['status']=='conversation':conversation(host,model,result['speaker']);continue
            if result['status']=='clarify':print(result.get('message','Please clarify what you want to do, or choose a listed action.'));continue
            if result['status']=='cancelled':print('No action taken.');continue
            print(result.get('message','Action rejected: '+result.get('reason','unknown')));show(host.call('state',{}))
        except (ValueError,OSError,RuntimeError) as e:print('AI turn paused: '+str(e))

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--cap',type=float,required=True)
    p.add_argument('--binary',type=pathlib.Path,default=pathlib.Path(__file__).resolve().parents[2]/'target/debug/examples/dungeon')
    p.add_argument('--count-cache',type=int,default=0,help='opt-in exact-request token-count cache, 0..64 entries');p.add_argument('--world',type=pathlib.Path);p.add_argument('--dialogue',choices=['sources','natural','legacy'],default='sources');p.add_argument('--model',default='arzaan789/qwen2.5-3b-uncensored:latest');p.add_argument('--seed',type=int,default=7)
    p.add_argument('--size',choices=['1','1.5','3','8','30','70'],default='3');p.add_argument('--guidance',choices=['standard','grounded','focused'],default='focused');p.add_argument('--context',type=int);p.add_argument('--choices-limit',type=int);p.add_argument('--max-calls',type=int,default=128);p.add_argument('--log',type=pathlib.Path)
    a=p.parse_args()
    if not 0<=a.count_cache<=64 or a.cap!=0 or not 0<=a.seed<=65535 or (a.context is not None and not 512<=a.context<=32768) or not 1<=a.max_calls<=256 or (a.choices_limit is not None and not 1<=a.choices_limit<=32):p.error('require --cap 0 and bounded seed/context/calls')
    if a.log is not None and a.log.exists():p.error('use a fresh log path')
    host=None
    try:
        model=LocalModel(a.model,a.context or 2048,a.max_calls,a.log,a.guidance,a.size,a.count_cache);model.dialogue=a.dialogue;model.knowledge=a.dialogue!='legacy';host=Host(a.binary,a.seed,a.world)
        identity={'model_digest':model.identity['digest'],'quantization':model.identity['quantization'],
            'template_hash':model.identity['template_sha256'],'tokenizer':'gguf:'+model.identity['digest'],'runner':model.identity['runner']}
        workflow=host.call('workflow',{'size':a.size,'task':'dialogue','identity':identity})
        if a.context is None:model.context=workflow['profile']['budget']['context']
        model.choices_limit=a.choices_limit or workflow['workflow']['choices_hint']
        play(host,model)
    except (KeyboardInterrupt,EOFError):print('\nSession ended.')
    except (ValueError,OSError,RuntimeError) as e:print('AI mode could not start: '+str(e),file=sys.stderr);return 1
    finally:
        if host is not None:host.close()
    return 0
if __name__=='__main__':raise SystemExit(main())
