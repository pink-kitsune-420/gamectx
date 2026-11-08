"""Offline end-to-end loopback checks. No model/provider service is contacted."""
import argparse,base64,http.client,json,pathlib,re,struct,subprocess,tempfile,zlib
ROOT=pathlib.Path(__file__).resolve().parents[2]
def png():
 def chunk(k,v):return struct.pack('>I',len(v))+k+v+struct.pack('>I',zlib.crc32(k+v))
 raw=(b'\x00'+b'\x34\x62\x73\xff'*512)*512
 return b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',512,512,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(raw))+chunk(b'IEND',b'')
def main():
 parser=argparse.ArgumentParser();parser.add_argument('--cap',type=float,required=True);args=parser.parse_args();assert args.cap==0
 checks=0
 with tempfile.TemporaryDirectory(prefix='gamectx-studio-') as d:
  p=subprocess.Popen([str(ROOT/'target/debug/examples/dungeon'),'--web','--port','0','--cap','0','--web-state',d],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
  try:
   line=p.stdout.readline();m=re.search(r'127\.0\.0\.1:(\d+)',line);assert m,(line,p.stderr.read());port=int(m[1])
   def call(op=None,args=None,path='/api/state',origin=None,custom=True,host=None):
    nonlocal checks
    c=http.client.HTTPConnection('127.0.0.1',port,timeout=8);headers={'Content-Type':'application/json'}
    if custom:headers['X-Studio-Request']='1'
    if origin:headers['Origin']=origin
    if host:headers['Host']=host
    c.request('POST' if op else 'GET','/api/'+op if op else path,None if op is None else json.dumps(args or {}),headers);r=c.getresponse();body=r.read();status=r.status;c.close();checks+=1
    return status,json.loads(body) if r.getheader('Content-Type','').startswith('application/json') else body
   def cmd(op,**args):
    status,r=call(op,args);assert status==200,(op,status,r);return r['result']
   def state():
    status,r=call();assert status==200,(status,r);return r
   for path in ['/','/studio.css','/studio.mjs','/voice.mjs']:assert call(path=path)[0]==200
   assert call(path='/no-such-file')[0]==404
   assert call('mic',origin='https://example.invalid')[0]==403
   assert call('mic',custom=False)[0]==403
   assert call(path='/api/state',host='example.invalid')[0]==403
   status,j=call(path='/api/journal');assert status==200 and j['ok'] and len(j['journal'])>=5
   status,t=call(path='/api/timeline');assert status==200 and t['ok'] and len(t['journal'])>=5
   s=state();assert s['game']['room']=='room:entry' and s['spending']['cap_micros']==0 and s['pending_jobs']==0
   assert s['portraits']['selection']['generation'] is None
   def act(action,target):
    s=state();cmd('act',revision=s['game']['revision'],decision=dict(action=action,target=target))
   act('go','room:gallery');act('go','room:workshop');cmd('chat',text='What stock do you sell?');s=state();assert s['npc_present'] and 'hinge oil' in s['speech']['text'];assert s['pending_jobs']==0
   for n in range(3):
    if n:cmd('chat',text='Please tell me about the counter.')
    s=state();id=s['speech']['id'];cmd('delivery',id=id,event='queued',through=0,heard=0);cmd('delivery',id=id,event='played',through=8,heard=5);cmd('delivery',id=id,event='interrupted',through=8,heard=5)
   s=state();assert any(x['id']=='interrupts' for x in s['living']['badges']),s['living'];assert s['speech']['delivery']['heard_text']==s['speech']['text'][:5]
   assert call('delivery',dict(id=id,event='finished',through=len(s['speech']['text']),heard=len(s['speech']['text'])))[0]==400
   before=state()['game']['revision'];session=cmd('mic')['session'];r=cmd('recognition',session=session,text='buy oil',final=False,uncertainty='interim');assert r['status']=='interim' and r['actions']==[];assert state()['game']['revision']==before
   cmd('recognition',session=session,text='What stock is listed?',final=True,uncertainty=None);assert call('recognition',dict(session=session,text='buy oil',final=True,uncertainty=None))[0]==400
   cmd('save',slot=0);saved=state()['game'];act('go','room:gallery');cmd('load',slot=0);assert state()['game']==saved
   s=state();settings={m:s['providers'][m] for m in ['text','image','tts','stt','size']};settings.update(text='online_openai_compatible',image='online_image_api',tts='online_tts_api',stt='online_stt_api',cap_micros=0,voices=s['providers']['voices']);cmd('settings',**settings);cmd('chat',text='What is visible?');assert not cmd('generate',target='portrait')['queued'];s=state();assert s['pending_jobs']==0 and s['spending']['charged_or_reserved_micros']==0
   assert call('settings',dict(settings,cap_micros=1))[0]==400
   for tier in ['1','3','8','30','70']:
    settings['size']=tier;cmd('settings',**settings)
    try:cmd('chat',text='Where was I born?')
    except AssertionError as e:raise AssertionError(('tier',tier,str(e))) from e
    assert 'do not know' in state()['speech']['text']
   cmd('promise');assert any(x['accepted'] for x in state()['promises'])
   act('buy','item:chalk');assert not any(x['accepted'] for x in state()['promises'])
   cmd('select',player='player_2',npc='merchant');act('buy','item:oil');s=state();assert any(a['definition']['subject']=='player_2' and a['definition']['kind']=='trust' and a['level']==2 for a in s['characters'][0]['life']['effective_attitudes'])
   act('go','room:gallery');act('wait',None);assert any(r['hops']>=1 for r in state()['rumors'])
   act('go','room:workshop')
   patch=dict(replace_tokens=[],add_tokens=['uncovered head'],negative_tokens=['wide-brimmed hat'],forbid_props=[],seed_delta=1,asset=None)
   cmd('repair',target='portrait',key=state()['portraits']['selection']['key'],patch=patch,image=base64.b64encode(png()).decode());s=state();assert s['portraits']['selection']['state']=='pinned' and s['portraits']['selection']['generation'] is None
   key=s['portraits']['selection']['asset']['key'];status,data=call(path='/api/asset/'+key+'?digest='+s['portraits']['selection']['asset']['digest']);assert status==200 and data==png()
   assert call(path='/api/asset/../../spending.json')[0] in [400,404]
   print(json.dumps(dict(http_checks=checks,actual_provider_calls=0,api_spend_usd=0,limits='Loopback endpoints and deterministic browser/provider fixtures; no real microphone or installed-model quality evaluation.')))
  finally:p.terminate();p.wait(timeout=5)
if __name__=='__main__':main()
