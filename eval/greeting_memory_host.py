"""Zero-model host encounters: heard introductions, party isolation, return/gap and branches."""
import argparse,http.client,http.server,json,pathlib,re,subprocess,tempfile,threading
ROOT=pathlib.Path(__file__).resolve().parents[1]
def run():
 with tempfile.TemporaryDirectory(prefix='greeting-host-')as tmp:
  d=pathlib.Path(tmp);requests=[]
  class Forbidden(http.server.BaseHTTPRequestHandler):
   def log_message(self,*a):pass
   def do_POST(self):requests.append(self.path);self.send_error(500)
  mock=http.server.ThreadingHTTPServer(('127.0.0.1',0),Forbidden);threading.Thread(target=mock.serve_forever,daemon=True).start();cfg=json.loads((ROOT/'examples/dungeon/web/providers.json').read_text());cfg['text']='local_ollama';cfg['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{mock.server_port}/api/chat';(d/'providers.json').write_text(json.dumps(cfg));mind=json.loads((ROOT/'examples/dungeon/web/personalities.json').read_text());mind['enabled']=True;(d/'personalities.json').write_text(json.dumps(mind));child=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--port','0','--cap','0','--web-config',str(d/'providers.json'),'--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);checks=0
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',child.stdout.readline())[1])
   def call(op=None,**args):
    nonlocal checks
    h=http.client.HTTPConnection('127.0.0.1',port,timeout=5);h.request('POST'if op else'GET','/api/'+op if op else'/api/state',json.dumps(args)if op else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=h.getresponse();v=json.loads(r.read());h.close();checks+=1;assert r.status==200,(r.status,v);return v['result']if op else v
   def go(room):s=call();call('act',revision=s['game']['revision'],decision=dict(action='go',target='room:'+room))
   def finish(s):
    sid=s['speech']['id'];n=len(s['speech']['text']);call('delivery',id=sid,event='queued',through=0,heard=0);call('delivery',id=sid,event='played',through=n,heard=n);call('delivery',id=sid,event='finished',through=n,heard=n)
   def greet(q='Hello.',heard=True):
    r=call('chat',text=q);s=call();assert not s['pending_jobs']and s['speech']['origin']=='authored_social';assert r['social']=='greeting';finish(s)if heard else None;return r['greeting'],s['speech']['text'],s
   go('gallery');go('workshop');plan,first,s=greet('Hello!',False);assert plan['meeting']=='first'and'Ilen'in first and'workshop'in first;assert not call()['greetings']['memory']['entries'];sid=s['speech']['id'];call('delivery',id=sid,event='queued',through=0,heard=0);call('delivery',id=sid,event='played',through=5,heard=5);call('delivery',id=sid,event='interrupted',through=5,heard=5);assert not call()['greetings']['memory']['entries']
   plan,_,_=greet('Hello again.');assert plan['meeting']=='first'and plan['needs_introduction'];entries=call()['greetings']['memory']['entries'];assert entries[0]['introduced']and entries[0]['greetings']==1
   same=[]
   for _ in range(3):plan,line,_=greet();same.append(line);assert plan['meeting']=='continuing'and not plan['needs_introduction']and'Ilen'not in line
   assert len(set(same))==3
   call('select',player='player_2',npc='merchant');plan,line,_=greet('Hi Ilen!');assert plan['meeting']=='first'and'Ilen'in line;call('select',player='player_1',npc='merchant');plan,_,_=greet();assert plan['meeting']=='continuing'
   go('gallery');go('workshop');plan,line,_=greet();assert plan['meeting']=='returning'and'back'in line.lower()and'Ilen'not in line
   call('greeting_settings',version=1,long_gap_turns=3);call('save',slot=0)
   for _ in range(3):s=call();call('act',revision=s['game']['revision'],decision=dict(action='wait',target=None))
   plan,line,_=greet();assert plan['meeting']=='after_gap'and'back'not in line.lower()
   call('load',slot=0);plan,_,_=greet();assert plan['meeting']=='continuing'
   # First greeting must survive eviction from the bounded dialogue receipt tail.
   for _ in range(110):plan,_,_=greet();assert plan['meeting']=='continuing'
   row=next(e for e in call()['greetings']['memory']['entries']if e['actor']=='merchant'and e['listener']=='player_1');assert row['introduced']and row['greetings']>=115
   call('save',slot=1);go('gallery');go('cistern');call('select',player='player_1',npc='keeper');plan,line,_=greet('Greetings.');assert plan['meeting']=='first'and'Tovin'in line and'cistern'in line
   call('load',slot=1);plan,_,_=greet();assert plan['meeting']=='continuing'
   call('save',slot=2);p=d/'branch-2.json';old=json.loads(p.read_text());old.pop('encounters');p.write_text(json.dumps(old));call('load',slot=2);plan,line,_=greet();assert plan['meeting']=='familiar'and'again'in line.lower()
   call('reset');assert not call()['greetings']['memory']['entries']and call()['greetings']['config']['long_gap_turns']==3;go('gallery');go('workshop');plan,line,_=greet();assert plan['meeting']=='first'and'Ilen'in line;assert not requests;assert call()['spending']['charged_or_reserved_micros']==0
   return dict(workflows=12,http_checks=checks,confirmed_first_introduction=first,partial_introduction_recorded=False,same_encounter_distinct_replies=len(set(same)),player_and_npc_isolation=True,return_and_pause_distinct=True,confirmed_greetings_survive_110_turn_noise=True,branch_restore=True,legacy_contact_migration=True,reset_clears_contacts=True,actual_provider_calls=0,mock_provider_requests=len(requests),api_spend_usd=0,limits='Original offline host with a provider endpoint that rejects any generation. Text read/delivery receipts, not physical audio. Demo NPCs stay at their workplaces; general changed-location and private/emergency cases are tested in the pure API and renderer.')
  finally:child.terminate();child.wait(timeout=5);mock.shutdown();mock.server_close()
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0;r=run()
 if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
 print(json.dumps(r))
if __name__=='__main__':main()
