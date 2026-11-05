//! Deterministic mutation testing: malformed input must produce errors, never panics,
//! and anything that parses must survive packing and serialize back to itself.

use gamectx::json::{parse, Value};
use gamectx::{catalog, schema, table};

const SEEDS: [&str; 5] = [
    r##"[{"id":"a","kind":"x","n":1.50},{"id":"b","kind":"x","n":-2e3}]"##,
    r##"{"entries":[{"kind":"item","id":"lamp","tags":["#lights"]}],"complete":true}"##,
    r##"[{"name":"wait","description":"d","strict":true,"parameters":{"type":"object","properties":{"s":{"type":"integer","minimum":1,"maximum":5}},"required":["s"],"additionalProperties":false}}]"##,
    r##"{"s":"😀 é\n","a":[[[]]],"t":true,"f":false,"z":null}"##,
    "[\"caf\u{e9}\",\"\u{1F600}\"]",
];
const NOISE: &[u8] = b"{}[]\":,\\-+.eE0123456789tfnul \t\nabc#\x01\xc3\xa9";

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn mutate(rng: &mut Rng, seed: &str) -> String {
    let mut bytes = seed.as_bytes().to_vec();
    for _ in 0..=rng.below(4) {
        let at = rng.below(bytes.len() + 1);
        match rng.below(3) {
            0 if at < bytes.len() => {
                bytes.remove(at);
            }
            1 => bytes.insert(at, NOISE[rng.below(NOISE.len())]),
            _ if at < bytes.len() => bytes[at] = NOISE[rng.below(NOISE.len())],
            _ => {}
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn exercise(value: &Value) {
    assert_eq!(parse(&value.to_json(), usize::MAX).as_ref(), Ok(value), "serialization must round-trip");
    if let Some(items) = value.as_array() {
        if let Ok(packed) = table::pack(items, None) {
            assert!(Value::Array(table::unpack(&packed).unwrap()).same(value));
        }
        if let Ok(packed) = table::pack(items, Some("kind")) {
            let restored = Value::Array(table::unpack(&packed).unwrap());
            assert!(restored.same(&Value::Array(table::group_order(items, "kind"))));
        }
        if let Ok(packed) = schema::pack(items) {
            assert!(Value::Array(schema::unpack(&packed).unwrap()).same(value));
            let _ = schema::outline(items);
        }
    }
    if let Ok(c) = catalog::Catalog::from_json(value, catalog::Limits::default()) {
        let _ = c.search("a #b cs", None, 0, 8, None);
    }
}

#[test]
fn mutated_inputs_never_panic() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut parsed = 0;
    for i in 0..40_000 {
        let text = mutate(&mut rng, SEEDS[i % SEEDS.len()]);
        if let Ok(value) = parse(&text, 1 << 16) {
            parsed += 1;
            exercise(&value);
        }
    }
    assert!(parsed > 1000, "mutations should still produce many valid documents ({parsed})");
}

#[test]
fn deep_nesting_is_refused_not_overflowed() {
    let deep = "[".repeat(100_000) + &"]".repeat(100_000);
    assert!(parse(&deep, usize::MAX).is_err());
}

#[test]
fn hostile_unpack_input_errors_cleanly() {
    for text in [
        r##"{"columns":["a"],"rows":[[1,2]]}"##,
        r##"{"columns":["a"],"rows":[[9]],"symbols":[],"symbol_columns":["a"]}"##,
        r##"{"columns":["a","b"],"rows":[[1]]}"##,
        r##"{"columns":"a","rows":[]}"##,
    ] {
        assert!(table::unpack(&parse(text, 1024).unwrap()).is_err(), "{text}");
    }
    for text in [
        r##"{"wrapper":"bare","rows":[["n",null,{"x":{"$ref":"#/$defs/p9"}},null]]}"##,
        r##"{"wrapper":"bare","rows":[["n",null,{},{"parameters":3}]]}"##,
        r##"{"wrapper":"other","rows":[]}"##,
        r##"{"wrapper":"bare","rows":[[1,null,{},null]]}"##,
    ] {
        assert!(schema::unpack(&parse(text, 1024).unwrap()).is_err(), "{text}");
    }
}
