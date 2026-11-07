use gamectx::{json,Value};
fn main(){
 let oracle=json::parse(&std::fs::read_to_string("target/validation-oracle.json").unwrap(),65536).unwrap();let mut cases=0;
 for row in oracle.get("number_cases").unwrap().as_array().unwrap(){let text=row.get("text").unwrap().as_str().unwrap();let v=Value::Number(text.into());assert_eq!(v.is_valid(65536),row.get("valid").unwrap().as_bool().unwrap());cases+=1;}
 for width in [0,1,32,33,1024,2049,20000]{let entries=(0..width).map(|i|(format!("key{i}"),Value::str("λ\n"))).collect::<Vec<_>>();let v=Value::Object(entries.clone());assert!(v.is_valid(v.json_len()));assert!(!v.is_valid(v.json_len()-1));if width>0{let mut entries=entries;entries.push(entries[0].clone());assert!(!Value::Object(entries).is_valid(1<<22));}cases+=1;}
 for depth in 0..=66{let mut v=Value::Null;for _ in 0..depth{v=vec![v].into();}assert_eq!(v.is_valid(65536),depth<=json::MAX_DEPTH);cases+=1;}
 println!("{{\"checks\":{cases},\"model_calls\":0,\"api_spend_usd\":0}}");
}
