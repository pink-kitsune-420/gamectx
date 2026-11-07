"""Fresh loopback English-input comfort holdout. No model installed or called."""
import argparse,http.client,json,pathlib,re,subprocess,tempfile
ROOT=pathlib.Path(__file__).resolve().parents[1]
def run():
 checks=0
 with tempfile.TemporaryDirectory(prefix='deep-input-') as d:
  p=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--port','0','--cap','0','--web-state',d],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  try:
   port=int(re.search(r'127\.0\.0\.1:(\d+)',p.stdout.readline())[1])
   def call(op=None,**v):
    nonlocal checks
    c=http.client.HTTPConnection('127.0.0.1',port,timeout=10);c.request('POST' if op else 'GET','/api/'+op if op else '/api/state',json.dumps(v) if op else None,{'Content-Type':'application/json','X-Studio-Request':'1'});r=c.getresponse();body=json.loads(r.read());c.close();checks+=1;assert r.status==200,(op,r.status,body);return body['result'] if op else body
   call('chat',text='walk to the gallery please');assert call()['game']['room']=='room:gallery'
   call('chat',text='travel to the workshop');s=call();assert s['game']['room']=='room:workshop'
   call('chat',text='How much is the oil?');assert 'coins' in call()['speech']['text']
   call('chat',text='buy that one');s=call();assert s['game']['coins']==2
   call('chat',text='Can I buy chalk?');s=call();before=s['game']['revision'];assert s['game']['coins']==2
   call('select',player='player_2',npc='merchant');r=call('chat',text='buy it');assert r['status']=='clarify' and call()['game']['revision']==before
   call('chat',text='I would like to purchase chalk.');assert call()['game']['coins']==1
   for phrase in ['buy oil and chalk','buy oill','buy two oil','buy unlisted ruby']:
    s=call();r=call('chat',text=phrase);assert r['status']=='clarify' and call()['game']==s['game']
   call('chat',text='Do not buy tonic.');assert call()['game']['coins']==1
   call('chat',text='examine the tonic please');s=call();assert s['game']['coins']==1
   call('save',slot=0);session=call('mic')['session'];before=call()['game']['revision'];r=call('recognition',session=session,text='walk to the gallery',final=False,uncertainty='interim');assert r['actions']==[] and call()['game']['revision']==before
   call('recognition',session=session,text='walk to the gallery',final=True,uncertainty=None);assert call()['game']['room']=='room:gallery'
   call('load',slot=0);assert call()['game']['room']=='room:workshop'
   s=call();assert s['pending_jobs']==0 and s['spending']['charged_or_reserved_micros']==0
   return dict(http_checks=checks,natural_phrases=14,unintended_purchases=0,interim_actions=0,actual_provider_calls=0,api_spend_usd=0,limits='One original English grammar/loopback holdout, two players and current legal pairs. Typos and multi-item requests clarify with action buttons; no general natural-language understanding, physical STT or accessibility user study claim.')
  finally:p.terminate();p.wait(timeout=5)
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0;r=run()
 if a.output:assert not a.output.exists();a.output.write_text(json.dumps(r,indent=2)+'\n')
 print(json.dumps(r))
if __name__=='__main__':main()
