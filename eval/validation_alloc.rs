use gamectx::{json,Value};
use std::{alloc::{GlobalAlloc,Layout,System},sync::atomic::{AtomicU64,Ordering},time::Instant};
struct A;static N:AtomicU64=AtomicU64::new(0);
unsafe impl GlobalAlloc for A{unsafe fn alloc(&self,l:Layout)->*mut u8{N.fetch_add(1,Ordering::Relaxed);System.alloc(l)}unsafe fn dealloc(&self,p:*mut u8,l:Layout){System.dealloc(p,l)}unsafe fn realloc(&self,p:*mut u8,l:Layout,n:usize)->*mut u8{N.fetch_add(1,Ordering::Relaxed);System.realloc(p,l,n)}}
#[global_allocator]static ALLOC:A=A;
fn main(){let v=Value::Array((0..128).map(|i|gamectx::obj!{"id"=>i,"text"=>"an attributed event"}).collect());let start=N.load(Ordering::Relaxed);let t=Instant::now();for _ in 0..1000{assert!(std::hint::black_box(&v).is_valid(65536));}let new_ns=t.elapsed().as_nanos();let new_alloc=N.load(Ordering::Relaxed)-start;assert_eq!(new_alloc,0);let start=N.load(Ordering::Relaxed);let t=Instant::now();for _ in 0..1000{assert!(json::parse(&std::hint::black_box(&v).to_json(),65536).is_ok());}println!("{{\"iterations\":1000,\"new_ns\":{new_ns},\"new_allocations\":{new_alloc},\"old_ns\":{},\"old_allocations\":{}}}",t.elapsed().as_nanos(),N.load(Ordering::Relaxed)-start);}
