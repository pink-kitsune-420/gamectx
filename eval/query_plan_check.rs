#[path = "../tests/support/query_plan.rs"]
mod query_plan;
use std::io::{self, BufRead};
fn main() {
    for line in io::stdin().lock().lines() {
        let out = gamectx::json::parse(&line.unwrap(), 1 << 20).and_then(|v| query_plan::process(&v));
        println!(
            "{}",
            match out {
                Ok(v) => v.to_json(),
                Err(_) => "ERROR".to_owned(),
            }
        );
    }
}
