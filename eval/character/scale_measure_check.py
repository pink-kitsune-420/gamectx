"""Retain or audit one local scale timing run (1,000 and 10,000 entries for forty-three character and world modules, with save and load timing for the thirty-four durable ones). No provider access."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,hashlib,json,pathlib,platform,subprocess,provenance
ROOT=pathlib.Path(__file__).resolve().parents[2]
MODULES=['beliefs','speech_act','diet','creed','explain','stability','expectation','attention','aspiration','habit','lesson','affect','skills','body','mind','deception','relationship','offscreen','curiosity','cooperation','cognition','chatter','survival','detail','retention','roles','economy','space','drift','packs','frame','cueing','pragmatics','embodiment','surroundings','fabric','genre','minds','defenses','culture','projects','chemistry','task']
DURABLE=[m for m in MODULES if m not in ('affect','skills','body','mind','deception','relationship','offscreen','curiosity','task')]
SOURCES=['src/character/limits.rs','src/data/persist.rs']+[next(f'src/{d}/{m}.rs' for d in ('character','knowledge') if (ROOT/f'src/{d}/{m}.rs').exists()) for m in MODULES]+['eval/character/scale_measure.rs']
FIELDS=['beings','module','fill_ns_per_op','state_bytes','state_bytes_per_being','query_median_ns','query_p95_ns','query_max_ns']
SAVE_FIELDS=['beings','module','save_bytes','save_bytes_per_being','save_ns','load_ns']
def hashes():return {p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in SOURCES}
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--capture',type=pathlib.Path);p.add_argument('--output',type=pathlib.Path);p.add_argument('--check',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 if a.capture:
  assert a.output and not a.output.exists();raw=json.loads(a.capture.read_text());hardware=dict(cpu=next((l.split(':',1)[1].strip() for l in pathlib.Path('/proc/cpuinfo').read_text().splitlines() if l.startswith('model name')),None),cores=len([l for l in pathlib.Path('/proc/cpuinfo').read_text().splitlines() if l.startswith('processor')]),architecture=platform.machine(),rustc=subprocess.check_output(['rustc','--version'],text=True).strip())
  v=dict(version=1,source_sha256=hashes(),hardware=hardware,measurement=raw,limits='One unpaced local release-mode run, single host thread, uncontrolled OS/desktop scheduling. Per module: fill N beings with one small state-building op each (64-op workflows, requests pre-parsed and excluded), then 1,000 single-op queries spread over the cast (JSON parse excluded, output Value construction included). For each durable module, one save of the filled manager to compact JSON text (save_ns: export plus serialization) and one load of that text into a fresh manager (load_ns: parse, full validation and settle), checked to save back byte for byte; save_bytes is the text length. state_bytes counts live heap growth during the fill via a counting allocator (minimal per-being state; richer beings use more). Excludes JSON parsing/serialization, the gamectx local CLI, cross-module wiring, model calls and engine work; not a whole-game or realtime guarantee.');a.output.write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n')
 else:assert a.check;v=json.loads(a.check.read_text())
 assert provenance.matches(v['source_sha256']);m=v['measurement'];rows=m['rows']
 assert m['actual_model_calls']==0 and m['queries_per_row']==1000 and [(r['beings'],r['module']) for r in rows]==[(n,x) for n in (1000,10000) for x in MODULES]
 s=m['saves'];assert len(DURABLE)==34 and [(r['beings'],r['module']) for r in s]==[(n,x) for n in (1000,10000) for x in DURABLE]
 for r in s:assert list(r)==SAVE_FIELDS and r['save_bytes_per_being']==r['save_bytes']//r['beings'] and r['save_ns']>0 and r['load_ns']>0
 for r in rows:assert list(r)==FIELDS and 0<r['query_median_ns']<=r['query_p95_ns']<=r['query_max_ns'] and r['state_bytes_per_being']==r['state_bytes']//r['beings'] and r['fill_ns_per_op']>0
 print(json.dumps(dict(timing_audited=True,rows=len(rows),saves=len(s),model_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
