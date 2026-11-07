"""Original stage-action prototype expectations against the real offline host."""
import argparse,http.client,json,pathlib,re,subprocess,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[1]
def run():
 with tempfile.TemporaryDirectory(prefix='greeting-actions-')as tmp:
  d=pathlib.Path(tmp);mind=json.loads((ROOT/'examples/dungeon/web/personalities.json').read_text());mind['enabled']=True;(d/'personalities.json').write_text(json.dumps(mind));host=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--port','0','--cap','0','--web-state',str(d)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True);checks=0
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',host.stdout.readline())[1])
   def call(op=None,error=False,**args):
    nonlocal checks
    h=http.client.HTTPConnection('127.0.0.1',port,timeout=5);h.request('POST'if op else'GET','/api/'+op if op else'/api/state',json.dumps(args)if op else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=h.getresponse();v=json.loads(r.read());h.close();checks+=1;assert (r.status==400)==error,(r.status,v);return v.get('result')if op else v
   def finish(s):
    sid=s['speech']['id'];n=len(s['speech']['text']);call('delivery',id=sid,event='queued',through=0,heard=0);call('delivery',id=sid,event='played',through=n,heard=n);call('delivery',id=sid,event='finished',through=n,heard=n)
   r=call('chat',text='*enters the gallery*');assert r['status']=='accepted';before=call()['game'];r=call('chat',text='*enters the vault*');assert r['status']=='rejected'and call()['game']==before
   r=call('chat',text='*enters the workshop*');assert r['status']=='accepted';s=call();assert'Ilen'in s['speech']['text']and not s['pending_jobs'];assert any(r['kind']=='action'and'workshop'in r['text']for r in s['transcript']);assert not any(r['kind']=='player'for r in s['transcript']);sid=s['speech']['id'];count=len(s['transcript']);r=call('approach',npc='merchant');assert r['already_presented'];s=call();assert s['speech']['id']==sid and len(s['transcript'])==count;finish(s)
   r=call('chat',text='*approaches the merchant*');assert r['greeting']['meeting']=='continuing';s=call();assert'Ilen'not in s['speech']['text'];assert s['transcript'][-1]['kind']=='action';finish(s)
   before=call()['game'];call('approach',npc='keeper',error=True);assert call()['game']==before
   r=call('chat',text='*enters the vault and takes the glass star*');assert r['stage']=='unsupported'and call()['game']==before
   r=call('mic');mic=r['mic']if'mic'in r else call()['mic'];call('recognition',session=mic,text='*enters the gallery*',final=False,uncertainty='Original interim fixture.');assert call()['game']==before
   call('recognition',session=mic,text='*enters the gallery*',final=True,uncertainty='Original final fixture.');assert call()['game']['room']=='room:gallery'
   # A travel button follows the same accepted-action narration/automatic greeting path.
   s=call();call('act',revision=s['game']['revision'],decision=dict(action='go',target='room:workshop'));s=call();assert'back'in s['speech']['text'].lower()and'Ilen'not in s['speech']['text'];assert not any(r['kind']=='player'for r in s['transcript']);assert s['spending']['charged_or_reserved_micros']==0
   return dict(workflows=7,http_checks=checks,entry_auto_greeting=True,no_fabricated_player_hello=True,approach_reuses_pending_greeting=True,blocked_vault_stays_locked=True,absent_npc_rejected=True,arbitrary_stage_effects_rejected=True,interim_stage_action_executed=False,final_stage_action_validated=True,actual_provider_calls=0,api_spend_usd=0,limits='Deterministic example host, text/recognition fixtures. No physical movement/microphone/audio or model quality claim.')
  finally:host.terminate();host.wait(timeout=5)
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0;r=run()
 if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
 print(json.dumps(r))
if __name__=='__main__':main()
