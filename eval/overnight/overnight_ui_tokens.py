"""Token-first check of the actual focused Studio provider body; local Qwen only."""
import argparse,copy,json,pathlib,sys
ROOT=pathlib.Path(__file__).resolve().parents[2];sys.path[1:1]=[str(d) for d in sorted((ROOT/'eval').iterdir()) if d.is_dir()]
import ollama_eval as oe
from overnight_deep import MODEL

def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',required=True,type=float);p.add_argument('--output',required=True,type=pathlib.Path);p.add_argument('--requests',type=pathlib.Path,default=ROOT/'eval/overnight/overnight-tier-requests.json');a=p.parse_args();assert a.cap==0 and not a.output.exists()
 audit=json.loads(a.requests.read_text());body=copy.deepcopy(next(p['request'] for p in audit['profiles'] if p['size']=='3'));output=body['options']['num_predict'];body.update(model=MODEL,keep_alive='10m');body['options'].update(num_predict=1,num_thread=4,num_gpu=0)
 server=oe.Server('http://127.0.0.1:11434',240);m=next(x for x in server.data('/api/tags')['models'] if x['name']==MODEL);oe.require_local_weights(m,server.data('/api/show',dict(model=MODEL)))
 r=dict(version=1,identity=dict(model=MODEL,digest=m['digest']),payload=body,status='reserved',actual_provider_calls=1,api_spend_usd=0,limits='One token-only call of the actual focused Studio body. Not a GUI end-to-end model-quality or larger-tier capability measurement.');oe.checkpoint(a.output,r)
 response=server.chat(body);tokens=response['final']['prompt_eval_count'];r.update(status='complete',response=response,input_tokens=tokens,reserved_output_tokens=output,context=2048,guard_tokens=256,fits=tokens+output+256<=2048);oe.checkpoint(a.output,r);assert r['fits'];print(json.dumps(dict(input_tokens=tokens,reserved_output_tokens=output,fits=True,api_spend_usd=0)))
if __name__=='__main__':main()
