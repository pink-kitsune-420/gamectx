"""Offline two-listener, two-player authorization and dialogue isolation holdout."""
import argparse,http.client,json,pathlib,re,subprocess,tempfile,time
ROOT=pathlib.Path(__file__).resolve().parents[2]
def run(config=None):
 with tempfile.TemporaryDirectory(prefix='lantern-lan-')as d:
  extra=[]
  if config is not None:
   cfg=pathlib.Path(d)/'providers.json';cfg.write_text(json.dumps(config));extra=['--web-config',str(cfg)]
  child=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--cap','0','--port','0','--lan','127.0.0.2','--web-state',d,*extra],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);checks=0;cookie=None;generation=1
  try:
   line=child.stdout.readline();port=int(re.search(r'127\.0\.0\.1:(\d+)',line)[1]);info=json.loads(child.stdout.readline().split('LAN invite: ',1)[1])
   def call(op=None,args=None,guest=False,status=200,origin=None,token=True):
    nonlocal checks,cookie,generation
    address='127.0.0.2'if guest else'127.0.0.1';h=http.client.HTTPConnection(address,port,timeout=5);headers={'Content-Type':'application/json','X-Studio-Request':'1'}
    if guest and cookie and token:headers['Cookie']=cookie
    if guest and args is not None and op!='join':headers['X-Studio-Generation']=str(generation)
    if origin:headers['Origin']=origin
    h.request('POST'if args is not None else'GET','/api/'+op if op else'/api/state',json.dumps(args)if args is not None else None,headers);r=h.getresponse();raw=r.read();checks+=1;assert r.status==status,(op,r.status,raw)
    if r.getheader('Set-Cookie'):cookie=r.getheader('Set-Cookie').split(';')[0]
    h.close();value=json.loads(raw)if raw.startswith(b'{')else dict(message=raw.decode());generation=value.get('generation',generation);return value
   def host(op,**args):return call(op,args)['result']
   def guest(op,**args):return call(op,args,True)['result']
   def settled():
    end=time.monotonic()+8
    while time.monotonic()<end:
     s=call()
     if not s['pending_jobs']:return s
     time.sleep(.02)
    raise AssertionError('Model call did not settle')
   def read_host():
    s=settled();reply=s['speech'];n=len(reply['text']);host('delivery',id=reply['id'],event='queued',through=0,heard=0);host('delivery',id=reply['id'],event='finished',through=n,heard=n)
   call('guest',guest=True,status=401);call('state',guest=True,status=401)
   call('join',dict(code='incorrect'),True,403);call('join',dict(code=info['code']),True);g=call('guest',guest=True);assert g['player']=='player_2'and g['transcript']==[]and'setup'not in g and'characters'not in g
   for op in ['state','session']:call(op,guest=True,status=403)
   for op in ['settings','manager_action','reset','act','delivery','synthesize']:call(op,{},True,403)
   call('guest_chat',dict(text='Hello.',npc='merchant'),True,403,origin='http://untrusted.invalid')
   for room in ['gallery','workshop']:s=call();host('act',revision=s['game']['revision'],decision=dict(action='go',target='room:'+room))
   host('chat',text='Hello merchant.');s=call();assert s['speech']['player']=='player_1';sid=s['speech']['id'];call('guest_read',dict(id=sid),True,400);call('guest_chat',dict(text='Hello.',npc='merchant'),True,400);read_host()
   call('guest_chat',dict(text='buy oil',npc='merchant'),True,400);before=call()['game'];call('guest_chat',dict(text='Hello, merchant.',npc='merchant',player='player_1'),True,400)
   guest('guest_chat',text='Hello, merchant.',npc='merchant');s=settled();assert s['player']=='player_1'and s['speech']['player']=='player_2'and s['game']['coins']==before['coins'];g=call('guest',guest=True);assert g['floor']=='player_2'and g['reply']['actor']=='merchant'
   call('chat',dict(text='Host tries overlap.'),False,409);call('delivery',dict(id=g['reply']['id'],event='queued',through=0,heard=0),False,409);guest('guest_read',id=g['reply']['id']);assert call('guest',guest=True)['floor']is None
   host('chat',text='Hello again, merchant.');read_host();g=call('guest',guest=True);speakers={r['speaker']for r in g['transcript']};assert {'player_1','player_2','merchant'}<=speakers;assert any(r['speaker']=='merchant'and r['recipient']=='player_2'for r in g['transcript']);assert any(r['speaker']=='merchant'and r['recipient']=='player_1'for r in g['transcript'])
   guest('guest_chat',text='One more greeting.',npc='merchant');settled();host('session_rotate');call('guest',guest=True,status=401);assert call()['speech']['delivery']['done'];new=call('session');assert new['code']!=info['code'];call('join',dict(code=info['code']),True,403,token=False)
   call('join',dict(code=new['code']),True,token=False);assert call('guest',guest=True)['transcript']==[]
   assert call()['spending']['charged_or_reserved_micros']==0
   return dict(http_checks=checks,workflows=10,shared_speakers=sorted(speakers),guest_identity='player_2',host_identity='player_1',guest_model_calls=0,api_spend_usd=0,actual_provider_calls=0,cases=['join required','code validation','host-only state and controls','cross-origin rejection','fixed guest identity','host and guest floor isolation','guest world actions denied','attributed shared NPC dialogue','token rotation and floor recovery','new guest excludes previous transcript'],limits='Real host HTTP on isolated loopback interfaces with 0B replies. Does not measure real Wi-Fi reliability, TLS, NAT traversal, physical microphones or live model quality.')
  finally:child.terminate();child.wait(timeout=5)
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0;r=run()
 if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
 print(json.dumps(r))
if __name__=='__main__':main()
