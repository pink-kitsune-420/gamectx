#[path = "../tests/support/runtime.rs"]
mod runtime;
use std::io::Read;
fn main() {
    let mut input = String::new();
    std::io::stdin().take(1_048_577).read_to_string(&mut input).unwrap();
    match gamectx::json::parse(&input, 1_048_576).and_then(|v| runtime::process(&v)) {
        Ok(value) => println!("{}", value.to_json()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
