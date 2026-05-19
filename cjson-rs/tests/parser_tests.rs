//! RFC 8259-driven parser tests.
//!
//! Each test cites its spec section. Written before the parser
//! implementation — see commit history.

use cjson_rs::{parse, parse_prefix, parse_with_options, Error, ParserOptions, Value};

// ===== RFC 8259 §3: any value can be a top-level JSON text =====

#[test]
fn parse_top_level_null() {
    assert_eq!(parse(b"null").unwrap(), Value::Null);
}

#[test]
fn parse_top_level_true() {
    assert_eq!(parse(b"true").unwrap(), Value::Bool(true));
}

#[test]
fn parse_top_level_false() {
    assert_eq!(parse(b"false").unwrap(), Value::Bool(false));
}

#[test]
fn parse_top_level_number() {
    assert_eq!(parse(b"42").unwrap(), Value::Number(42.0));
}

#[test]
fn parse_top_level_string() {
    assert_eq!(parse(b"\"hi\"").unwrap(), Value::String("hi".into()));
}

#[test]
fn parse_top_level_empty_array() {
    assert_eq!(parse(b"[]").unwrap(), Value::Array(vec![]));
}

#[test]
fn parse_top_level_empty_object() {
    assert_eq!(parse(b"{}").unwrap(), Value::Object(vec![]));
}

// ===== RFC 8259 §4: objects =====

#[test]
fn parse_object_single_member() {
    let got = parse(br#"{"k":1}"#).unwrap();
    let expected = Value::Object(vec![("k".to_string(), Value::Number(1.0))]);
    assert_eq!(got, expected);
}

#[test]
fn parse_object_multiple_members_preserve_order() {
    // RFC 8259 §4: ordering is implementation-defined, but cJSON's linked
    // list and our Vec<(String, Value)> both preserve insertion order.
    let got = parse(br#"{"a":1,"b":2,"c":3}"#).unwrap();
    let expected = Value::Object(vec![
        ("a".to_string(), Value::Number(1.0)),
        ("b".to_string(), Value::Number(2.0)),
        ("c".to_string(), Value::Number(3.0)),
    ]);
    assert_eq!(got, expected);
}

#[test]
fn parse_object_nested() {
    let got = parse(br#"{"outer":{"inner":true}}"#).unwrap();
    let expected = Value::Object(vec![(
        "outer".to_string(),
        Value::Object(vec![("inner".to_string(), Value::Bool(true))]),
    )]);
    assert_eq!(got, expected);
}

#[test]
fn parse_object_duplicate_keys_preserved() {
    // RFC 8259 §4: names *should* be unique, not *must*. cJSON preserves
    // duplicates; we match that behaviour.
    let got = parse(br#"{"a":1,"a":2}"#).unwrap();
    let expected = Value::Object(vec![
        ("a".to_string(), Value::Number(1.0)),
        ("a".to_string(), Value::Number(2.0)),
    ]);
    assert_eq!(got, expected);
}

#[test]
fn parse_object_with_unicode_keys() {
    let got = parse("{\"café\":1,\"日本\":2}".as_bytes()).unwrap();
    let expected = Value::Object(vec![
        ("café".to_string(), Value::Number(1.0)),
        ("日本".to_string(), Value::Number(2.0)),
    ]);
    assert_eq!(got, expected);
}

#[test]
fn parse_object_missing_colon_is_rejected() {
    assert!(parse(br#"{"k"1}"#).is_err());
}

#[test]
fn parse_object_trailing_comma_is_rejected() {
    // RFC 8259 §4 grammar forbids trailing commas.
    assert!(parse(br#"{"a":1,}"#).is_err());
}

#[test]
fn parse_object_missing_close_brace_is_rejected() {
    assert!(matches!(
        parse(br#"{"a":1"#),
        Err(Error::UnexpectedEof { .. })
    ));
}

#[test]
fn parse_object_non_string_key_is_rejected() {
    // RFC 8259 §4: member = string name-separator value
    assert!(parse(br#"{1:"v"}"#).is_err());
}

// ===== RFC 8259 §5: arrays =====

#[test]
fn parse_array_single_element() {
    assert_eq!(
        parse(b"[42]").unwrap(),
        Value::Array(vec![Value::Number(42.0)])
    );
}

#[test]
fn parse_array_multiple_elements() {
    assert_eq!(
        parse(b"[1,2,3]").unwrap(),
        Value::Array(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
        ])
    );
}

#[test]
fn parse_array_mixed_types() {
    let got = parse(br#"[null,true,42,"x",[],{}]"#).unwrap();
    let expected = Value::Array(vec![
        Value::Null,
        Value::Bool(true),
        Value::Number(42.0),
        Value::String("x".to_string()),
        Value::Array(vec![]),
        Value::Object(vec![]),
    ]);
    assert_eq!(got, expected);
}

#[test]
fn parse_array_nested() {
    let got = parse(b"[[1,2],[3,4]]").unwrap();
    let expected = Value::Array(vec![
        Value::Array(vec![Value::Number(1.0), Value::Number(2.0)]),
        Value::Array(vec![Value::Number(3.0), Value::Number(4.0)]),
    ]);
    assert_eq!(got, expected);
}

#[test]
fn parse_array_trailing_comma_is_rejected() {
    assert!(parse(b"[1,]").is_err());
}

#[test]
fn parse_array_missing_close_bracket_is_rejected() {
    assert!(matches!(
        parse(b"[1,2"),
        Err(Error::UnexpectedEof { .. })
    ));
}

#[test]
fn parse_array_missing_comma_is_rejected() {
    assert!(parse(b"[1 2]").is_err());
}

#[test]
fn parse_array_leading_comma_is_rejected() {
    assert!(parse(b"[,1]").is_err());
}

// ===== Mixed nesting =====

#[test]
fn parse_object_with_array_value() {
    let got = parse(br#"{"items":[1,2,3]}"#).unwrap();
    let expected = Value::Object(vec![(
        "items".to_string(),
        Value::Array(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
        ]),
    )]);
    assert_eq!(got, expected);
}

#[test]
fn parse_array_of_objects() {
    let got = parse(br#"[{"a":1},{"b":2}]"#).unwrap();
    let expected = Value::Array(vec![
        Value::Object(vec![("a".to_string(), Value::Number(1.0))]),
        Value::Object(vec![("b".to_string(), Value::Number(2.0))]),
    ]);
    assert_eq!(got, expected);
}

#[test]
fn parse_deeply_nested_within_limit() {
    // 100 levels of nesting — well within the 1000 default limit.
    let mut s = String::new();
    for _ in 0..100 {
        s.push('[');
    }
    s.push('1');
    for _ in 0..100 {
        s.push(']');
    }
    assert!(parse(s.as_bytes()).is_ok());
}

#[test]
fn parse_nesting_at_default_limit_is_rejected() {
    // Build 1001 deep — must exceed the 1000 default.
    let mut s = String::new();
    for _ in 0..1001 {
        s.push('[');
    }
    s.push('1');
    for _ in 0..1001 {
        s.push(']');
    }
    assert!(matches!(
        parse(s.as_bytes()),
        Err(Error::NestingLimitExceeded { .. })
    ));
}

#[test]
fn parse_with_custom_nesting_limit() {
    let opts = ParserOptions { nesting_limit: 5 };
    // 4 levels: within
    assert!(parse_with_options(b"[[[[1]]]]", &opts).is_ok());
    // 6 levels: over
    assert!(matches!(
        parse_with_options(b"[[[[[[1]]]]]]", &opts),
        Err(Error::NestingLimitExceeded { .. })
    ));
}

// ===== Whitespace =====

#[test]
fn parse_whitespace_around_values_is_ignored() {
    assert_eq!(parse(b"   null   ").unwrap(), Value::Null);
    assert_eq!(
        parse(b"\n\t[\n  1,\n  2\n]\n").unwrap(),
        Value::Array(vec![Value::Number(1.0), Value::Number(2.0)])
    );
}

// ===== Empty / trailing input =====

#[test]
fn parse_empty_input_is_rejected() {
    assert!(matches!(parse(b""), Err(Error::UnexpectedEof { .. })));
}

#[test]
fn parse_whitespace_only_input_is_rejected() {
    assert!(matches!(parse(b"   "), Err(Error::UnexpectedEof { .. })));
}

#[test]
fn parse_trailing_data_after_value_is_rejected() {
    // Any trailing non-whitespace after the JSON value is rejected. The
    // specific error variant depends on whether the trailing bytes
    // tokenise cleanly or not — both are valid rejections.
    assert!(parse(b"null garbage").is_err());
}

#[test]
fn parse_trailing_value_yields_trailing_data() {
    // Trailing tokens that DO tokenise cleanly must yield TrailingData.
    assert!(matches!(
        parse(b"null []"),
        Err(Error::TrailingData { .. })
    ));
}

#[test]
fn parse_two_top_level_values_is_rejected() {
    assert!(matches!(
        parse(b"{} {}"),
        Err(Error::TrailingData { .. })
    ));
}

#[test]
fn parse_trailing_whitespace_is_allowed() {
    // Whitespace after the value is fine; only non-WS data is "trailing".
    assert_eq!(parse(b"null   \n").unwrap(), Value::Null);
}

// ===== Number value plumbing =====

#[test]
fn parse_negative_number() {
    assert_eq!(parse(b"-3.14").unwrap(), Value::Number(-3.14));
}

#[test]
fn parse_zero() {
    assert_eq!(parse(b"0").unwrap(), Value::Number(0.0));
}

// ===== String value plumbing =====

#[test]
fn parse_string_with_escape() {
    assert_eq!(
        parse(br#""line1\nline2""#).unwrap(),
        Value::String("line1\nline2".to_string())
    );
}

#[test]
fn parse_string_with_unicode_escape() {
    // RFC 8259 §7: \uXXXX escape for U+00E9 LATIN SMALL LETTER E WITH ACUTE.
    // Input bytes are pure ASCII: " \ u 0 0 E 9 "
    assert_eq!(
        parse(b"\"\\u00E9\"").unwrap(),
        Value::String("é".to_string())
    );
}

#[test]
fn parse_string_with_direct_utf8() {
    // RFC 8259 §7: any Unicode character may also appear unescaped.
    assert_eq!(
        parse("\"é\"".as_bytes()).unwrap(),
        Value::String("é".to_string())
    );
}

// ===== parse_prefix: backs cJSON_ParseWithOpts(require_null_terminated=false) =====

#[test]
fn parse_prefix_stops_after_first_value_with_trailing_garbage() {
    // Upstream cJSON parse_with_opts_should_return_parse_end:
    // "[] empty array XD" → returns []; parse_end points to byte 2 (the space).
    let (v, end) = parse_prefix(b"[] empty array XD").unwrap();
    assert_eq!(v, Value::Array(vec![]));
    assert_eq!(end, 2);
}

#[test]
fn parse_prefix_on_clean_input_consumes_everything() {
    let (v, end) = parse_prefix(br#"{"a":1}"#).unwrap();
    assert!(matches!(v, Value::Object(_)));
    assert_eq!(end, 7);
}

#[test]
fn parse_prefix_with_trailing_whitespace_stops_after_value() {
    // Trailing whitespace is *not* part of the value — parse_end should point
    // right after `]`, not after the whitespace.
    let (v, end) = parse_prefix(b"[]   \n\t").unwrap();
    assert_eq!(v, Value::Array(vec![]));
    assert_eq!(end, 2);
}

#[test]
fn parse_prefix_with_second_valid_value_stops_after_first() {
    // "[1,2] 3" — `3` is a valid JSON number too, but it belongs to the
    // caller's trailing data; parse_prefix returns only the array.
    // parse_end points right after `]` (byte 5), not past the trailing space.
    let (v, end) = parse_prefix(b"[1,2] 3").unwrap();
    if let Value::Array(items) = v {
        assert_eq!(items.len(), 2);
    } else {
        panic!("expected array");
    }
    assert_eq!(end, 5);
}

#[test]
fn parse_prefix_incomplete_value_errors() {
    // "[1, 2" — no closing bracket — must error, even though garbage-trailing
    // is normally allowed. parse_prefix returns the first value, but there
    // isn't one.
    assert!(parse_prefix(b"[1, 2").is_err());
}

#[test]
fn parse_prefix_pure_garbage_errors() {
    assert!(parse_prefix(b"junk").is_err());
}

#[test]
fn parse_prefix_number_then_garbage() {
    let (v, end) = parse_prefix(b"42 not json").unwrap();
    assert_eq!(v, Value::Number(42.0));
    assert_eq!(end, 2);
}

#[test]
fn parse_prefix_string_then_garbage() {
    let (v, end) = parse_prefix(b"\"hi\" garbage").unwrap();
    assert_eq!(v, Value::String("hi".into()));
    assert_eq!(end, 4);
}
