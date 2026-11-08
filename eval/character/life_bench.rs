//! Compare the exact pre-optimization revision with current code, same host workload.
use gamectx::{character::life::{Config,Definition,Life,Setting},data::json,obj,Value};
use std::{alloc::{GlobalAlloc,Layout,System},sync::{Arc,atomic::{AtomicU64,Ordering}},time::Instant};
struct CountAlloc;static ALLOCS:AtomicU64=AtomicU64::new(0);
unsafe impl GlobalAlloc for CountAlloc{unsafe fn alloc(&self,l:Layout)->*mut u8{ALLOCS.fetch_add(1,Ordering::Relaxed);System.alloc(l)}unsafe fn dealloc(&self,p:*mut u8,l:Layout){System.dealloc(p,l)}unsafe fn realloc(&self,p:*mut u8,l:Layout,n:usize)->*mut u8{ALLOCS.fetch_add(1,Ordering::Relaxed);System.realloc(p,l,n)}}
#[global_allocator]static A:CountAlloc=CountAlloc;
fn main(){
 let p=|s:&str|json::parse(s,65536).unwrap();let setting=Setting::from_value(&p(include_str!("../../examples/fixtures/characters/setting.json"))).unwrap();let d=Arc::new(Definition::draft(&p(include_str!("../../examples/fixtures/characters/merchant-life.json")),&setting).unwrap());let mut l=Life::accept("bench","merchant",&vec!["merchant","player"].into(),d,Config::resolve(&obj!{"enabled"=>true},&obj!{}).unwrap()).unwrap();
 let events=(1..=128).map(|n|obj!{"token"=>n,"at"=>n*3,"kind"=>"outcome","tag"=>"returned","subject"=>"player","source"=>obj!{"id"=>format!("receipt:{n}"),"speaker"=>"merchant","text"=>"A lamp was returned intact."},"audience"=>vec!["merchant","player"],"text"=>"A lamp was returned intact.","causes"=>Vec::<Value>::new(),"retain"=>true}).collect::<Vec<_>>();
 let before=ALLOCS.load(Ordering::Relaxed);let t=Instant::now();for e in events{l.experience(&e).unwrap();l.appraise("returned_lamp",e.get("token").unwrap().as_usize().unwrap() as u64,l.revision()).unwrap();}let ns=t.elapsed().as_nanos();let allocations=ALLOCS.load(Ordering::Relaxed)-before;let state=l.inspect().to_json();println!("{{\"events\":128,\"appraisals\":128,\"ns\":{ns},\"allocations\":{allocations},\"bytes\":{},\"fingerprint\":\"{}\"}}",state.len(),gamectx::fingerprint(&state));
}
