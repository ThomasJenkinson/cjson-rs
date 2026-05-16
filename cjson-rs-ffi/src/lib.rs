// Naming mirrors cJSON.h exactly so existing C consumers find the
// expected symbols; suppress the non-conventional-case lints crate-wide.
#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

//! cjson-rs-ffi — C ABI shim exposing the cJSON.h interface.
//!
//! Every public function:
//! - is `extern "C"` with `#[no_mangle]` and the exact name from cJSON.h
//! - tolerates NULL inputs (returns the documented failure value)
//! - runs inside `panic_guard::guard(...)` so panics never unwind into C
//!
//! All `unsafe` in the project lives in this crate. The safe Rust core
//! (`cjson-rs`) is `#![forbid(unsafe_code)]`.

mod alloc_hooks;
mod convert;
mod panic_guard;
pub mod types;

use crate::convert::{
    alloc_cstr, attach_child, attach_named_child, child_at_index, clamp_to_int, detach,
    duplicate, free_cjson, minify_in_place, new_node, value_to_cjson,
};
use crate::panic_guard::guard;
use crate::types::{
    cJSON, CJSON_ARRAY, CJSON_FALSE, CJSON_INVALID, CJSON_IS_REFERENCE, CJSON_NULL, CJSON_NUMBER,
    CJSON_OBJECT, CJSON_RAW, CJSON_STRING, CJSON_STRING_IS_CONST, CJSON_TRUE,
};
use cjson_rs::{parse, serialise, Value};
use std::cell::Cell;
use std::ffi::{c_char, c_double, c_int, CStr};
use std::ptr;

pub type cJSON_bool = c_int;
const CJSON_BOOL_TRUE: c_int = 1;
const CJSON_BOOL_FALSE: c_int = 0;

const CJSON_VERSION: &str = "cjson-rs 0.0.1\0";

// Thread-local: on parse failure we remember a pointer into the caller's
// input buffer at the byte offset of the error, so `cJSON_GetErrorPtr`
// can return it. The caller is responsible for keeping the input buffer
// alive while inspecting the error pointer (same contract as cJSON).
thread_local! {
    static LAST_ERROR_PTR: Cell<*const c_char> = const { Cell::new(std::ptr::null()) };
}

unsafe fn set_last_error(p: *const c_char) {
    LAST_ERROR_PTR.with(|c| c.set(p));
}

/// Strip a UTF-8 BOM (EF BB BF) prefix if present, returning the
/// (potentially shorter) byte slice and the number of bytes skipped.
/// RFC 8259 §8.1 permits parsers to ignore a leading BOM.
fn strip_utf8_bom(bytes: &[u8]) -> (&[u8], usize) {
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        (&bytes[3..], 3)
    } else {
        (bytes, 0)
    }
}

/// Byte offset of the error within the input, with EOF errors mapped to
/// the end of the input (matching cJSON's `cJSON_GetErrorPtr` semantics).
fn error_byte_offset(e: &cjson_rs::Error, input_len: usize) -> usize {
    match e {
        cjson_rs::Error::UnexpectedEof { .. } => input_len,
        other => other.position().offset,
    }
}

// =============================================================
// Lifecycle: Version, Parse, Delete, Print, PrintUnformatted, free
// =============================================================

/// `cJSON_Version` — returns a static, NUL-terminated version string.
#[no_mangle]
pub extern "C" fn cJSON_Version() -> *const c_char {
    CJSON_VERSION.as_ptr() as *const c_char
}

/// `cJSON_Parse` — parse a NUL-terminated JSON string. Returns NULL on failure.
///
/// # Safety
/// `value` must be a valid NUL-terminated UTF-8 C string, or NULL.
#[no_mangle]
pub unsafe extern "C" fn cJSON_Parse(value: *const c_char) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        if value.is_null() {
            set_last_error(ptr::null());
            return ptr::null_mut();
        }
        let bytes = CStr::from_ptr(value).to_bytes();
        let (parsed_bytes, bom_skip) = strip_utf8_bom(bytes);
        match parse(parsed_bytes) {
            Ok(v) => {
                set_last_error(ptr::null());
                value_to_cjson(&v)
            }
            Err(e) => {
                set_last_error(value.add(bom_skip + error_byte_offset(&e, parsed_bytes.len())));
                ptr::null_mut()
            }
        }
    })
}

/// `cJSON_ParseWithLength` — parse `buffer_length` bytes; not required to be NUL-terminated.
///
/// # Safety
/// `value` must be a valid pointer to at least `buffer_length` bytes, or NULL.
#[no_mangle]
pub unsafe extern "C" fn cJSON_ParseWithLength(
    value: *const c_char,
    buffer_length: usize,
) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        if value.is_null() {
            set_last_error(ptr::null());
            return ptr::null_mut();
        }
        let bytes = std::slice::from_raw_parts(value as *const u8, buffer_length);
        let (parsed_bytes, bom_skip) = strip_utf8_bom(bytes);
        match parse(parsed_bytes) {
            Ok(v) => {
                set_last_error(ptr::null());
                value_to_cjson(&v)
            }
            Err(e) => {
                set_last_error(value.add(bom_skip + error_byte_offset(&e, parsed_bytes.len())));
                ptr::null_mut()
            }
        }
    })
}

/// `cJSON_Delete` — recursively free a cJSON tree.
///
/// # Safety
/// `item` must be NULL or a pointer returned from a `cJSON_*` constructor in this crate.
#[no_mangle]
pub unsafe extern "C" fn cJSON_Delete(item: *mut cJSON) {
    guard((), || free_cjson(item))
}

/// `cJSON_Print` — pretty-print to a malloc'd C string. Caller frees with `cJSON_free` or `free`.
#[no_mangle]
pub unsafe extern "C" fn cJSON_Print(item: *const cJSON) -> *mut c_char {
    print_impl(item, true)
}

/// `cJSON_PrintUnformatted` — compact-print to a malloc'd C string.
#[no_mangle]
pub unsafe extern "C" fn cJSON_PrintUnformatted(item: *const cJSON) -> *mut c_char {
    print_impl(item, false)
}

unsafe fn print_impl(item: *const cJSON, pretty: bool) -> *mut c_char {
    guard(ptr::null_mut(), || {
        if item.is_null() {
            return ptr::null_mut();
        }
        let s = if pretty {
            // Match cJSON's exact pretty format (tabs, `:\t`, newlines
            // around braces but not brackets) for binary compatibility.
            let mut out = String::new();
            write_cjson_pretty(&mut out, item, 0);
            out
        } else {
            let value = match cjson_to_value(item) {
                Some(v) => v,
                None => return ptr::null_mut(),
            };
            serialise(&value)
        };
        let bytes = s.as_bytes();
        let buf = alloc_hooks::hook_malloc(bytes.len() + 1) as *mut c_char;
        if buf.is_null() {
            return ptr::null_mut();
        }
        ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, buf, bytes.len());
        *buf.add(bytes.len()) = 0;
        buf
    })
}

/// Walk a cJSON tree and emit cJSON-compatible pretty-printed text.
///
/// Format rules (derived from observation of upstream `cJSON_Print`
/// output, not from reading cJSON.c):
/// - Tab character (`\t`) per indent level
/// - Object: `{\n<indent+1>"key":\t<value>,\n...\n<indent>}`
/// - Array: `[<value>, <value>, ...]` (no newlines around brackets)
/// - Empty containers collapse to `{}` / `[]`
unsafe fn write_cjson_pretty(out: &mut String, item: *const cJSON, depth: usize) {
    if item.is_null() {
        out.push_str("null");
        return;
    }
    let t = (*item).type_ & 0xFF;
    match t {
        x if x == CJSON_NULL => out.push_str("null"),
        x if x == CJSON_TRUE => out.push_str("true"),
        x if x == CJSON_FALSE => out.push_str("false"),
        x if x == CJSON_NUMBER => {
            let n = (*item).valuedouble;
            if n.is_finite() {
                use std::fmt::Write;
                let _ = write!(out, "{n}");
            } else {
                out.push_str("null");
            }
        }
        x if x == CJSON_STRING => {
            if (*item).valuestring.is_null() {
                out.push_str("\"\"");
            } else if let Ok(s) = CStr::from_ptr((*item).valuestring).to_str() {
                write_escaped(out, s);
            } else {
                out.push_str("\"\"");
            }
        }
        x if x == CJSON_RAW => {
            if !(*item).valuestring.is_null() {
                if let Ok(s) = CStr::from_ptr((*item).valuestring).to_str() {
                    out.push_str(s);
                }
            }
        }
        x if x == CJSON_ARRAY => {
            let first = (*item).child;
            if first.is_null() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            let mut cur = first;
            let mut first_iter = true;
            while !cur.is_null() {
                if !first_iter {
                    out.push_str(", ");
                }
                first_iter = false;
                write_cjson_pretty(out, cur, depth + 1);
                cur = (*cur).next;
            }
            out.push(']');
        }
        x if x == CJSON_OBJECT => {
            let first = (*item).child;
            if first.is_null() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            out.push('\n');
            let mut cur = first;
            let mut first_iter = true;
            while !cur.is_null() {
                if !first_iter {
                    out.push_str(",\n");
                }
                first_iter = false;
                for _ in 0..=depth {
                    out.push('\t');
                }
                if !(*cur).string.is_null() {
                    if let Ok(k) = CStr::from_ptr((*cur).string).to_str() {
                        write_escaped(out, k);
                    } else {
                        out.push_str("\"\"");
                    }
                } else {
                    out.push_str("\"\"");
                }
                out.push_str(":\t");
                write_cjson_pretty(out, cur, depth + 1);
                cur = (*cur).next;
            }
            out.push('\n');
            for _ in 0..depth {
                out.push('\t');
            }
            out.push('}');
        }
        _ => out.push_str("null"),
    }
}

fn write_escaped(out: &mut String, s: &str) {
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
                use std::fmt::Write;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `cJSON_free` — free a pointer that this library allocated.
///
/// # Safety
/// `object` must be NULL or a pointer this library returned.
#[no_mangle]
pub unsafe extern "C" fn cJSON_free(object: *mut std::ffi::c_void) {
    guard((), || {
        if !object.is_null() {
            alloc_hooks::hook_free(object);
        }
    })
}

// =============================================================
// Type predicates
// =============================================================

unsafe fn type_eq(item: *const cJSON, t: c_int) -> cJSON_bool {
    if item.is_null() {
        return CJSON_BOOL_FALSE;
    }
    if ((*item).type_ & 0xFF) == t {
        CJSON_BOOL_TRUE
    } else {
        CJSON_BOOL_FALSE
    }
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_IsInvalid(item: *const cJSON) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || type_eq(item, CJSON_INVALID))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_IsFalse(item: *const cJSON) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || type_eq(item, CJSON_FALSE))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_IsTrue(item: *const cJSON) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || type_eq(item, CJSON_TRUE))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_IsBool(item: *const cJSON) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || {
        if item.is_null() {
            return CJSON_BOOL_FALSE;
        }
        let t = (*item).type_ & 0xFF;
        if t == CJSON_TRUE || t == CJSON_FALSE {
            CJSON_BOOL_TRUE
        } else {
            CJSON_BOOL_FALSE
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_IsNull(item: *const cJSON) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || type_eq(item, CJSON_NULL))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_IsNumber(item: *const cJSON) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || type_eq(item, CJSON_NUMBER))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_IsString(item: *const cJSON) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || type_eq(item, CJSON_STRING))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_IsArray(item: *const cJSON) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || type_eq(item, CJSON_ARRAY))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_IsObject(item: *const cJSON) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || type_eq(item, CJSON_OBJECT))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_IsRaw(item: *const cJSON) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || type_eq(item, CJSON_RAW))
}

// =============================================================
// Navigation + value getters
// =============================================================

#[no_mangle]
pub unsafe extern "C" fn cJSON_GetArraySize(array: *const cJSON) -> c_int {
    guard(0, || {
        if array.is_null() {
            return 0;
        }
        let mut count: c_int = 0;
        let mut cur = (*array).child;
        while !cur.is_null() {
            count = count.saturating_add(1);
            cur = (*cur).next;
        }
        count
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_GetArrayItem(array: *const cJSON, index: c_int) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        if array.is_null() || index < 0 {
            return ptr::null_mut();
        }
        let mut cur = (*array).child;
        let mut i: c_int = 0;
        while !cur.is_null() {
            if i == index {
                return cur;
            }
            cur = (*cur).next;
            i += 1;
        }
        ptr::null_mut()
    })
}

unsafe fn get_object_item_impl(
    object: *const cJSON,
    string: *const c_char,
    case_sensitive: bool,
) -> *mut cJSON {
    if object.is_null() || string.is_null() {
        return ptr::null_mut();
    }
    let key = match CStr::from_ptr(string).to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let mut cur = (*object).child;
    while !cur.is_null() {
        if !(*cur).string.is_null() {
            if let Ok(child_key) = CStr::from_ptr((*cur).string).to_str() {
                let matched = if case_sensitive {
                    child_key == key
                } else {
                    child_key.eq_ignore_ascii_case(key)
                };
                if matched {
                    return cur;
                }
            }
        }
        cur = (*cur).next;
    }
    ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_GetObjectItem(
    object: *const cJSON,
    string: *const c_char,
) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        get_object_item_impl(object, string, false)
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_GetObjectItemCaseSensitive(
    object: *const cJSON,
    string: *const c_char,
) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        get_object_item_impl(object, string, true)
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_HasObjectItem(
    object: *const cJSON,
    string: *const c_char,
) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || {
        if get_object_item_impl(object, string, false).is_null() {
            CJSON_BOOL_FALSE
        } else {
            CJSON_BOOL_TRUE
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_GetStringValue(item: *const cJSON) -> *mut c_char {
    guard(ptr::null_mut(), || {
        if item.is_null() || ((*item).type_ & 0xFF) != CJSON_STRING {
            return ptr::null_mut();
        }
        (*item).valuestring
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_GetNumberValue(item: *const cJSON) -> c_double {
    guard(f64::NAN, || {
        if item.is_null() || ((*item).type_ & 0xFF) != CJSON_NUMBER {
            return f64::NAN;
        }
        (*item).valuedouble
    })
}

// =============================================================
// Internal: cJSON tree → Value (for Print path)
// =============================================================

unsafe fn cjson_to_value(item: *const cJSON) -> Option<Value> {
    if item.is_null() {
        return None;
    }
    let t = (*item).type_ & 0xFF;
    let v = match t {
        x if x == CJSON_NULL => Value::Null,
        x if x == CJSON_TRUE => Value::Bool(true),
        x if x == CJSON_FALSE => Value::Bool(false),
        x if x == CJSON_NUMBER => Value::Number((*item).valuedouble),
        x if x == CJSON_STRING || x == CJSON_RAW => {
            // Raw nodes carry pre-serialised JSON in valuestring. For
            // comparison purposes we treat them as strings; the serialiser
            // path for Raw will be addressed separately.
            if (*item).valuestring.is_null() {
                Value::String(String::new())
            } else {
                let s = CStr::from_ptr((*item).valuestring).to_str().ok()?;
                Value::String(s.to_string())
            }
        }
        x if x == CJSON_ARRAY => {
            let mut items = Vec::new();
            let mut cur = (*item).child;
            while !cur.is_null() {
                items.push(cjson_to_value(cur)?);
                cur = (*cur).next;
            }
            Value::Array(items)
        }
        x if x == CJSON_OBJECT => {
            let mut members = Vec::new();
            let mut cur = (*item).child;
            while !cur.is_null() {
                let key = if (*cur).string.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr((*cur).string).to_str().ok()?.to_string()
                };
                members.push((key, cjson_to_value(cur)?));
                cur = (*cur).next;
            }
            Value::Object(members)
        }
        _ => return None,
    };
    Some(v)
}

// =============================================================
// Constructors (cJSON.h §"create a cJSON item of the appropriate type")
// =============================================================

#[no_mangle]
pub unsafe extern "C" fn cJSON_CreateNull() -> *mut cJSON {
    guard(ptr::null_mut(), || new_node(CJSON_NULL))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_CreateTrue() -> *mut cJSON {
    guard(ptr::null_mut(), || new_node(CJSON_TRUE))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_CreateFalse() -> *mut cJSON {
    guard(ptr::null_mut(), || new_node(CJSON_FALSE))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_CreateBool(b: cJSON_bool) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        let t = if b != 0 { CJSON_TRUE } else { CJSON_FALSE };
        new_node(t)
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_CreateNumber(num: c_double) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        let n = new_node(CJSON_NUMBER);
        if n.is_null() {
            return ptr::null_mut();
        }
        (*n).valuedouble = num;
        (*n).valueint = clamp_to_int(num);
        n
    })
}

unsafe fn create_string_like(s: *const c_char, type_tag: c_int) -> *mut cJSON {
    if s.is_null() {
        return ptr::null_mut();
    }
    let s_str = match CStr::from_ptr(s).to_str() {
        Ok(v) => v,
        Err(_) => return ptr::null_mut(),
    };
    let n = new_node(type_tag);
    if n.is_null() {
        return ptr::null_mut();
    }
    (*n).valuestring = alloc_cstr(s_str);
    if (*n).valuestring.is_null() {
        alloc_hooks::hook_free(n as *mut libc::c_void);
        return ptr::null_mut();
    }
    n
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_CreateString(s: *const c_char) -> *mut cJSON {
    guard(ptr::null_mut(), || create_string_like(s, CJSON_STRING))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_CreateRaw(raw: *const c_char) -> *mut cJSON {
    guard(ptr::null_mut(), || create_string_like(raw, CJSON_RAW))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_CreateArray() -> *mut cJSON {
    guard(ptr::null_mut(), || new_node(CJSON_ARRAY))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_CreateObject() -> *mut cJSON {
    guard(ptr::null_mut(), || new_node(CJSON_OBJECT))
}

// =============================================================
// Typed-array constructors
// =============================================================

unsafe fn create_numeric_array<T, F>(values: *const T, count: c_int, mk: F) -> *mut cJSON
where
    F: Fn(T) -> f64,
    T: Copy,
{
    if values.is_null() || count < 0 {
        return ptr::null_mut();
    }
    let arr = new_node(CJSON_ARRAY);
    if arr.is_null() {
        return ptr::null_mut();
    }
    for i in 0..count {
        let v = *values.offset(i as isize);
        let n = new_node(CJSON_NUMBER);
        if n.is_null() {
            free_cjson(arr);
            return ptr::null_mut();
        }
        let d = mk(v);
        (*n).valuedouble = d;
        (*n).valueint = clamp_to_int(d);
        attach_child(arr, n);
    }
    arr
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_CreateIntArray(numbers: *const c_int, count: c_int) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        create_numeric_array(numbers, count, |v| v as f64)
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_CreateFloatArray(numbers: *const f32, count: c_int) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        create_numeric_array(numbers, count, |v| v as f64)
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_CreateDoubleArray(numbers: *const f64, count: c_int) -> *mut cJSON {
    guard(ptr::null_mut(), || create_numeric_array(numbers, count, |v| v))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_CreateStringArray(
    strings: *const *const c_char,
    count: c_int,
) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        if strings.is_null() || count < 0 {
            return ptr::null_mut();
        }
        let arr = new_node(CJSON_ARRAY);
        if arr.is_null() {
            return ptr::null_mut();
        }
        for i in 0..count {
            let s_ptr = *strings.offset(i as isize);
            let item = create_string_like(s_ptr, CJSON_STRING);
            if item.is_null() {
                free_cjson(arr);
                return ptr::null_mut();
            }
            attach_child(arr, item);
        }
        arr
    })
}

// =============================================================
// Reference constructors — wrap caller-owned data without copying
// =============================================================

#[no_mangle]
pub unsafe extern "C" fn cJSON_CreateStringReference(s: *const c_char) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        if s.is_null() {
            return ptr::null_mut();
        }
        let n = new_node(CJSON_STRING | CJSON_IS_REFERENCE);
        if n.is_null() {
            return ptr::null_mut();
        }
        (*n).valuestring = s as *mut c_char;
        n
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_CreateObjectReference(child: *const cJSON) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        let n = new_node(CJSON_OBJECT | CJSON_IS_REFERENCE);
        if n.is_null() {
            return ptr::null_mut();
        }
        (*n).child = child as *mut cJSON;
        n
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_CreateArrayReference(child: *const cJSON) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        let n = new_node(CJSON_ARRAY | CJSON_IS_REFERENCE);
        if n.is_null() {
            return ptr::null_mut();
        }
        (*n).child = child as *mut cJSON;
        n
    })
}

// =============================================================
// Tree mutation — AddItem* (parents take ownership of `item`)
// =============================================================

#[no_mangle]
pub unsafe extern "C" fn cJSON_AddItemToArray(
    array: *mut cJSON,
    item: *mut cJSON,
) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || {
        if array.is_null() || item.is_null() {
            return CJSON_BOOL_FALSE;
        }
        if ((*array).type_ & 0xFF) != CJSON_ARRAY {
            return CJSON_BOOL_FALSE;
        }
        attach_child(array, item);
        CJSON_BOOL_TRUE
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_AddItemToObject(
    object: *mut cJSON,
    name: *const c_char,
    item: *mut cJSON,
) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || {
        if object.is_null() || name.is_null() || item.is_null() {
            return CJSON_BOOL_FALSE;
        }
        if ((*object).type_ & 0xFF) != CJSON_OBJECT {
            return CJSON_BOOL_FALSE;
        }
        if attach_named_child(object, name, item) {
            CJSON_BOOL_TRUE
        } else {
            CJSON_BOOL_FALSE
        }
    })
}

/// Constant-string variant: borrows the caller's `name` pointer.
#[no_mangle]
pub unsafe extern "C" fn cJSON_AddItemToObjectCS(
    object: *mut cJSON,
    name: *const c_char,
    item: *mut cJSON,
) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || {
        if object.is_null() || name.is_null() || item.is_null() {
            return CJSON_BOOL_FALSE;
        }
        if ((*object).type_ & 0xFF) != CJSON_OBJECT {
            return CJSON_BOOL_FALSE;
        }
        // Free any pre-existing owned key.
        if !(*item).string.is_null() && ((*item).type_ & CJSON_STRING_IS_CONST) == 0 {
            alloc_hooks::hook_free((*item).string as *mut libc::c_void);
        }
        (*item).type_ |= CJSON_STRING_IS_CONST;
        (*item).string = name as *mut c_char;
        attach_child(object, item);
        CJSON_BOOL_TRUE
    })
}

unsafe fn wrap_as_reference(item: *mut cJSON) -> *mut cJSON {
    let r = new_node(((*item).type_ & 0xFF) | CJSON_IS_REFERENCE);
    if r.is_null() {
        return ptr::null_mut();
    }
    (*r).valuestring = (*item).valuestring;
    (*r).valueint = (*item).valueint;
    (*r).valuedouble = (*item).valuedouble;
    (*r).child = (*item).child;
    r
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_AddItemReferenceToArray(
    array: *mut cJSON,
    item: *mut cJSON,
) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || {
        if array.is_null() || item.is_null() {
            return CJSON_BOOL_FALSE;
        }
        if ((*array).type_ & 0xFF) != CJSON_ARRAY {
            return CJSON_BOOL_FALSE;
        }
        let r = wrap_as_reference(item);
        if r.is_null() {
            return CJSON_BOOL_FALSE;
        }
        attach_child(array, r);
        CJSON_BOOL_TRUE
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_AddItemReferenceToObject(
    object: *mut cJSON,
    name: *const c_char,
    item: *mut cJSON,
) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || {
        if object.is_null() || name.is_null() || item.is_null() {
            return CJSON_BOOL_FALSE;
        }
        if ((*object).type_ & 0xFF) != CJSON_OBJECT {
            return CJSON_BOOL_FALSE;
        }
        let r = wrap_as_reference(item);
        if r.is_null() {
            return CJSON_BOOL_FALSE;
        }
        if attach_named_child(object, name, r) {
            CJSON_BOOL_TRUE
        } else {
            free_cjson(r);
            CJSON_BOOL_FALSE
        }
    })
}

// =============================================================
// Convenience helpers (Create + AddItemToObject combinators)
// =============================================================

unsafe fn add_to_object(
    object: *mut cJSON,
    name: *const c_char,
    item: *mut cJSON,
) -> *mut cJSON {
    if item.is_null() {
        return ptr::null_mut();
    }
    if cJSON_AddItemToObject(object, name, item) == CJSON_BOOL_FALSE {
        free_cjson(item);
        return ptr::null_mut();
    }
    item
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_AddNullToObject(
    object: *mut cJSON,
    name: *const c_char,
) -> *mut cJSON {
    guard(ptr::null_mut(), || add_to_object(object, name, cJSON_CreateNull()))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_AddTrueToObject(
    object: *mut cJSON,
    name: *const c_char,
) -> *mut cJSON {
    guard(ptr::null_mut(), || add_to_object(object, name, cJSON_CreateTrue()))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_AddFalseToObject(
    object: *mut cJSON,
    name: *const c_char,
) -> *mut cJSON {
    guard(ptr::null_mut(), || add_to_object(object, name, cJSON_CreateFalse()))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_AddBoolToObject(
    object: *mut cJSON,
    name: *const c_char,
    boolean: cJSON_bool,
) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        add_to_object(object, name, cJSON_CreateBool(boolean))
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_AddNumberToObject(
    object: *mut cJSON,
    name: *const c_char,
    number: c_double,
) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        add_to_object(object, name, cJSON_CreateNumber(number))
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_AddStringToObject(
    object: *mut cJSON,
    name: *const c_char,
    string: *const c_char,
) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        add_to_object(object, name, cJSON_CreateString(string))
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_AddRawToObject(
    object: *mut cJSON,
    name: *const c_char,
    raw: *const c_char,
) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        add_to_object(object, name, cJSON_CreateRaw(raw))
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_AddObjectToObject(
    object: *mut cJSON,
    name: *const c_char,
) -> *mut cJSON {
    guard(ptr::null_mut(), || add_to_object(object, name, cJSON_CreateObject()))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_AddArrayToObject(
    object: *mut cJSON,
    name: *const c_char,
) -> *mut cJSON {
    guard(ptr::null_mut(), || add_to_object(object, name, cJSON_CreateArray()))
}

// =============================================================
// Compare — full implementation
// =============================================================

fn compare_values(a: &Value, b: &Value, case_sensitive: bool) -> bool {
    use cjson_rs::Value::*;
    match (a, b) {
        (Null, Null) => true,
        (Bool(x), Bool(y)) => x == y,
        (Number(x), Number(y)) => x == y,
        (String(x), String(y)) => x == y,
        (Array(x), Array(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|(p, q)| compare_values(p, q, case_sensitive))
        }
        (Object(x), Object(y)) => {
            // cJSON object equality is set-like: each key in `a` must appear
            // in `b` with an equal value, regardless of ordering.
            if x.len() != y.len() {
                return false;
            }
            x.iter().all(|(k1, v1)| {
                y.iter().any(|(k2, v2)| {
                    let keys_match = if case_sensitive {
                        k1 == k2
                    } else {
                        k1.eq_ignore_ascii_case(k2)
                    };
                    keys_match && compare_values(v1, v2, case_sensitive)
                })
            })
        }
        _ => false,
    }
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_Compare(
    a: *const cJSON,
    b: *const cJSON,
    case_sensitive: cJSON_bool,
) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || {
        if a.is_null() || b.is_null() {
            return CJSON_BOOL_FALSE;
        }
        match (cjson_to_value(a), cjson_to_value(b)) {
            (Some(va), Some(vb)) => {
                if compare_values(&va, &vb, case_sensitive != 0) {
                    CJSON_BOOL_TRUE
                } else {
                    CJSON_BOOL_FALSE
                }
            }
            _ => CJSON_BOOL_FALSE,
        }
    })
}

// =============================================================
// ParseWithOpts / ParseWithLengthOpts — full implementation
// =============================================================

#[no_mangle]
pub unsafe extern "C" fn cJSON_ParseWithOpts(
    value: *const c_char,
    return_parse_end: *mut *const c_char,
    _require_null_terminated: cJSON_bool,
) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        if value.is_null() {
            set_last_error(ptr::null());
            if !return_parse_end.is_null() {
                *return_parse_end = ptr::null();
            }
            return ptr::null_mut();
        }
        let bytes = CStr::from_ptr(value).to_bytes();
        let (parsed_bytes, bom_skip) = strip_utf8_bom(bytes);
        match parse(parsed_bytes) {
            Ok(v) => {
                set_last_error(ptr::null());
                if !return_parse_end.is_null() {
                    *return_parse_end = value.add(bytes.len());
                }
                value_to_cjson(&v)
            }
            Err(e) => {
                let err_ptr = value.add(bom_skip + error_byte_offset(&e, parsed_bytes.len()));
                set_last_error(err_ptr);
                if !return_parse_end.is_null() {
                    *return_parse_end = err_ptr;
                }
                ptr::null_mut()
            }
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_ParseWithLengthOpts(
    value: *const c_char,
    buffer_length: usize,
    return_parse_end: *mut *const c_char,
    _require_null_terminated: cJSON_bool,
) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        if value.is_null() {
            set_last_error(ptr::null());
            return ptr::null_mut();
        }
        let bytes = std::slice::from_raw_parts(value as *const u8, buffer_length);
        let (parsed_bytes, bom_skip) = strip_utf8_bom(bytes);
        match parse(parsed_bytes) {
            Ok(v) => {
                set_last_error(ptr::null());
                if !return_parse_end.is_null() {
                    *return_parse_end = value.add(buffer_length);
                }
                value_to_cjson(&v)
            }
            Err(e) => {
                let err_ptr = value.add(bom_skip + error_byte_offset(&e, parsed_bytes.len()));
                set_last_error(err_ptr);
                if !return_parse_end.is_null() {
                    *return_parse_end = err_ptr;
                }
                ptr::null_mut()
            }
        }
    })
}

// =============================================================
// GetErrorPtr — returns a pointer into the caller's last-parsed buffer
// at the byte where the parser stopped, or NULL if the last parse
// succeeded or no parse has occurred on this thread.
// =============================================================

#[no_mangle]
pub unsafe extern "C" fn cJSON_GetErrorPtr() -> *const c_char {
    guard(ptr::null(), || LAST_ERROR_PTR.with(|c| c.get()))
}

// =============================================================
// InitHooks — install caller-supplied malloc/free
// =============================================================

#[repr(C)]
pub struct cJSON_Hooks {
    pub malloc_fn: Option<alloc_hooks::MallocFn>,
    pub free_fn: Option<alloc_hooks::FreeFn>,
}

/// Replace the default `libc::malloc` / `libc::free` with caller-supplied
/// functions. Passing a NULL `hooks` pointer reverts to defaults; passing
/// a `cJSON_Hooks` whose function fields are NULL reverts each slot
/// individually.
#[no_mangle]
pub unsafe extern "C" fn cJSON_InitHooks(hooks: *mut cJSON_Hooks) {
    guard((), || {
        if hooks.is_null() {
            alloc_hooks::set_hooks(None, None);
            return;
        }
        alloc_hooks::set_hooks((*hooks).malloc_fn, (*hooks).free_fn);
    })
}

// =============================================================
// Detach / Delete item
// =============================================================

#[no_mangle]
pub unsafe extern "C" fn cJSON_DetachItemViaPointer(
    parent: *mut cJSON,
    item: *mut cJSON,
) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        if parent.is_null() || item.is_null() {
            return ptr::null_mut();
        }
        detach(parent, item);
        item
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_DetachItemFromArray(
    array: *mut cJSON,
    which: c_int,
) -> *mut cJSON {
    guard(ptr::null_mut(), || {
        let item = child_at_index(array, which);
        if item.is_null() {
            return ptr::null_mut();
        }
        detach(array, item);
        item
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_DeleteItemFromArray(array: *mut cJSON, which: c_int) {
    guard((), || {
        let item = cJSON_DetachItemFromArray(array, which);
        if !item.is_null() {
            free_cjson(item);
        }
    })
}

unsafe fn detach_object_item(
    object: *mut cJSON,
    string: *const c_char,
    case_sensitive: bool,
) -> *mut cJSON {
    let item = get_object_item_impl(object, string, case_sensitive);
    if item.is_null() {
        return ptr::null_mut();
    }
    detach(object, item);
    item
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_DetachItemFromObject(
    object: *mut cJSON,
    string: *const c_char,
) -> *mut cJSON {
    guard(ptr::null_mut(), || detach_object_item(object, string, false))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_DetachItemFromObjectCaseSensitive(
    object: *mut cJSON,
    string: *const c_char,
) -> *mut cJSON {
    guard(ptr::null_mut(), || detach_object_item(object, string, true))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_DeleteItemFromObject(object: *mut cJSON, string: *const c_char) {
    guard((), || {
        let item = detach_object_item(object, string, false);
        if !item.is_null() {
            free_cjson(item);
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_DeleteItemFromObjectCaseSensitive(
    object: *mut cJSON,
    string: *const c_char,
) {
    guard((), || {
        let item = detach_object_item(object, string, true);
        if !item.is_null() {
            free_cjson(item);
        }
    })
}

// =============================================================
// Insert / Replace item
// =============================================================

#[no_mangle]
pub unsafe extern "C" fn cJSON_InsertItemInArray(
    array: *mut cJSON,
    which: c_int,
    newitem: *mut cJSON,
) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || {
        if array.is_null() || newitem.is_null() || which < 0 {
            return CJSON_BOOL_FALSE;
        }
        if ((*array).type_ & 0xFF) != CJSON_ARRAY {
            return CJSON_BOOL_FALSE;
        }
        let existing = child_at_index(array, which);
        if existing.is_null() {
            // Out-of-bounds: append (cJSON behaviour for negative is reject;
            // for past-end, we treat it as append).
            attach_child(array, newitem);
            return CJSON_BOOL_TRUE;
        }
        // Splice newitem before `existing`.
        (*newitem).prev = (*existing).prev;
        (*newitem).next = existing;
        if (*existing).prev.is_null() {
            (*array).child = newitem;
        } else {
            (*(*existing).prev).next = newitem;
        }
        (*existing).prev = newitem;
        CJSON_BOOL_TRUE
    })
}

unsafe fn replace_via_pointer(
    parent: *mut cJSON,
    item: *mut cJSON,
    replacement: *mut cJSON,
) -> cJSON_bool {
    if parent.is_null() || item.is_null() || replacement.is_null() {
        return CJSON_BOOL_FALSE;
    }
    // Splice replacement into item's place.
    (*replacement).prev = (*item).prev;
    (*replacement).next = (*item).next;
    if !(*item).prev.is_null() {
        (*(*item).prev).next = replacement;
    } else {
        (*parent).child = replacement;
    }
    if !(*item).next.is_null() {
        (*(*item).next).prev = replacement;
    }
    // Preserve the original key on the replacement (for object members).
    if !(*item).string.is_null() {
        if (*replacement).string.is_null() {
            (*replacement).string = (*item).string;
            (*item).string = ptr::null_mut();
        }
    }
    (*item).prev = ptr::null_mut();
    (*item).next = ptr::null_mut();
    free_cjson(item);
    CJSON_BOOL_TRUE
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_ReplaceItemViaPointer(
    parent: *mut cJSON,
    item: *mut cJSON,
    replacement: *mut cJSON,
) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || {
        replace_via_pointer(parent, item, replacement)
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_ReplaceItemInArray(
    array: *mut cJSON,
    which: c_int,
    newitem: *mut cJSON,
) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || {
        let item = child_at_index(array, which);
        if item.is_null() {
            return CJSON_BOOL_FALSE;
        }
        replace_via_pointer(array, item, newitem)
    })
}

unsafe fn replace_object_item(
    object: *mut cJSON,
    string: *const c_char,
    newitem: *mut cJSON,
    case_sensitive: bool,
) -> cJSON_bool {
    let item = get_object_item_impl(object, string, case_sensitive);
    if item.is_null() {
        return CJSON_BOOL_FALSE;
    }
    replace_via_pointer(object, item, newitem)
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_ReplaceItemInObject(
    object: *mut cJSON,
    string: *const c_char,
    newitem: *mut cJSON,
) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || {
        replace_object_item(object, string, newitem, false)
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_ReplaceItemInObjectCaseSensitive(
    object: *mut cJSON,
    string: *const c_char,
    newitem: *mut cJSON,
) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || {
        replace_object_item(object, string, newitem, true)
    })
}

// =============================================================
// Duplicate + Minify
// =============================================================

#[no_mangle]
pub unsafe extern "C" fn cJSON_Duplicate(
    item: *const cJSON,
    recurse: cJSON_bool,
) -> *mut cJSON {
    guard(ptr::null_mut(), || duplicate(item, recurse != 0))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_Minify(json: *mut c_char) {
    guard((), || minify_in_place(json))
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_PrintBuffered(
    item: *const cJSON,
    _prebuffer: c_int,
    fmt: cJSON_bool,
) -> *mut c_char {
    // Forward to Print / PrintUnformatted; the prebuffer hint is ignored
    // (Rust's String/Vec grows as needed).
    if fmt != 0 {
        cJSON_Print(item)
    } else {
        cJSON_PrintUnformatted(item)
    }
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_PrintPreallocated(
    _item: *mut cJSON,
    _buffer: *mut c_char,
    _length: c_int,
    _format: cJSON_bool,
) -> cJSON_bool {
    guard(CJSON_BOOL_FALSE, || CJSON_BOOL_FALSE)
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_SetNumberHelper(
    object: *mut cJSON,
    number: c_double,
) -> c_double {
    guard(number, || {
        if object.is_null() || ((*object).type_ & 0xFF) != CJSON_NUMBER {
            return number;
        }
        (*object).valuedouble = number;
        (*object).valueint = clamp_to_int(number);
        number
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_SetValuestring(
    object: *mut cJSON,
    valuestring: *const c_char,
) -> *mut c_char {
    guard(ptr::null_mut(), || {
        if object.is_null()
            || valuestring.is_null()
            || ((*object).type_ & 0xFF) != CJSON_STRING
            || ((*object).type_ & CJSON_IS_REFERENCE) != 0
        {
            return ptr::null_mut();
        }
        let s = match CStr::from_ptr(valuestring).to_str() {
            Ok(v) => v,
            Err(_) => return ptr::null_mut(),
        };
        let new_buf = alloc_cstr(s);
        if new_buf.is_null() {
            return ptr::null_mut();
        }
        if !(*object).valuestring.is_null() {
            alloc_hooks::hook_free((*object).valuestring as *mut libc::c_void);
        }
        (*object).valuestring = new_buf;
        new_buf
    })
}

#[no_mangle]
pub unsafe extern "C" fn cJSON_malloc(size: usize) -> *mut std::ffi::c_void {
    guard(ptr::null_mut(), || alloc_hooks::hook_malloc(size))
}
