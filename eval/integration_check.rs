#[path = "../tests/support/integration.rs"]
mod integration;
use std::io::Read;
fn main() {
    let mut text = String::new();
    std::io::stdin().take(4_194_305).read_to_string(&mut text).unwrap();
    match gamectx::json::parse(&text, 4_194_304).and_then(|v| integration::process(&v)) {
        Ok(value) => println!("{}", value.to_json()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
