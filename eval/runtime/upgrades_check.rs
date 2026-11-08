#[path="../../tests/support/upgrades.rs"]mod upgrades;
use std::io::Read;
fn main(){let mut input=String::new();std::io::stdin().take(1048577).read_to_string(&mut input).unwrap();match gamectx::data::json::parse(&input,1048576).and_then(|v|upgrades::process(&v)){Ok(value)=>println!("{}",value.to_json()),Err(error)=>{eprintln!("{error}");std::process::exit(1);}}}
