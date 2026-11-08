//! Standalone host-work timing; no model, network, sleeps or engine callbacks.
use gamectx::{character::cast::CastPack,character::living::{LivingWorld,SpeechQueue},obj,Result,Value};
use std::{hint::black_box,sync::Arc,time::Instant};
fn run()->Result<Value> {
    let pack=Arc::new(CastPack::parse(include_str!("../../examples/fixtures/cast/chime-town.json"))?);let people=pack.summary().get("participants").unwrap().clone();let mut w=LivingWorld::new(pack,"timing_save")?;
    for n in 1..=64 {w.observe(&obj!{"id"=>format!("return_{n}"),"actor"=>"merchant","event"=>obj!{"token"=>n,"at"=>n,"kind"=>"direct","tag"=>"returned","subject"=>"player_1","source"=>obj!{"id"=>format!("receipt_{n}"),"speaker"=>"merchant","text"=>"A host-confirmed return."},"audience"=>people.clone(),"text"=>"The borrowed lamp was returned intact.","causes"=>Vec::<Value>::new(),"retain"=>true},"significance"=>80,"share_with"=>vec!["keeper","companion"],"appraise"=>"careful_return"})?;}
    let mut queue=SpeechQueue::new();let mut times=Vec::new();let mut reports=0;let mut checks=0;let mut simulated=0;
    for now in 65..1065 {
        let start=Instant::now();
        let frame=w.tick(&obj!{"now"=>now,"windows"=>vec![obj!{"id"=>"tavern_window","location"=>"tavern","start"=>0,"end"=>100000,"participants"=>vec!["guard","merchant","keeper","companion"],"kind"=>"scene"}],"routines"=>vec!["guard","merchant","keeper","companion"].into_iter().map(|actor|obj!{"actor"=>actor,"frame"=>obj!{"revision"=>now,"now"=>now,"conditions"=>Vec::<Value>::new(),"allowed"=>vec!["work"],"emergency"=>false}}).collect::<Vec<_>>()})?;
        reports+=frame.get("reports").unwrap().as_array().unwrap().len();checks=checks.max(frame.get("checked_pairs").unwrap().as_usize().unwrap());
        let proposal=w.consider("guard",&obj!{"revision"=>now,"now"=>now,"turn"=>now,"conditions"=>Vec::<Value>::new(),"allowed"=>vec!["work"],"resources"=>Vec::<Value>::new(),"audience"=>people.clone(),"emergency"=>false,"depth"=>0,"social_remaining"=>1},&vec![obj!{"id"=>"work","kind"=>"activity","priority"=>10,"reason"=>"Resume the watch.","requires"=>Vec::<Value>::new(),"resource"=>Value::Null,"recipient"=>Value::Null,"attitudes"=>Vec::<Value>::new(),"events"=>Vec::<Value>::new(),"promises"=>Vec::<Value>::new()}].into())?;black_box(proposal);
        let ctx=w.speech_context("merchant",1)?;let (h,line)=queue.issue(&ctx,now,10,"One moment, please.")?;black_box(line);
        if now%5==0 {black_box(queue.complete(&h,&ctx,now,r#"{"speech":"I am listening."}"#,|_|true)?);simulated+=1;}
        black_box(queue.freeze(&h,&ctx,now)?);queue.retire(&h,now)?;
        times.push(start.elapsed().as_nanos().div_ceil(1000) as usize);
    }
    times.sort_unstable();
    Ok(obj!{"version"=>1,"frames"=>times.len(),"characters"=>4,"roots"=>64,"delivered_reports"=>reports,"max_checked_pairs"=>checks,"median_frame_micros"=>times[(times.len()-1)/2],"p95_frame_micros"=>times[(times.len()-1)*95/100],"max_frame_micros"=>*times.last().unwrap(),"within_16667_micros"=>times.iter().filter(|t|**t<=16667).count(),"simulated_completions"=>simulated,"actual_model_calls"=>0})
}
fn main() {println!("{}",run().unwrap().to_json());}
