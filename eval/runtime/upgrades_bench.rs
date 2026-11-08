//! Paired release benchmark; all predictions checked before timing.
use gamectx::{runtime::brain::{Brain,BrainBundle,Prediction},obj,Value};
use std::{hint::black_box,time::Instant};
fn run(perception:&Brain,decision:&Brain,shared:bool,frames:u64)->(u128,i64){
 let mut bundle=BrainBundle::new(perception.clone(),decision.clone(),&Value::Array(vec![obj!{"input"=>"alert","output"=>"alert"}])).unwrap();let mut observed=Prediction::default();let mut prediction=Prediction::default();let mut sum=0;
 let started=Instant::now();
 for frame in 0..frames{
  let value=Some((frame*7919%1000001) as i64);
  let prepared=if shared{Some(bundle.prepare("actor_a",frame,&[value]).unwrap())}else{None};
  for candidate in 0..12{let cost=Some(candidate*7919%1000001);
   if let Some(prepared)=&prepared{bundle.run_into(prepared,"actor_a",frame,&[None,cost],1,0,0,&mut prediction).unwrap()}
   else{perception.run_into(&[value],0,0,0,&mut observed).unwrap();decision.run_into(&[observed.scores()[0],cost],1,0,0,&mut prediction).unwrap()}
   sum+=black_box(prediction.scores()[0].unwrap_or(0));
  }
 }
 (started.elapsed().as_nanos(),sum)
}
fn main(){let mut rows=Vec::new();for hidden in [0,16]{let p=Brain::dense(&["signal"],&["alert"],hidden,9101).unwrap();let d=Brain::dense(&["alert","cost"],&["wait"],hidden,9102).unwrap();assert_eq!(run(&p,&d,false,100).1,run(&p,&d,true,100).1);
 let mut naive=Vec::new();let mut shared=Vec::new();for trial in 0..7{let(a,b)=if trial%2==0{(run(&p,&d,false,1000),run(&p,&d,true,1000))}else{let b=run(&p,&d,true,1000);(run(&p,&d,false,1000),b)};assert_eq!(a.1,b.1);naive.push(a.0 as u64);shared.push(b.0 as u64)}naive.sort();shared.sort();rows.push(obj!{"hidden"=>hidden,"frames"=>1000,"candidates"=>12000,"naive_ns"=>naive[3],"shared_ns"=>shared[3]});}
 println!("{}",(obj!{"trials"=>7,"results"=>rows}).to_json());}
