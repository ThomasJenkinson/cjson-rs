//! RFC 8259 serialiser.
//!
//! Two output modes:
//!
//! - `serialise`         — compact (no whitespace outside strings), matches
//!                         the semantics of `cJSON_PrintUnformatted`.
//! - `serialise_pretty`  — formatted with newlines and indentation. The safe
//!                         Rust API uses 2-space indent (idiomatic Rust);
//!                         the FFI shim layer will match cJSON's exact byte
//!                         output (tabs) for `cJSON_Print` compatibility.

use crate::value::Value;
use std::fmt::Write as _;

/// Serialise a `Value` as compact JSON text (no whitespace outside strings).
pub fn serialise(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value, None, 0);
    out
}

/// Serialise a `Value` as pretty-printed JSON text (2-space indent).
pub fn serialise_pretty(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value, Some(2), 0);
    out
}

/// `indent`: `None` = compact, `Some(n)` = pretty with `n`-space indent per level.
fn write_value(out: &mut String, value: &Value, indent: Option<usize>, depth: usize) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => write_number(out, *n),
        Value::String(s) => write_string(out, s),
        Value::Array(items) => write_array(out, items, indent, depth),
        Value::Object(members) => write_object(out, members, indent, depth),
    }
}

fn write_number(out: &mut String, n: f64) {
    if n.is_finite() {
        // f64's Display impl emits the shortest representation that round-trips
        // (e.g. 42.0 → "42", 3.14 → "3.14"). Writing to String is infallible.
        let _ = write!(out, "{n}");
    } else {
        // RFC 8259 §6 forbids NaN / Infinity. Emit "null" rather than invalid JSON.
        out.push_str("null");
    }
}

/// RFC 8259 §7: string = quotation-mark *char quotation-mark.
/// Quotation mark, reverse solidus, and U+0000..=U+001F must be escaped.
fn write_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn write_array(out: &mut String, items: &[Value], indent: Option<usize>, depth: usize) {
    if items.is_empty() {
        out.push_str("[]");
        return;
    }
    out.push('[');
    if let Some(step) = indent {
        out.push('\n');
        for (i, item) in items.iter().enumerate() {
            write_indent(out, step, depth + 1);
            write_value(out, item, indent, depth + 1);
            if i + 1 < items.len() {
                out.push(',');
            }
            out.push('\n');
        }
        write_indent(out, step, depth);
    } else {
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            write_value(out, item, indent, depth + 1);
        }
    }
    out.push(']');
}

fn write_object(out: &mut String, members: &[(String, Value)], indent: Option<usize>, depth: usize) {
    if members.is_empty() {
        out.push_str("{}");
        return;
    }
    out.push('{');
    if let Some(step) = indent {
        out.push('\n');
        for (i, (k, v)) in members.iter().enumerate() {
            write_indent(out, step, depth + 1);
            write_string(out, k);
            out.push_str(": ");
            write_value(out, v, indent, depth + 1);
            if i + 1 < members.len() {
                out.push(',');
            }
            out.push('\n');
        }
        write_indent(out, step, depth);
    } else {
        for (i, (k, v)) in members.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            write_string(out, k);
            out.push(':');
            write_value(out, v, indent, depth + 1);
        }
    }
    out.push('}');
}

fn write_indent(out: &mut String, step: usize, depth: usize) {
    for _ in 0..(step * depth) {
        out.push(' ');
    }
}
