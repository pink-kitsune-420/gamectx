"""Explicit delayed request/player-switch race. Local HTTP only, no provider calls."""
import argparse,http.client,json,pathlib,re,subprocess,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[1]
def run(expect):
 with tempfile.TemporaryDirectory(prefix='deep-scope-') as d:
  p=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--port','0','--cap','0','--web-state',d],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',p.stdout.readline())[1]);checks=0
   def call(op=None,args=None,generation=None):
    nonlocal checks
    c=http.client.HTTPConnection('127.0.0.1',port,timeout=10);headers={'Content-Type':'application/json','X-Studio-Request':'1'}
    if generation is not None:headers['X-Studio-Generation']=str(generation)
    c.request('POST' if op else 'GET','/api/'+op if op else '/api/state',json.dumps(args or {}) if op else None,headers);r=c.getresponse();body=json.loads(r.read());c.close();checks+=1;return r.status,body
   for target in ['room:gallery','room:workshop']:
    _,s=call();assert call('act',dict(revision=s['game']['revision'],decision=dict(action='go',target=target)))[0]==200
   _,before=call();old=before['generation'];assert call('select',dict(player='player_2',npc='merchant'))[0]==200
   status,result=call('chat',dict(text='buy oil'),old);_,after=call();changed=after['game']['coins']!=before['game']['coins']
   if expect:
    assert status==409 and not changed,(status,after)
    assert call('mic',{},old)[0]==409
    assert call('chat',dict(text='buy oil'),str(after['generation'])+'x')[0]==400
    status,result=call('chat',dict(text='What stock is listed?'),after['generation']);assert status==200 and result['generation']>after['generation']
   else:assert status==200 and changed
   return dict(http_checks=checks,stale_request_status=409 if expect else status,unintended_purchase=changed,active_player=after['player'],actual_provider_calls=0,api_spend_usd=0,limits='Deterministic reordered HTTP requests, not a physical browser/device or multi-controller load test. Generation headers are optimistic concurrency, not authentication; legacy hosts remain responsible for scope.')
  finally:p.terminate();p.wait(timeout=5)
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--expect-rejection',action='store_true');p.add_argument('--output',required=True,type=pathlib.Path);a=p.parse_args();assert a.cap==0 and not a.output.exists();r=run(a.expect_rejection);a.output.write_text(json.dumps(r,indent=2)+'\n');print(json.dumps(r))
if __name__=='__main__':main()
