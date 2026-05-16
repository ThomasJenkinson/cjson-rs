//! Serialiser tests — RFC 8259-driven, with round-trip coverage.
//!
//! Each test cites its spec section. Written before the implementation;
//! see commit history.

use cjson_rs::{parse, serialise, serialise_pretty, Value};

// ===== RFC 8259 §3: scalar values =====

#[test]
fn serialise_null() {
    assert_eq!(serialise(&Value::Null), "null");
}

#[test]
fn serialise_true() {
    assert_eq!(serialise(&Value::Bool(true)), "true");
}

#[test]
fn serialise_false() {
    assert_eq!(serialise(&Value::Bool(false)), "false");
}

#[test]
fn serialise_integer_number() {
    // Numbers stored as f64; integers round-trip without ".0" decoration.
    assert_eq!(serialise(&Value::Number(42.0)), "42");
}

#[test]
fn serialise_negative_number() {
    assert_eq!(serialise(&Value::Number(-17.0)), "-17");
}

#[test]
fn serialise_fractional_number() {
    assert_eq!(serialise(&Value::Number(3.14)), "3.14");
}

#[test]
fn serialise_zero() {
    assert_eq!(serialise(&Value::Number(0.0)), "0");
}

#[test]
fn serialise_nan_emits_null() {
    // RFC 8259 §6 forbids NaN/Infinity. Match cJSON: emit "null" rather
    // than producing invalid JSON.
    assert_eq!(serialise(&Value::Number(f64::NAN)), "null");
}

#[test]
fn serialise_infinity_emits_null() {
    assert_eq!(serialise(&Value::Number(f64::INFINITY)), "null");
    assert_eq!(serialise(&Value::Number(f64::NEG_INFINITY)), "null");
}

// ===== RFC 8259 §7: strings =====

#[test]
fn serialise_empty_string() {
    assert_eq!(serialise(&Value::String(String::new())), "\"\"");
}

#[test]
fn serialise_simple_string() {
    assert_eq!(serialise(&Value::String("hello".into())), "\"hello\"");
}

#[test]
fn serialise_string_escapes_quote() {
    assert_eq!(
        serialise(&Value::String("say \"hi\"".into())),
        "\"say \\\"hi\\\"\""
    );
}

#[test]
fn serialise_string_escapes_backslash() {
    assert_eq!(serialise(&Value::String("a\\b".into())), "\"a\\\\b\"");
}

#[test]
fn serialise_string_escapes_newline() {
    assert_eq!(serialise(&Value::String("a\nb".into())), "\"a\\nb\"");
}

#[test]
fn serialise_string_escapes_tab() {
    assert_eq!(serialise(&Value::String("a\tb".into())), "\"a\\tb\"");
}

#[test]
fn serialise_string_escapes_carriage_return() {
    assert_eq!(serialise(&Value::String("a\rb".into())), "\"a\\rb\"");
}

#[test]
fn serialise_string_escapes_other_control_chars() {
    // Control char U+0001 — must be escaped (RFC 8259 §7).
    assert_eq!(
        serialise(&Value::String("\u{0001}".into())),
        "\"\\u0001\""
    );
}

#[test]
fn serialise_string_passes_through_utf8() {
    // RFC 8259 §7: Unicode characters may appear directly in output.
    assert_eq!(serialise(&Value::String("café".into())), "\"café\"");
    assert_eq!(serialise(&Value::String("日本".into())), "\"日本\"");
}

// ===== RFC 8259 §5: arrays (compact) =====

#[test]
fn serialise_empty_array() {
    assert_eq!(serialise(&Value::Array(vec![])), "[]");
}

#[test]
fn serialise_array_single_element() {
    assert_eq!(
        serialise(&Value::Array(vec![Value::Number(1.0)])),
        "[1]"
    );
}

#[test]
fn serialise_array_multiple_elements() {
    assert_eq!(
        serialise(&Value::Array(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
        ])),
        "[1,2,3]"
    );
}

#[test]
fn serialise_array_mixed_types() {
    assert_eq!(
        serialise(&Value::Array(vec![
            Value::Null,
            Value::Bool(true),
            Value::Number(42.0),
            Value::String("x".into()),
        ])),
        r#"[null,true,42,"x"]"#
    );
}

#[test]
fn serialise_nested_arrays() {
    assert_eq!(
        serialise(&Value::Array(vec![
            Value::Array(vec![Value::Number(1.0)]),
            Value::Array(vec![Value::Number(2.0)]),
        ])),
        "[[1],[2]]"
    );
}

// ===== RFC 8259 §4: objects (compact) =====

#[test]
fn serialise_empty_object() {
    assert_eq!(serialise(&Value::Object(vec![])), "{}");
}

#[test]
fn serialise_object_single_member() {
    assert_eq!(
        serialise(&Value::Object(vec![("k".into(), Value::Number(1.0))])),
        r#"{"k":1}"#
    );
}

#[test]
fn serialise_object_multiple_members_preserve_order() {
    assert_eq!(
        serialise(&Value::Object(vec![
            ("a".into(), Value::Number(1.0)),
            ("b".into(), Value::Number(2.0)),
            ("c".into(), Value::Number(3.0)),
        ])),
        r#"{"a":1,"b":2,"c":3}"#
    );
}

#[test]
fn serialise_nested_object() {
    assert_eq!(
        serialise(&Value::Object(vec![(
            "outer".into(),
            Value::Object(vec![("inner".into(), Value::Bool(true))])
        )])),
        r#"{"outer":{"inner":true}}"#
    );
}

// ===== Pretty printing (2-space indent) =====

#[test]
fn pretty_empty_array() {
    assert_eq!(serialise_pretty(&Value::Array(vec![])), "[]");
}

#[test]
fn pretty_empty_object() {
    assert_eq!(serialise_pretty(&Value::Object(vec![])), "{}");
}

#[test]
fn pretty_simple_object() {
    let v = Value::Object(vec![
        ("a".into(), Value::Number(1.0)),
        ("b".into(), Value::Bool(true)),
    ]);
    let expected = "{\n  \"a\": 1,\n  \"b\": true\n}";
    assert_eq!(serialise_pretty(&v), expected);
}

#[test]
fn pretty_simple_array() {
    let v = Value::Array(vec![Value::Number(1.0), Value::Number(2.0)]);
    let expected = "[\n  1,\n  2\n]";
    assert_eq!(serialise_pretty(&v), expected);
}

#[test]
fn pretty_nested_indentation() {
    let v = Value::Object(vec![(
        "outer".into(),
        Value::Array(vec![
            Value::Number(1.0),
            Value::Object(vec![("inner".into(), Value::Bool(false))]),
        ]),
    )]);
    let expected = "{\n  \"outer\": [\n    1,\n    {\n      \"inner\": false\n    }\n  ]\n}";
    assert_eq!(serialise_pretty(&v), expected);
}

// ===== Round-trip: parse(serialise(v)) == v =====

fn roundtrip(v: Value) {
    let s = serialise(&v);
    let parsed = parse(s.as_bytes()).unwrap_or_else(|e| {
        panic!("round-trip failed: serialise produced {s:?}, parse rejected with {e}")
    });
    assert_eq!(parsed, v, "round-trip mismatch for compact output {s:?}");

    let p = serialise_pretty(&v);
    let parsed_pretty = parse(p.as_bytes())
        .unwrap_or_else(|e| panic!("pretty round-trip failed: {p:?} rejected with {e}"));
    assert_eq!(
        parsed_pretty, v,
        "round-trip mismatch for pretty output {p:?}"
    );
}

#[test]
fn roundtrip_scalars() {
    roundtrip(Value::Null);
    roundtrip(Value::Bool(true));
    roundtrip(Value::Bool(false));
    roundtrip(Value::Number(0.0));
    roundtrip(Value::Number(42.0));
    roundtrip(Value::Number(-3.14));
    roundtrip(Value::String("hello".into()));
    roundtrip(Value::String(String::new()));
}

#[test]
fn roundtrip_string_with_escapes() {
    roundtrip(Value::String("contains \"quotes\" and \\ and \n newline".into()));
    roundtrip(Value::String("\t\r\u{0001}".into()));
    roundtrip(Value::String("unicode: café 日本".into()));
}

#[test]
fn roundtrip_complex_structure() {
    let v = Value::Object(vec![
        ("name".into(), Value::String("alice".into())),
        ("age".into(), Value::Number(30.0)),
        ("verified".into(), Value::Bool(true)),
        ("tags".into(), Value::Array(vec![
            Value::String("admin".into()),
            Value::String("user".into()),
        ])),
        ("address".into(), Value::Object(vec![
            ("city".into(), Value::String("London".into())),
            ("postcode".into(), Value::String("SW1A 1AA".into())),
        ])),
        ("metadata".into(), Value::Null),
    ]);
    roundtrip(v);
}

#[test]
fn roundtrip_deep_nesting() {
    let mut v = Value::Number(1.0);
    for _ in 0..50 {
        v = Value::Array(vec![v]);
    }
    roundtrip(v);
}

#[test]
fn roundtrip_empty_containers() {
    roundtrip(Value::Array(vec![]));
    roundtrip(Value::Object(vec![]));
    roundtrip(Value::Array(vec![
        Value::Array(vec![]),
        Value::Object(vec![]),
    ]));
}
