"""Retain reproducible offline studio checks; never dispatch an installed provider."""
import argparse,hashlib,json,pathlib,subprocess,sys
ROOT=pathlib.Path(__file__).resolve().parents[1]
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--report',type=pathlib.Path);p.add_argument('--check',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 commands=[[sys.executable,'-I','eval/studio_cases.py','--cap','0','--check','eval/studio-results.json'],[sys.executable,'-I','eval/test_studio_web.py','--cap','0'],['node','eval/test_studio_voice.mjs','--cap','0']]
 outputs=[]
 for command in commands:
  run=subprocess.run(command,cwd=ROOT,capture_output=True,text=True);assert run.returncode==0,(command,run.stdout,run.stderr);outputs.append(json.loads(run.stdout.strip().splitlines()[-1]))
 report=dict(version=1,policy=outputs[0],http=outputs[1],browser=outputs[2],frozen_life_model_sha256={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((ROOT/'eval').glob('life-model-*.json'))},actual_provider_calls=0,api_spend_usd=0,provider_tokens=None,limits='Offline deterministic metadata, loopback mock transport and fake browser events. No physical microphone/voice accuracy, image quality, installed runner latency, TLS-provider integration or whole-game frame-rate measurement.')
 if a.report:assert not a.report.exists();a.report.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
 if a.check:assert json.loads(a.check.read_text())==report
 print(json.dumps(report,separators=(',',':')))
if __name__=='__main__':main()
