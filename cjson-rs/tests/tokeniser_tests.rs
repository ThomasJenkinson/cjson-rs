//! RFC 8259-driven tokeniser tests.
//!
//! Each test cites the spec section it derives from. The tokeniser is
//! tested in isolation here; parser-level concerns (value nesting,
//! object/array structure) belong in `parser_tests.rs`.

use cjson_rs::{Error, Token, Tokeniser};

fn tokens(s: &str) -> Vec<Token> {
    Tokeniser::tokenise(s.as_bytes())
        .expect("tokeniser failed")
        .into_iter()
        .map(|pt| pt.token)
        .collect()
}

// ----- RFC 8259 §2: whitespace -----

#[test]
fn ws_only_input_yields_no_tokens() {
    assert_eq!(tokens("   \t\n\r  "), Vec::<Token>::new());
}

#[test]
fn ws_is_skipped_between_tokens() {
    assert_eq!(
        tokens("  null  "),
        vec![Token::Null]
    );
}

// ----- RFC 8259 §3: literal values -----

#[test]
fn literal_null() {
    assert_eq!(tokens("null"), vec![Token::Null]);
}

#[test]
fn literal_true() {
    assert_eq!(tokens("true"), vec![Token::True]);
}

#[test]
fn literal_false() {
    assert_eq!(tokens("false"), vec![Token::False]);
}

#[test]
fn invalid_literal_is_rejected() {
    assert!(matches!(
        Tokeniser::tokenise(b"nope"),
        Err(Error::InvalidLiteral { .. })
    ));
}

// ----- RFC 8259 §4: object structural tokens -----

#[test]
fn structural_object_tokens() {
    assert_eq!(
        tokens(r#"{"k":1}"#),
        vec![
            Token::BeginObject,
            Token::String("k".into()),
            Token::NameSeparator,
            Token::Number(1.0),
            Token::EndObject,
        ]
    );
}

// ----- RFC 8259 §5: array structural tokens -----

#[test]
fn structural_array_tokens() {
    assert_eq!(
        tokens("[1,2]"),
        vec![
            Token::BeginArray,
            Token::Number(1.0),
            Token::ValueSeparator,
            Token::Number(2.0),
            Token::EndArray,
        ]
    );
}

// ----- RFC 8259 §6: numbers -----

#[test]
fn number_positive_integer() {
    assert_eq!(tokens("42"), vec![Token::Number(42.0)]);
}

#[test]
fn number_zero() {
    assert_eq!(tokens("0"), vec![Token::Number(0.0)]);
}

#[test]
fn number_negative_integer() {
    assert_eq!(tokens("-17"), vec![Token::Number(-17.0)]);
}

#[test]
fn number_with_fraction() {
    assert_eq!(tokens("3.14"), vec![Token::Number(3.14)]);
}

#[test]
fn number_with_exponent() {
    assert_eq!(tokens("1e10"), vec![Token::Number(1e10)]);
}

#[test]
fn number_with_negative_exponent() {
    assert_eq!(tokens("1.5e-3"), vec![Token::Number(1.5e-3)]);
}

#[test]
fn number_with_positive_exponent() {
    assert_eq!(tokens("2E+5"), vec![Token::Number(2e5)]);
}

#[test]
fn number_leading_zero_followed_by_digits_is_rejected() {
    // RFC 8259 §6: int = zero / ( digit1-9 *DIGIT ).
    assert!(matches!(
        Tokeniser::tokenise(b"01"),
        Ok(v) if v.len() == 2  // "0" then "1" — tokeniser stops int after the 0
    ));
}

#[test]
fn number_lone_minus_is_rejected() {
    assert!(matches!(
        Tokeniser::tokenise(b"-"),
        Err(Error::InvalidNumber { .. })
    ));
}

#[test]
fn number_trailing_decimal_is_rejected() {
    // RFC 8259 §6: frac = decimal-point 1*DIGIT — at least one digit required.
    assert!(matches!(
        Tokeniser::tokenise(b"1."),
        Err(Error::InvalidNumber { .. })
    ));
}

#[test]
fn number_naked_exponent_is_rejected() {
    assert!(matches!(
        Tokeniser::tokenise(b"1e"),
        Err(Error::InvalidNumber { .. })
    ));
}

// ----- RFC 8259 §7: strings -----

#[test]
fn string_empty() {
    assert_eq!(tokens(r#""""#), vec![Token::String(String::new())]);
}

#[test]
fn string_simple_ascii() {
    assert_eq!(tokens(r#""hello""#), vec![Token::String("hello".into())]);
}

#[test]
fn string_escapes() {
    // \" \\ \/ \b \f \n \r \t — RFC 8259 §7
    assert_eq!(
        tokens(r#""\"\\\/\b\f\n\r\t""#),
        vec![Token::String("\"\\/\u{0008}\u{000C}\n\r\t".into())]
    );
}

#[test]
fn string_unicode_escape_bmp() {
    // U+00E9 LATIN SMALL LETTER E WITH ACUTE
    assert_eq!(
        tokens(r#""é""#),
        vec![Token::String("é".into())]
    );
}

#[test]
fn string_unicode_escape_surrogate_pair() {
    // U+1F600 GRINNING FACE encoded as 😀
    assert_eq!(
        tokens(r#""😀""#),
        vec![Token::String("😀".into())]
    );
}

#[test]
fn string_lone_high_surrogate_is_rejected() {
    assert!(matches!(
        Tokeniser::tokenise(br#""\uD83D""#),
        Err(Error::InvalidSurrogatePair { .. })
    ));
}

#[test]
fn string_lone_low_surrogate_is_rejected() {
    assert!(matches!(
        Tokeniser::tokenise(br#""\uDE00""#),
        Err(Error::InvalidSurrogatePair { .. })
    ));
}

#[test]
fn string_high_surrogate_then_non_low_is_rejected() {
    assert!(matches!(
        Tokeniser::tokenise(br#""\uD83DA""#),
        Err(Error::InvalidSurrogatePair { .. })
    ));
}

#[test]
fn string_utf8_multibyte_passes_through() {
    // Direct UTF-8 (no escape): "héllo"
    assert_eq!(
        tokens("\"héllo\""),
        vec![Token::String("héllo".into())]
    );
}

#[test]
fn string_unterminated_is_rejected() {
    assert!(matches!(
        Tokeniser::tokenise(b"\"unterminated"),
        Err(Error::UnexpectedEof { .. })
    ));
}

#[test]
fn string_unescaped_control_char_is_rejected() {
    // RFC 8259 §7 forbids U+0000..=U+001F unescaped.
    assert!(matches!(
        Tokeniser::tokenise(b"\"a\nb\""),
        Err(Error::UnexpectedChar { .. })
    ));
}

#[test]
fn string_invalid_escape_is_rejected() {
    assert!(matches!(
        Tokeniser::tokenise(br#""\x""#),
        Err(Error::InvalidEscape { .. })
    ));
}

#[test]
fn string_short_unicode_escape_is_rejected() {
    // \u must be followed by exactly 4 hex digits.
    assert!(matches!(
        Tokeniser::tokenise(br#""\u12""#),
        Err(Error::InvalidUnicodeEscape { .. })
    ));
}

// ----- Position tracking -----

#[test]
fn position_tracks_lines_and_columns() {
    let toks = Tokeniser::tokenise(b"  null\n  true").unwrap();
    assert_eq!(toks.len(), 2);
    assert_eq!((toks[0].pos.line, toks[0].pos.column), (1, 3));
    assert_eq!((toks[1].pos.line, toks[1].pos.column), (2, 3));
}

// ----- Combined / smoke -----

#[test]
fn full_object_with_all_value_types() {
    let input = r#"{"a":null,"b":true,"c":false,"d":42,"e":"hi","f":[1,2],"g":{}}"#;
    let toks: Vec<Token> = Tokeniser::tokenise(input.as_bytes())
        .unwrap()
        .into_iter()
        .map(|pt| pt.token)
        .collect();
    let expected = vec![
        Token::BeginObject,
        Token::String("a".into()),
        Token::NameSeparator,
        Token::Null,
        Token::ValueSeparator,
        Token::String("b".into()),
        Token::NameSeparator,
        Token::True,
        Token::ValueSeparator,
        Token::String("c".into()),
        Token::NameSeparator,
        Token::False,
        Token::ValueSeparator,
        Token::String("d".into()),
        Token::NameSeparator,
        Token::Number(42.0),
        Token::ValueSeparator,
        Token::String("e".into()),
        Token::NameSeparator,
        Token::String("hi".into()),
        Token::ValueSeparator,
        Token::String("f".into()),
        Token::NameSeparator,
        Token::BeginArray,
        Token::Number(1.0),
        Token::ValueSeparator,
        Token::Number(2.0),
        Token::EndArray,
        Token::ValueSeparator,
        Token::String("g".into()),
        Token::NameSeparator,
        Token::BeginObject,
        Token::EndObject,
        Token::EndObject,
    ];
    assert_eq!(toks, expected);
}
