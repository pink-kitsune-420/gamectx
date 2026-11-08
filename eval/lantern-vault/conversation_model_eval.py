"""Fresh paired host-driven local 3B greeting -> wellbeing -> why conversation."""
import argparse,http.client,http.server,json,pathlib,re,subprocess,tempfile,threading,time,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2]
QUESTIONS=['hello, how are you today?','Why is that?']
def cohort(model=None):
 rows=[]
 for mode in ['baseline','candidate']:
  with tempfile.TemporaryDirectory(prefix='conversation-model-')as tmp:
   d=pathlib.Path(tmp);captured=[]
   class Proxy(http.server.BaseHTTPRequestHandler):
    def log_message(self,*a):pass
    def do_POST(self):
     req=json.loads(self.rfile.read(int(self.headers['Content-Length'])));start=time.monotonic()
     if model:
      with urllib.request.urlopen('http://127.0.0.1:8787/api/state',timeout=3)as f:assert not json.load(f)['pending_jobs'],'Player request is running; do not overlap it.'
      request=urllib.request.Request('http://127.0.0.1:11434/api/chat',json.dumps(req).encode(),{'Content-Type':'application/json'})
      with urllib.request.urlopen(request,timeout=180)as f:raw=json.load(f)
     else:
      speech=req['format']['properties']['speech'].get('enum',['I feel settled and willing to listen.'if not captured else'I value patience because careful listening matters to me.'])[0];raw=dict(message=dict(content=json.dumps(dict(speech=speech))))
     captured.append(dict(request=req,raw=raw,wall_seconds=round(time.monotonic()-start,4)));b=json.dumps(raw).encode();self.send_response(200);self.send_header('Content-Length',str(len(b)));self.end_headers();self.wfile.write(b)
   server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Proxy);threading.Thread(target=server.serve_forever,daemon=True).start();cfg=json.loads((ROOT/'examples/lantern-vault/web/providers.json').read_text());cfg['cpu_threads']=4;cfg['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{server.server_port}/api/chat';cfg['text_models']['local_ollama']['3']=model or'fixture:3b';(d/'providers.json').write_text(json.dumps(cfg));mind=json.loads((ROOT/'examples/lantern-vault/web/personalities.json').read_text());mind['enabled']=mode=='candidate';(d/'personalities.json').write_text(json.dumps(mind));child=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--cap','0','--port','0','--web-config',str(d/'providers.json'),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
   try:
    port=int(re.search(r'127\.0\.0\.1:(\d+)',child.stdout.readline())[1])
    def call(op=None,**args):
     h=http.client.HTTPConnection('127.0.0.1',port,timeout=5);h.request('POST'if op else'GET','/api/'+op if op else'/api/state',json.dumps(args)if op else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=h.getresponse();v=json.loads(r.read());h.close();assert r.status==200,(r.status,v);return v['result']if op else v
    def wait():
     end=time.monotonic()+185
     while True:
      s=call()
      if not s['pending_jobs']:return s
      assert time.monotonic()<end;time.sleep(.05)
    def finish(s):
     sid=s['speech']['id'];n=len(s['speech']['text']);call('delivery',id=sid,event='queued',through=0,heard=0);call('delivery',id=sid,event='played',through=n,heard=n);call('delivery',id=sid,event='finished',through=n,heard=n)
    for room in ['gallery','workshop']:s=call();call('act',revision=s['game']['revision'],decision=dict(action='go',target='room:'+room))
    call('chat',text='Hello.');s=wait();greeting=s['speech']['text'];finish(s);settings={k:v for k,v in s['providers'].items()if k in ['text','image','tts','stt','size','voices','speech_mode']};settings.update(text='local_ollama',cap_micros=0);call('settings',**settings)
    for q in QUESTIONS:
     before=len(captured);call('chat',text=q);s=wait();packet=captured[-1]if len(captured)>before else dict(request=None,raw=None,wall_seconds=0);record=dict(mode=mode,question=q,greeting=greeting,**packet,delivered=s['speech']['text'],origin=s['speech']['origin'],permanent_views=s['personality']['viewpoints']['entries']);rows.append(record);finish(s);record['confirmed_views']=call()['personality']['viewpoints']['entries'];print(json.dumps(dict(mode=mode,question=q,delivered=record['delivered'],origin=record['origin'],wall_seconds=record['wall_seconds'])),flush=True)
   finally:child.terminate();child.wait(timeout=5);server.shutdown();server.server_close()
 return rows
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--tokens-only',action='store_true');p.add_argument('--model');p.add_argument('--holdout',action='store_true');p.add_argument('--reason-holdout',action='store_true');p.add_argument('--final-holdout',action='store_true');p.add_argument('--stem',default='conversation-model');a=p.parse_args();assert a.cap==0;
 if a.holdout:QUESTIONS[:]=['Hey, how are you feeling?','What do you mean?']
 if a.reason_holdout:QUESTIONS[:]=["How's it going?",'Can you explain?']
 if a.final_holdout:QUESTIONS[:]=['Are you doing okay?','Tell me more.','What do you enjoy?']
 prefix=ROOT/'eval'/'lantern-vault'/a.stem;tokenfile=pathlib.Path(str(prefix)+'-tokens.json')
 if a.tokens_only:
  assert not tokenfile.exists();r=cohort();tokenfile.write_text(json.dumps(dict(rows=r,prepared_bytes=[len(json.dumps(c['request'],separators=(',',':')).encode())if c['request']is not None else 0 for c in r],actual_provider_calls=0,api_spend_usd=0,limits='Measured request bytes; exact token counts require live local inference.'),indent=2)+'\n');return
 assert tokenfile.exists()and a.model;results=pathlib.Path(str(prefix)+'-results.json');review=pathlib.Path(str(prefix)+'-review.json');assert not results.exists()and not review.exists();rows=cohort(a.model);results.write_text(json.dumps(dict(model=a.model,threads=4,rows=rows,local_calls=4,api_spend_usd=0),indent=2)+'\n')
 scores=[]
 for c in rows:
  line=c['delivered'];well=c['question']==QUESTIONS[0];relevant=line.startswith('I feel ')if well else any(w in line.lower()for w in ['because','value','prefer','care','appreciate','trust','listen','quiet','matter']);previous=next((v['delivered']for v in rows if v['mode']==c['mode']and v['question']==QUESTIONS[0]),'')
  meta_free=not any(w in ['host','engine','localhost','npc','player_1','player_2']for w in re.findall('[a-z0-9_]+',line.lower()));relevant=relevant and meta_free
  if c['question']=='What do you enjoy?':relevant=meta_free and any(w in line.lower()for w in ['sketch','design','draw','craft','read','repair','tinker','chime','lamp'])
  scores.append(dict(meta_free=meta_free,mode=c['mode'],question=c['question'],relevant=relevant,model_accepted=c['origin']=='subjective_model',duplicate_previous=not well and line==previous,permanent_views=len(c.get('confirmed_views',c['permanent_views'])),prompt_tokens=(c['raw']or{}).get('prompt_eval_count'),output_tokens=(c['raw']or{}).get('eval_count')))
 review.write_text(json.dumps(dict(scores=scores,summary={m:dict(relevant=sum(v['relevant']for v in scores if v['mode']==m),n=len(QUESTIONS),model_calls=sum(c['request']is not None for c in rows if c['mode']==m),duplicate_followups=sum(v['duplicate_previous']for v in scores if v['mode']==m))for m in ['baseline','candidate']},api_spend_usd=0,limits='One original paired merchant conversation; lexical relevance and mechanical acceptance, not general conversational intelligence or semantic grounding proof. Raw provider and delivered outputs retained.'),indent=2)+'\n');print(review.read_text())
if __name__=='__main__':main()
