use gamectx::{
    data::json::{self, Value},
    obj,
};
#[test]
fn in_memory_grammar_depth_and_duplicates() {
    for text in ["0", "-0", "1.50", "1e999999", "-12E-50"] {
        assert!(Value::Number(text.into()).is_valid(65536));
    }
    for text in ["", "+1", "00", "NaN", "true", "[1]", "1 ", "1.", "1e+", "١"] {
        assert!(!Value::Number(text.into()).is_valid(65536));
    }
    let mut deep = Value::Null;
    for _ in 0..65 {
        deep = vec![deep].into();
    }
    assert!(!deep.is_valid(65536));
    let duplicate = Value::Object(vec![("a".into(), Value::Null), ("a".into(), Value::Null)]);
    assert!(!duplicate.is_valid(65536));
    let rows = (0..9000).map(|i| (format!("key{i}"), obj! {"text"=>"λ\"\\\n\u{0001}"})).collect::<Vec<_>>();
    let v = Value::Object(rows.clone());
    assert!(v.is_valid(v.json_len()));
    assert_eq!(json::parse(&v.to_json(), 1 << 22).unwrap(), v);
    let mut duplicate = rows;
    duplicate.push(duplicate[0].clone());
    assert!(!Value::Object(duplicate).is_valid(1 << 22));
    assert!(!v.is_valid(v.json_len() - 1));
}
