"""Fresh paired player cohort with original mocked AI, all work on the host."""
import argparse,http.server,json,pathlib,sys,threading
ROOT=pathlib.Path(__file__).resolve().parents[1];sys.path.insert(0,str(ROOT/'eval'))
from lan_session_host import run

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',type=pathlib.Path);a=p.parse_args();assert a.cap==0;requests=[]
 class Mock(http.server.BaseHTTPRequestHandler):
  def log_message(self,*args):pass
  def do_POST(self):
   assert self.path=='/api/chat';v=json.loads(self.rfile.read(int(self.headers['Content-Length'])));requests.append(v);speech=v['format']['properties']['speech']['enum'][0];b=json.dumps(dict(message=dict(content=json.dumps(dict(speech=speech))))).encode();self.send_response(200);self.send_header('Content-Length',str(len(b)));self.end_headers();self.wfile.write(b)
 server=http.server.ThreadingHTTPServer(('127.0.0.1',0),Mock);threading.Thread(target=server.serve_forever,daemon=True).start()
 try:
  c=json.loads((ROOT/'examples/dungeon/web/providers.json').read_text());c['text']='local_ollama';c['text_models']['local_ollama']['3']='fixture:3b';c['endpoints']['local_ollama']['url']=f'http://127.0.0.1:{server.server_port}/api/chat';r=run(c)
  assert len(requests)==4,len(requests)
  subjects=[]
  for v in requests:
   tail=v['messages'][-1]['content'];subjects.append('player_2'if '[player_2 -> merchant]'in tail else'player_1'if '[player_1 -> merchant]'in tail else'unknown');assert v['options']['num_thread']<=4
  assert subjects==['player_1','player_2','player_1','player_2'],subjects
  result=dict(host_mock_llm_calls=4,request_subjects=subjects,guest_model_calls=0,authorization_workflows=r['workflows'],api_spend_usd=0,actual_provider_calls=0,limits='Original Ollama-compatible mock validates compiled player subjects, authorized reply validation and host routing. No live model quality, physical LAN or hardware latency measurement.')
  if a.output:assert not a.output.exists();a.output.write_text(json.dumps(result,indent=2)+'\n')
  print(json.dumps(result))
 finally:server.shutdown();server.server_close()
if __name__=='__main__':main()
