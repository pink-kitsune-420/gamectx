"""Retain or audit one local scale timing run for knowledge modules with host-set limits (1,000 and 10,000 entries; evidence at 256 and 1,024 records; facts at 1,000 and 4,096). No provider access."""
import pathlib as _pl, sys as _sys; _sys.path[1:1] = [str(d) for d in sorted(_pl.Path(__file__).resolve().parents[1].iterdir()) if d.is_dir()]  # every eval/ topic folder
import argparse,hashlib,json,pathlib,platform,subprocess,provenance
ROOT=pathlib.Path(__file__).resolve().parents[2]
MODULES=['places','timeline','things','species','recipes','communities','actions','lore','inventory','info']
ORDER=[(n,m) for n in (1000,10000) for m in MODULES]+[(256,'evidence'),(1024,'evidence'),(1000,'facts'),(4096,'facts')]
SOURCES=['src/character/limits.rs','src/knowledge/codex.rs','src/knowledge/places.rs','src/knowledge/timeline.rs','src/knowledge/things.rs','src/knowledge/species.rs','src/knowledge/recipes.rs','src/knowledge/communities.rs','src/knowledge/action_catalog.rs','src/knowledge/lore.rs','src/knowledge/inventory.rs','src/knowledge/info.rs','src/knowledge/selection.rs','src/knowledge/facts.rs','eval/knowledge/scale_measure.rs']
FIELDS=['entries','module','load_ns_per_entry','state_bytes','state_bytes_per_entry','query_median_ns','query_p95_ns','query_max_ns']
def hashes():return {p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in SOURCES}
def main():
 p=argparse.ArgumentParser();p.add_argument('--cap',type=float,required=True);p.add_argument('--capture',type=pathlib.Path);p.add_argument('--output',type=pathlib.Path);p.add_argument('--check',type=pathlib.Path);a=p.parse_args();assert a.cap==0
 if a.capture:
  assert a.output and not a.output.exists();raw=json.loads(a.capture.read_text());cpu=pathlib.Path('/proc/cpuinfo').read_text().splitlines();hardware=dict(cpu=next((l.split(':',1)[1].strip() for l in cpu if l.startswith('model name')),None),cores=len([l for l in cpu if l.startswith('processor')]),architecture=platform.machine(),rustc=subprocess.check_output(['rustc','--version'],text=True).strip())
  v=dict(version=1,source_sha256=hashes(),hardware=hardware,measurement=raw,limits='One unpaced local release-mode run, single host thread, uncontrolled OS/desktop scheduling. Per row: load N entries from pre-built Values (timed; heap growth counted by a counting allocator), then 1,000 single queries spread over the table (JSON text parsing excluded, output Value construction included). Queries: places nearby (max_distance 8; every field is in one valley, so each answer counts all N siblings), timeline query known_to, things move, species pet, recipes check, communities check_relation, actions describe, lore select, inventory rows, info lookup, evidence select, facts check. Things, species, recipes, communities and actions run their stateless workflow, which returns the whole table on every call, so their per-call cost grows with the table. Facts are loaded by registering one at a time. Excludes the gamectx local CLI, cross-module wiring, model calls and engine work; not a whole-game or realtime guarantee.');a.output.write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n')
 else:assert a.check;v=json.loads(a.check.read_text())
 assert provenance.matches(v['source_sha256']);m=v['measurement'];rows=m['rows']
 assert m['actual_model_calls']==0 and m['queries_per_row']==1000 and [(r['entries'],r['module']) for r in rows]==ORDER
 for r in rows:assert list(r)==FIELDS and 0<r['query_median_ns']<=r['query_p95_ns']<=r['query_max_ns'] and r['state_bytes_per_entry']==r['state_bytes']//r['entries'] and r['load_ns_per_entry']>0
 print(json.dumps(dict(timing_audited=True,rows=len(rows),model_calls=0,api_spend_usd=0)))
if __name__=='__main__':main()
