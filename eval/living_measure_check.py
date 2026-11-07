"""Retain or audit one local host-work timing run. No provider access."""
import argparse,hashlib,json,pathlib,platform,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[1]
SOURCES=['src/cast.rs','src/living.rs','src/living/context.rs','src/living/speech.rs','examples/cast/chime-town.json','eval/living_measure.rs']
def hashes():return {p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in SOURCES}
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--capture',type=pathlib.Path);p.add_argument('--output',type=pathlib.Path);p.add_argument('--check',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 if a.capture:
  assert a.output and not a.output.exists();raw=json.loads(a.capture.read_text());hardware=dict(cpu=next((l.split(':',1)[1].strip() for l in pathlib.Path('/proc/cpuinfo').read_text().splitlines() if l.startswith('model name')),None),architecture=platform.machine(),rustc=subprocess.check_output(['rustc','--version'],text=True).strip())
  v=dict(version=1,source_sha256=hashes(),hardware=hardware,measurement=raw,limits='One unpaced local release-mode trace, single host thread, uncontrolled OS/desktop scheduling; four routines plus initiative, rumor checks and speech queue. 200 simulated worker completions, zero model calls. Excludes renderer/physics, real worker latency and authoritative game callbacks; not a whole-game FPS or realtime guarantee.');a.output.write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n')
 else:assert a.check;v=json.loads(a.check.read_text())
 assert v['source_sha256']==hashes();m=v['measurement'];assert m['frames']==1000 and m['characters']==4 and m['roots']==64 and m['delivered_reports']==128 and m['max_checked_pairs']<=128 and m['actual_model_calls']==0 and m['simulated_completions']==200
 assert 0<=m['median_frame_micros']<=m['p95_frame_micros']<=m['max_frame_micros'] and 0<=m['within_16667_micros']<=1000
 print(json.dumps(dict(timing_audited=True,model_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
