#[path="../../tests/support/facts.rs"]mod facts;
use std::io::Read;
fn main(){let mut input=String::new();std::io::stdin().take(4_194_305).read_to_string(&mut input).unwrap();match gamectx::data::json::parse(&input,4_194_304).and_then(|v|facts::process(&v)){Ok(value)=>println!("{}",value.to_json()),Err(error)=>{eprintln!("{error}");std::process::exit(1);}}}
