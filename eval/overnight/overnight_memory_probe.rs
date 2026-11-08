//! Original long-campaign/allocator holdout. No model, file storage service or network.
use gamectx::{character::life::{Config,Definition,Life,Setting},obj,Result,Value};
use std::{alloc::{GlobalAlloc,Layout,System},sync::{Arc,atomic::{AtomicUsize,Ordering}},time::Instant};
struct Alloc;static CALLS:AtomicUsize=AtomicUsize::new(0);static LIVE:AtomicUsize=AtomicUsize::new(0);static PEAK:AtomicUsize=AtomicUsize::new(0);
unsafe impl GlobalAlloc for Alloc {
 unsafe fn alloc(&self,l:Layout)->*mut u8 {let p=System.alloc(l);if !p.is_null(){CALLS.fetch_add(1,Ordering::Relaxed);let n=LIVE.fetch_add(l.size(),Ordering::Relaxed)+l.size();PEAK.fetch_max(n,Ordering::Relaxed);}p}
 unsafe fn dealloc(&self,p:*mut u8,l:Layout){LIVE.fetch_sub(l.size(),Ordering::Relaxed);System.dealloc(p,l)}
 unsafe fn realloc(&self,p:*mut u8,l:Layout,n:usize)->*mut u8{let q=System.realloc(p,l,n);if !q.is_null(){CALLS.fetch_add(1,Ordering::Relaxed);let live=LIVE.fetch_sub(l.size(),Ordering::Relaxed)-l.size()+n;LIVE.fetch_add(n,Ordering::Relaxed);PEAK.fetch_max(live,Ordering::Relaxed);}q}
}
#[global_allocator]static ALLOC:Alloc=Alloc;
fn number(v:&Value,k:&str)->usize{v.get(k).unwrap().as_usize().unwrap()}
fn run()->Result<Value>{
 let pack=gamectx::data::json::parse(include_str!("../../examples/fixtures/cast/chime-town.json"),524288)?;
 let setting=Setting::from_value(pack.get("setting").unwrap())?;let mut people=pack.get("players").unwrap().as_array().unwrap().to_vec();
 for c in pack.get("characters").unwrap().as_array().unwrap(){people.push(c.get("id").unwrap().clone());}
 let people=Value::from(people);let legacy=std::env::args().any(|a|a=="--legacy");let mut restored=0;let mut passed=0;let mut probes=0;let mut durations=Vec::new();let mut checkpoints=Vec::new();let mut retained_bytes=Vec::new();let start_calls=CALLS.load(Ordering::Relaxed);
 for character in pack.get("characters").unwrap().as_array().unwrap(){
  let actor=character.get("id").unwrap().as_str().unwrap();let d=Arc::new(Definition::draft(character.get("definition").unwrap(),&setting)?);let cfg=Config::resolve(&obj!{"enabled"=>true},&obj!{})?;
  let fresh=||Life::accept("deep_campaign",actor,&people,d.clone(),cfg.clone());let mut life=fresh()?;let id=obj!{"campaign"=>"lantern_holdout","branch"=>"chapter_a","instance"=>actor};
  for turn in 1..=1000usize{
   let (subject,tag,text,causes,retain)=match turn {
    1=>("player_1","preference","player_1 prefers amber wicks.",vec![],true),
    2=>("player_1","name","player_1 is named Vira.",vec![],true),
    3=>("player_1","correction","player_1 corrected the name to Vela, not Vira.",vec![2],true),
    4=>("player_1","promise","player_1 promises to return the borrowed lamp; unresolved.",vec![],true),
    5=>("player_1","pivotal","player_1 rescued the merchant during a stair collapse.",vec![],true),
    6=>("player_2","preference","player_2 prefers blue wicks.",vec![],true),
    7=>("player_2","name","player_2 is named Toma.",vec![],true),
    8=>("player_2","promise","player_2 promises to deliver chalk; unresolved.",vec![],true),
    _=>(if turn%2==0{"player_1"}else{"player_2"},"chatter","Routine discussion of the counter and room.",vec![],false)
   };
   let e=obj!{"token"=>turn,"at"=>turn,"kind"=>"direct","tag"=>tag,"subject"=>subject,"source"=>obj!{"id"=>format!("receipt_{turn}"),"speaker"=>subject,"text"=>"Host-confirmed heard statement."},"audience"=>people.clone(),"text"=>text,"causes"=>causes,"retain"=>retain};
   let start=Instant::now();life.experience(&e)?;durations.push(start.elapsed().as_nanos() as usize);
   if turn%100==0{
    life.consolidate(turn as u64-99,turn as u64)?;life.acknowledge_archive(turn as u64)?;life.evict(turn as u64)?;
    let save=life.checkpoint(&id)?;let mut next=fresh()?;next.restore_checkpoint(&id,&save,&obj!{"min_revision"=>0,"max_revision"=>u32::MAX as usize,"min_token"=>0,"max_token"=>turn,"max_tick"=>turn})?;life=next;restored+=1;
    for (subject,required) in [("player_1",vec![1,2,3,4,5]),("player_2",vec![6,7,8])]{
     let recalled=if legacy {life.recall_episodes("amber Vela blue Toma promise",&vec!["preference","promise"].into(),8)?}else{life.recall_selected(&obj!{"input"=>"amber Vela blue Toma promise","tags"=>Vec::<Value>::new(),"subjects"=>vec![subject],"required"=>required.clone(),"count"=>required.len(),"archive"=>true})?};
     let rows=recalled.as_array().unwrap();probes+=1;if rows.len()==required.len()&&required.iter().all(|n|rows.iter().any(|e|number(e,"token")==*n))&&rows.iter().all(|e|e.get("subject").and_then(Value::as_str)==Some(subject)){passed+=1;}
    }
    if actor=="merchant"{checkpoints.push(obj!{"turn"=>turn,"checkpoint_bytes"=>save.to_json().len(),"working_events"=>number(&life.memory_stats(),"events")});}
   }
  }
  retained_bytes.push(LIVE.load(Ordering::Relaxed));
 }
 durations.sort_unstable();Ok(obj!{"version"=>1,"mode"=>if legacy{"legacy"}else{"selected"},"characters"=>4,"turns_per_character"=>1000,"events"=>4000,"save_restores"=>restored,"scoped_anchor_probes"=>probes,"passed_probes"=>passed,"median_experience_ns"=>durations[durations.len()/2],"p95_experience_ns"=>durations[durations.len()*95/100],"allocator_calls"=>CALLS.load(Ordering::Relaxed)-start_calls,"peak_live_allocated_bytes"=>PEAK.load(Ordering::Relaxed),"live_bytes_after_character"=>retained_bytes,"merchant_checkpoints"=>checkpoints,"actual_provider_calls"=>0,"api_spend_usd"=>0})
}
fn main(){println!("{}",run().unwrap().to_json());}
