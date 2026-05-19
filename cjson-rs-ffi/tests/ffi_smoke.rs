//! Rust-side FFI smoke tests.
//!
//! These exercise the FFI exports through `unsafe` Rust calls — the same
//! code path a C caller would hit. They run with plain `cargo test` and
//! catch regressions before the C-side smoke test runs.

#![allow(non_snake_case)]

use cjson::types::{cJSON, CJSON_ARRAY, CJSON_NUMBER, CJSON_OBJECT, CJSON_STRING};
use cjson::*;
use std::ffi::{c_char, CStr, CString};

fn cstr(s: &str) -> CString {
    CString::new(s).unwrap()
}

#[test]
fn version_returns_static_string() {
    unsafe {
        let p = cJSON_Version();
        assert!(!p.is_null());
        let s = CStr::from_ptr(p).to_str().unwrap();
        assert!(s.starts_with("cjson-rs"));
    }
}

#[test]
fn parse_null_returns_null_pointer() {
    unsafe {
        assert!(cJSON_Parse(std::ptr::null()).is_null());
    }
}

#[test]
fn parse_invalid_json_returns_null() {
    unsafe {
        let bad = cstr("{not json");
        let p = cJSON_Parse(bad.as_ptr());
        assert!(p.is_null());
    }
}

#[test]
fn parse_simple_object_produces_correct_tree() {
    unsafe {
        let json = cstr(r#"{"a":1,"b":true}"#);
        let root = cJSON_Parse(json.as_ptr());
        assert!(!root.is_null());
        assert_eq!(cJSON_IsObject(root), 1);

        let a_key = cstr("a");
        let a = cJSON_GetObjectItemCaseSensitive(root, a_key.as_ptr());
        assert!(!a.is_null());
        assert_eq!(cJSON_IsNumber(a), 1);
        assert_eq!(cJSON_GetNumberValue(a), 1.0);

        let b_key = cstr("b");
        let b = cJSON_GetObjectItemCaseSensitive(root, b_key.as_ptr());
        assert!(!b.is_null());
        assert_eq!(cJSON_IsBool(b), 1);
        assert_eq!(cJSON_IsTrue(b), 1);

        cJSON_Delete(root);
    }
}

#[test]
fn parse_array_navigation_works() {
    unsafe {
        let json = cstr(r#"[10,20,30,40]"#);
        let arr = cJSON_Parse(json.as_ptr());
        assert!(!arr.is_null());
        assert_eq!(cJSON_IsArray(arr), 1);
        assert_eq!(cJSON_GetArraySize(arr), 4);

        for (i, expected) in [10.0, 20.0, 30.0, 40.0].iter().enumerate() {
            let item = cJSON_GetArrayItem(arr, i as i32);
            assert!(!item.is_null());
            assert_eq!(cJSON_GetNumberValue(item), *expected);
        }

        // Out-of-bounds index returns NULL.
        assert!(cJSON_GetArrayItem(arr, 4).is_null());
        // Negative index returns NULL.
        assert!(cJSON_GetArrayItem(arr, -1).is_null());

        cJSON_Delete(arr);
    }
}

#[test]
fn get_object_item_case_insensitive_vs_case_sensitive() {
    unsafe {
        let json = cstr(r#"{"Name":"alice"}"#);
        let root = cJSON_Parse(json.as_ptr());
        assert!(!root.is_null());

        let lower = cstr("name");
        // Case-insensitive: finds it.
        assert!(!cJSON_GetObjectItem(root, lower.as_ptr()).is_null());
        // Case-sensitive: misses.
        assert!(cJSON_GetObjectItemCaseSensitive(root, lower.as_ptr()).is_null());
        // Exact case: hits both.
        let exact = cstr("Name");
        assert!(!cJSON_GetObjectItem(root, exact.as_ptr()).is_null());
        assert!(!cJSON_GetObjectItemCaseSensitive(root, exact.as_ptr()).is_null());

        cJSON_Delete(root);
    }
}

#[test]
fn get_string_value_returns_inner_pointer() {
    unsafe {
        let json = cstr(r#""hello world""#);
        let root = cJSON_Parse(json.as_ptr());
        assert!(!root.is_null());
        assert_eq!(cJSON_IsString(root), 1);

        let s_ptr = cJSON_GetStringValue(root);
        assert!(!s_ptr.is_null());
        let s = CStr::from_ptr(s_ptr).to_str().unwrap();
        assert_eq!(s, "hello world");

        cJSON_Delete(root);
    }
}

#[test]
fn type_predicate_on_null_pointer_returns_false() {
    unsafe {
        // Every predicate must be NULL-safe.
        assert_eq!(cJSON_IsNull(std::ptr::null()), 0);
        assert_eq!(cJSON_IsTrue(std::ptr::null()), 0);
        assert_eq!(cJSON_IsFalse(std::ptr::null()), 0);
        assert_eq!(cJSON_IsBool(std::ptr::null()), 0);
        assert_eq!(cJSON_IsNumber(std::ptr::null()), 0);
        assert_eq!(cJSON_IsString(std::ptr::null()), 0);
        assert_eq!(cJSON_IsArray(std::ptr::null()), 0);
        assert_eq!(cJSON_IsObject(std::ptr::null()), 0);
        assert_eq!(cJSON_IsRaw(std::ptr::null()), 0);
        assert_eq!(cJSON_IsInvalid(std::ptr::null()), 0);
    }
}

#[test]
fn get_number_value_on_wrong_type_returns_nan() {
    unsafe {
        let json = cstr(r#""not a number""#);
        let root = cJSON_Parse(json.as_ptr());
        assert!(!root.is_null());
        // Recent cJSON CVE: type confusion. Our impl must NOT just read
        // valuedouble blindly — must verify the type tag first.
        assert!(cJSON_GetNumberValue(root).is_nan());
        cJSON_Delete(root);
    }
}

#[test]
fn print_round_trips_through_parse() {
    unsafe {
        let original = cstr(r#"{"x":[1,2,{"y":true}]}"#);
        let tree1 = cJSON_Parse(original.as_ptr());
        assert!(!tree1.is_null());

        let printed = cJSON_PrintUnformatted(tree1);
        assert!(!printed.is_null());
        let printed_str = CStr::from_ptr(printed).to_str().unwrap().to_string();

        // Re-parse the printed output.
        let tree2 = cJSON_Parse(printed as *const c_char);
        assert!(!tree2.is_null());

        // Print tree2 too; the outputs should match.
        let printed2 = cJSON_PrintUnformatted(tree2);
        let printed2_str = CStr::from_ptr(printed2).to_str().unwrap();
        assert_eq!(printed_str, printed2_str);

        cJSON_free(printed as *mut std::ffi::c_void);
        cJSON_free(printed2 as *mut std::ffi::c_void);
        cJSON_Delete(tree1);
        cJSON_Delete(tree2);
    }
}

#[test]
fn print_pretty_produces_indented_output() {
    unsafe {
        let json = cstr(r#"{"a":1}"#);
        let root = cJSON_Parse(json.as_ptr());
        let printed = cJSON_Print(root);
        let s = CStr::from_ptr(printed).to_str().unwrap();
        // cJSON_Print matches upstream's byte-exact format: tab indent,
        // ":\t" after keys, newlines around object braces.
        assert_eq!(s, "{\n\t\"a\":\t1\n}");
        cJSON_free(printed as *mut std::ffi::c_void);
        cJSON_Delete(root);
    }
}

#[test]
fn delete_null_is_safe() {
    unsafe {
        cJSON_Delete(std::ptr::null_mut());
        cJSON_free(std::ptr::null_mut());
    }
}

#[test]
fn parse_with_length_does_not_require_nul() {
    unsafe {
        let buf: &[u8] = b"[1,2,3]extra-garbage-after-len";
        // Pass only the first 7 bytes ("[1,2,3]") — the trailing garbage
        // is past the length and must not be read.
        let root = cJSON_ParseWithLength(buf.as_ptr() as *const c_char, 7);
        assert!(!root.is_null());
        assert_eq!(cJSON_GetArraySize(root), 3);
        cJSON_Delete(root);
    }
}

#[test]
fn struct_field_layout_matches_cjson_h() {
    // Spot-check: walking children via the public struct fields works.
    // This is the access pattern many C consumers use directly.
    unsafe {
        let json = cstr(r#"{"a":1,"b":2,"c":3}"#);
        let root = cJSON_Parse(json.as_ptr());

        let mut cur = (*root).child;
        let mut keys = Vec::new();
        while !cur.is_null() {
            let k = CStr::from_ptr((*cur).string).to_str().unwrap().to_string();
            keys.push(k);
            cur = (*cur).next;
        }
        assert_eq!(keys, vec!["a", "b", "c"]);

        cJSON_Delete(root);
    }
}

#[test]
fn deeply_nested_input_is_rejected_safely() {
    unsafe {
        // 2000 levels — exceeds the default 1000 nesting limit.
        let mut s = String::new();
        for _ in 0..2000 {
            s.push('[');
        }
        s.push('1');
        for _ in 0..2000 {
            s.push(']');
        }
        let c = CString::new(s).unwrap();
        // Must return NULL (not panic, not stack-overflow).
        assert!(cJSON_Parse(c.as_ptr()).is_null());
    }
}

#[test]
fn struct_size_is_stable() {
    // Sanity: cJSON has 8 fields on LP64 → 56 or 64 bytes (depending on
    // C int width / padding). If a Rust compiler change ever altered
    // the layout, this would catch it.
    let sz = std::mem::size_of::<cJSON>();
    assert!(sz == 56 || sz == 64, "unexpected cJSON size: {sz}");
}

// =============================================================
// Constructors + convenience helpers
// =============================================================

#[test]
fn build_object_from_scratch_with_convenience_helpers() {
    unsafe {
        // Build {"name":"alice","age":30,"verified":true,"meta":null}
        let root = cJSON_CreateObject();
        assert!(!root.is_null());

        let name_key = cstr("name");
        let name_val = cstr("alice");
        assert!(!cJSON_AddStringToObject(root, name_key.as_ptr(), name_val.as_ptr()).is_null());

        let age_key = cstr("age");
        assert!(!cJSON_AddNumberToObject(root, age_key.as_ptr(), 30.0).is_null());

        let verified_key = cstr("verified");
        assert!(!cJSON_AddTrueToObject(root, verified_key.as_ptr()).is_null());

        let meta_key = cstr("meta");
        assert!(!cJSON_AddNullToObject(root, meta_key.as_ptr()).is_null());

        // Round-trip via print → parse → compare.
        let printed = cJSON_PrintUnformatted(root);
        let s = CStr::from_ptr(printed).to_str().unwrap();
        assert_eq!(s, r#"{"name":"alice","age":30,"verified":true,"meta":null}"#);

        cJSON_free(printed as *mut std::ffi::c_void);
        cJSON_Delete(root);
    }
}

#[test]
fn typed_array_constructors() {
    unsafe {
        let ints: [i32; 4] = [10, 20, 30, 40];
        let arr = cJSON_CreateIntArray(ints.as_ptr(), 4);
        assert!(!arr.is_null());
        assert_eq!(cJSON_GetArraySize(arr), 4);
        let printed = cJSON_PrintUnformatted(arr);
        let s = CStr::from_ptr(printed).to_str().unwrap();
        assert_eq!(s, "[10,20,30,40]");
        cJSON_free(printed as *mut std::ffi::c_void);
        cJSON_Delete(arr);

        let doubles: [f64; 3] = [1.5, 2.5, 3.5];
        let darr = cJSON_CreateDoubleArray(doubles.as_ptr(), 3);
        let printed = cJSON_PrintUnformatted(darr);
        let s = CStr::from_ptr(printed).to_str().unwrap();
        assert_eq!(s, "[1.5,2.5,3.5]");
        cJSON_free(printed as *mut std::ffi::c_void);
        cJSON_Delete(darr);

        let alice = cstr("alice");
        let bob = cstr("bob");
        let strs: [*const c_char; 2] = [alice.as_ptr(), bob.as_ptr()];
        let sarr = cJSON_CreateStringArray(strs.as_ptr(), 2);
        assert_eq!(cJSON_GetArraySize(sarr), 2);
        let printed = cJSON_PrintUnformatted(sarr);
        let s = CStr::from_ptr(printed).to_str().unwrap();
        assert_eq!(s, r#"["alice","bob"]"#);
        cJSON_free(printed as *mut std::ffi::c_void);
        cJSON_Delete(sarr);
    }
}

#[test]
fn add_item_to_array() {
    unsafe {
        let arr = cJSON_CreateArray();
        for v in [1.0, 2.0, 3.0] {
            assert_eq!(cJSON_AddItemToArray(arr, cJSON_CreateNumber(v)), 1);
        }
        assert_eq!(cJSON_GetArraySize(arr), 3);
        let printed = cJSON_PrintUnformatted(arr);
        let s = CStr::from_ptr(printed).to_str().unwrap();
        assert_eq!(s, "[1,2,3]");
        cJSON_free(printed as *mut std::ffi::c_void);
        cJSON_Delete(arr);
    }
}

#[test]
fn add_item_to_array_rejects_non_array_parent() {
    unsafe {
        let obj = cJSON_CreateObject();
        let n = cJSON_CreateNumber(42.0);
        assert_eq!(cJSON_AddItemToArray(obj, n), 0);
        // The orphan item is the caller's responsibility on rejection.
        cJSON_Delete(n);
        cJSON_Delete(obj);
    }
}

#[test]
fn string_reference_does_not_double_free() {
    unsafe {
        // The C caller owns this string buffer; the reference node MUST NOT free it.
        let buf = cstr("borrowed");
        let n = cJSON_CreateStringReference(buf.as_ptr());
        assert!(!n.is_null());
        assert_eq!(cJSON_IsString(n), 1);
        let s_ptr = cJSON_GetStringValue(n);
        assert_eq!(CStr::from_ptr(s_ptr).to_str().unwrap(), "borrowed");
        // Delete the reference wrapper; `buf` must still be valid.
        cJSON_Delete(n);
        // If the reference node had freed it, CStr::from_ptr below would
        // touch released memory. Use-after-free.
        assert_eq!(CStr::from_ptr(buf.as_ptr()).to_str().unwrap(), "borrowed");
    }
}

#[test]
fn array_reference_does_not_free_children() {
    unsafe {
        let owned = cJSON_CreateArray();
        cJSON_AddItemToArray(owned, cJSON_CreateNumber(1.0));
        cJSON_AddItemToArray(owned, cJSON_CreateNumber(2.0));

        // Build a reference to `owned`'s first child.
        let first_child = (*owned).child;
        let ref_arr = cJSON_CreateArrayReference(first_child);
        assert!(!ref_arr.is_null());
        // Deleting the reference must not touch `owned` or its children.
        cJSON_Delete(ref_arr);

        // The original is still intact.
        assert_eq!(cJSON_GetArraySize(owned), 2);
        assert_eq!(cJSON_GetNumberValue(cJSON_GetArrayItem(owned, 0)), 1.0);
        cJSON_Delete(owned);
    }
}

#[test]
fn create_scalar_constructors() {
    unsafe {
        let n = cJSON_CreateNull();
        assert_eq!(cJSON_IsNull(n), 1);
        cJSON_Delete(n);

        let t = cJSON_CreateTrue();
        assert_eq!(cJSON_IsBool(t), 1);
        assert_eq!(cJSON_IsTrue(t), 1);
        cJSON_Delete(t);

        let f = cJSON_CreateFalse();
        assert_eq!(cJSON_IsFalse(f), 1);
        cJSON_Delete(f);

        let b0 = cJSON_CreateBool(0);
        assert_eq!(cJSON_IsFalse(b0), 1);
        cJSON_Delete(b0);

        let b1 = cJSON_CreateBool(1);
        assert_eq!(cJSON_IsTrue(b1), 1);
        cJSON_Delete(b1);

        let num = cJSON_CreateNumber(3.14);
        assert_eq!(cJSON_GetNumberValue(num), 3.14);
        cJSON_Delete(num);

        let s = cstr("hello");
        let str_node = cJSON_CreateString(s.as_ptr());
        let got = cJSON_GetStringValue(str_node);
        assert_eq!(CStr::from_ptr(got).to_str().unwrap(), "hello");
        cJSON_Delete(str_node);
    }
}

#[test]
fn create_string_with_null_returns_null() {
    unsafe {
        assert!(cJSON_CreateString(std::ptr::null()).is_null());
        assert!(cJSON_CreateRaw(std::ptr::null()).is_null());
    }
}

#[test]
fn add_to_object_with_null_args_is_safe() {
    unsafe {
        let obj = cJSON_CreateObject();
        let name = cstr("k");
        // null item:
        assert_eq!(cJSON_AddItemToObject(obj, name.as_ptr(), std::ptr::null_mut()), 0);
        // null name:
        let n = cJSON_CreateNumber(1.0);
        assert_eq!(cJSON_AddItemToObject(obj, std::ptr::null(), n), 0);
        cJSON_Delete(n);
        // null object:
        let n2 = cJSON_CreateNumber(2.0);
        assert_eq!(cJSON_AddItemToObject(std::ptr::null_mut(), name.as_ptr(), n2), 0);
        cJSON_Delete(n2);
        cJSON_Delete(obj);
    }
}

// =============================================================
// Mutators: Detach, Delete, Insert, Replace, Duplicate, Minify
// =============================================================

#[test]
fn detach_item_from_array_removes_and_returns() {
    unsafe {
        let arr = cJSON_Parse(cstr("[10,20,30]").as_ptr());
        let item = cJSON_DetachItemFromArray(arr, 1);
        assert!(!item.is_null());
        assert_eq!(cJSON_GetNumberValue(item), 20.0);
        // Caller owns the detached item; must free it explicitly.
        cJSON_Delete(item);
        // Array now has 2 items: [10, 30]
        assert_eq!(cJSON_GetArraySize(arr), 2);
        assert_eq!(cJSON_GetNumberValue(cJSON_GetArrayItem(arr, 0)), 10.0);
        assert_eq!(cJSON_GetNumberValue(cJSON_GetArrayItem(arr, 1)), 30.0);
        cJSON_Delete(arr);
    }
}

#[test]
fn delete_item_from_array_frees_it() {
    unsafe {
        let arr = cJSON_Parse(cstr("[10,20,30]").as_ptr());
        cJSON_DeleteItemFromArray(arr, 0);
        assert_eq!(cJSON_GetArraySize(arr), 2);
        assert_eq!(cJSON_GetNumberValue(cJSON_GetArrayItem(arr, 0)), 20.0);
        cJSON_Delete(arr);
    }
}

#[test]
fn detach_item_from_object_case_insensitive_vs_sensitive() {
    unsafe {
        let obj = cJSON_Parse(cstr(r#"{"Name":"alice"}"#).as_ptr());
        let lower = cstr("name");
        // Case-sensitive: misses.
        assert!(cJSON_DetachItemFromObjectCaseSensitive(obj, lower.as_ptr()).is_null());
        // Case-insensitive: hits, returns item.
        let item = cJSON_DetachItemFromObject(obj, lower.as_ptr());
        assert!(!item.is_null());
        cJSON_Delete(item);
        // Object should now be empty.
        let printed = cJSON_PrintUnformatted(obj);
        assert_eq!(CStr::from_ptr(printed).to_str().unwrap(), "{}");
        cJSON_free(printed as *mut std::ffi::c_void);
        cJSON_Delete(obj);
    }
}

#[test]
fn insert_item_in_array_shifts_following() {
    unsafe {
        let arr = cJSON_Parse(cstr("[10,30]").as_ptr());
        let new_item = cJSON_CreateNumber(20.0);
        assert_eq!(cJSON_InsertItemInArray(arr, 1, new_item), 1);
        assert_eq!(cJSON_GetArraySize(arr), 3);
        assert_eq!(cJSON_GetNumberValue(cJSON_GetArrayItem(arr, 0)), 10.0);
        assert_eq!(cJSON_GetNumberValue(cJSON_GetArrayItem(arr, 1)), 20.0);
        assert_eq!(cJSON_GetNumberValue(cJSON_GetArrayItem(arr, 2)), 30.0);
        cJSON_Delete(arr);
    }
}

#[test]
fn replace_item_in_array_swaps_and_frees_old() {
    unsafe {
        let arr = cJSON_Parse(cstr("[10,20,30]").as_ptr());
        let new_item = cJSON_CreateString(cstr("twenty").as_ptr());
        assert_eq!(cJSON_ReplaceItemInArray(arr, 1, new_item), 1);
        assert_eq!(cJSON_GetArraySize(arr), 3);
        let mid = cJSON_GetArrayItem(arr, 1);
        assert_eq!(cJSON_IsString(mid), 1);
        assert_eq!(
            CStr::from_ptr(cJSON_GetStringValue(mid)).to_str().unwrap(),
            "twenty"
        );
        cJSON_Delete(arr);
    }
}

#[test]
fn replace_item_in_object_preserves_key() {
    unsafe {
        let obj = cJSON_Parse(cstr(r#"{"k":1}"#).as_ptr());
        let new_v = cJSON_CreateNumber(99.0);
        let key = cstr("k");
        assert_eq!(cJSON_ReplaceItemInObjectCaseSensitive(obj, key.as_ptr(), new_v), 1);
        let printed = cJSON_PrintUnformatted(obj);
        assert_eq!(CStr::from_ptr(printed).to_str().unwrap(), r#"{"k":99}"#);
        cJSON_free(printed as *mut std::ffi::c_void);
        cJSON_Delete(obj);
    }
}

#[test]
fn duplicate_clones_tree() {
    unsafe {
        let src = cJSON_Parse(cstr(r#"{"a":[1,2,{"b":true}]}"#).as_ptr());
        let copy = cJSON_Duplicate(src, 1);
        assert!(!copy.is_null());
        // Modify copy; src must be unchanged.
        cJSON_DeleteItemFromObjectCaseSensitive(copy, cstr("a").as_ptr());
        let src_printed = cJSON_PrintUnformatted(src);
        assert_eq!(
            CStr::from_ptr(src_printed).to_str().unwrap(),
            r#"{"a":[1,2,{"b":true}]}"#
        );
        cJSON_free(src_printed as *mut std::ffi::c_void);
        cJSON_Delete(src);
        cJSON_Delete(copy);
    }
}

#[test]
fn duplicate_shallow_skips_children() {
    unsafe {
        let src = cJSON_Parse(cstr("[1,2,3]").as_ptr());
        let copy = cJSON_Duplicate(src, 0);
        assert_eq!(cJSON_IsArray(copy), 1);
        // Shallow copy: no children.
        assert_eq!(cJSON_GetArraySize(copy), 0);
        cJSON_Delete(src);
        cJSON_Delete(copy);
    }
}

#[test]
fn minify_strips_whitespace_in_place() {
    unsafe {
        // Mutable buffer (Minify operates in-place).
        let mut buf: Vec<u8> = b"  {\n  \"a\" : 1 ,\n  \"b\" : 2\n}  \0".to_vec();
        cJSON_Minify(buf.as_mut_ptr() as *mut c_char);
        let s = CStr::from_ptr(buf.as_ptr() as *const c_char).to_str().unwrap();
        assert_eq!(s, r#"{"a":1,"b":2}"#);
    }
}

// -- cJSON_PrintPreallocated ------------------------------------------------
//
// Previously a stub returning false unconditionally. These tests pin down
// the real semantics: NULL inputs and non-positive lengths return false;
// undersized buffers return false without touching the buffer; successful
// writes are NUL-terminated and byte-identical to cJSON_PrintUnformatted /
// cJSON_Print output.

fn parse_simple_obj() -> *mut cJSON {
    unsafe {
        let json = cstr(r#"{"a":1,"b":true}"#);
        cJSON_Parse(json.as_ptr())
    }
}

#[test]
fn print_preallocated_compact_fills_caller_buffer() {
    unsafe {
        let root = parse_simple_obj();
        assert!(!root.is_null());
        let mut buf = [0u8; 64];
        let ok = cJSON_PrintPreallocated(root, buf.as_mut_ptr() as *mut c_char, buf.len() as i32, 0);
        assert_eq!(ok, 1);
        let s = CStr::from_ptr(buf.as_ptr() as *const c_char).to_str().unwrap();
        // Same compact serialisation cJSON_PrintUnformatted would produce.
        assert_eq!(s, r#"{"a":1,"b":true}"#);
        cJSON_Delete(root);
    }
}

#[test]
fn print_preallocated_pretty_matches_print() {
    unsafe {
        let root = parse_simple_obj();
        assert!(!root.is_null());
        let mut buf = [0u8; 256];
        let ok = cJSON_PrintPreallocated(root, buf.as_mut_ptr() as *mut c_char, buf.len() as i32, 1);
        assert_eq!(ok, 1);
        let from_pre = CStr::from_ptr(buf.as_ptr() as *const c_char).to_str().unwrap().to_string();

        // Compare to what cJSON_Print would have produced for the same tree.
        let alloced = cJSON_Print(root);
        assert!(!alloced.is_null());
        let from_print = CStr::from_ptr(alloced).to_str().unwrap().to_string();
        cJSON_free(alloced as *mut std::ffi::c_void);

        assert_eq!(from_pre, from_print);
        cJSON_Delete(root);
    }
}

#[test]
fn print_preallocated_null_item_returns_false() {
    unsafe {
        let mut buf = [0u8; 16];
        let r = cJSON_PrintPreallocated(std::ptr::null_mut(), buf.as_mut_ptr() as *mut c_char, buf.len() as i32, 1);
        assert_eq!(r, 0);
    }
}

#[test]
fn print_preallocated_null_buffer_returns_false() {
    unsafe {
        let root = parse_simple_obj();
        assert!(!root.is_null());
        let r = cJSON_PrintPreallocated(root, std::ptr::null_mut(), 16, 1);
        assert_eq!(r, 0);
        cJSON_Delete(root);
    }
}

#[test]
fn print_preallocated_zero_length_returns_false() {
    unsafe {
        let root = parse_simple_obj();
        assert!(!root.is_null());
        let mut buf = [0u8; 16];
        let r = cJSON_PrintPreallocated(root, buf.as_mut_ptr() as *mut c_char, 0, 0);
        assert_eq!(r, 0);
        cJSON_Delete(root);
    }
}

#[test]
fn print_preallocated_negative_length_returns_false() {
    unsafe {
        let root = parse_simple_obj();
        assert!(!root.is_null());
        let mut buf = [0u8; 16];
        let r = cJSON_PrintPreallocated(root, buf.as_mut_ptr() as *mut c_char, -1, 0);
        assert_eq!(r, 0);
        cJSON_Delete(root);
    }
}

#[test]
fn print_preallocated_buffer_too_small_returns_false_and_leaves_buffer_untouched() {
    unsafe {
        let root = parse_simple_obj();
        assert!(!root.is_null());
        // Pre-fill with a sentinel; compact output is 16 bytes + NUL = 17.
        let mut buf = [0xAAu8; 8];
        let r = cJSON_PrintPreallocated(root, buf.as_mut_ptr() as *mut c_char, buf.len() as i32, 0);
        assert_eq!(r, 0);
        // Buffer must not have been written to.
        assert!(buf.iter().all(|&b| b == 0xAA), "buffer was mutated on failure");
        cJSON_Delete(root);
    }
}

#[test]
fn print_preallocated_exact_size_buffer_succeeds() {
    unsafe {
        let root = parse_simple_obj();
        assert!(!root.is_null());
        // Compact form is exactly 16 bytes — buffer of 17 is the minimum.
        let mut buf = [0u8; 17];
        let r = cJSON_PrintPreallocated(root, buf.as_mut_ptr() as *mut c_char, buf.len() as i32, 0);
        assert_eq!(r, 1);
        assert_eq!(buf[16], 0, "trailing NUL not written");
        let s = CStr::from_ptr(buf.as_ptr() as *const c_char).to_str().unwrap();
        assert_eq!(s, r#"{"a":1,"b":true}"#);
        cJSON_Delete(root);
    }
}

// -- cJSON_ParseWithOpts: require_null_terminated = false ------------------
//
// Mirrors the upstream test parse_with_opts_should_return_parse_end at
// conformance/upstream-tests/tests/parse_with_opts.c:73. Previously we
// rejected valid-prefix-then-trailing-bytes as a parse error; now we accept
// the prefix and report parse_end pointing right after its last byte.

#[test]
fn parse_with_opts_lax_accepts_prefix_with_trailing_garbage() {
    unsafe {
        let s = cstr("[] empty array XD");
        let mut parse_end: *const c_char = std::ptr::null();
        let item = cJSON_ParseWithOpts(s.as_ptr(), &mut parse_end, 0);
        assert!(!item.is_null(), "valid prefix [] must parse in lax mode");
        assert_eq!(cJSON_IsArray(item), 1);
        // parse_end must point to byte 2 — right after `]`, at the space.
        let offset = parse_end as usize - s.as_ptr() as usize;
        assert_eq!(offset, 2, "parse_end should point right after `]`");
        cJSON_Delete(item);
    }
}

#[test]
fn parse_with_opts_lax_consumes_whole_input_when_clean() {
    unsafe {
        let s = cstr(r#"{"a":1}"#);
        let mut parse_end: *const c_char = std::ptr::null();
        let item = cJSON_ParseWithOpts(s.as_ptr(), &mut parse_end, 0);
        assert!(!item.is_null());
        let offset = parse_end as usize - s.as_ptr() as usize;
        assert_eq!(offset, 7);
        cJSON_Delete(item);
    }
}

#[test]
fn parse_with_opts_strict_rejects_trailing_garbage() {
    unsafe {
        let s = cstr("{}x");
        let item = cJSON_ParseWithOpts(s.as_ptr(), std::ptr::null_mut(), 1);
        assert!(item.is_null(), "strict mode must reject `{{}}x`");
    }
}

#[test]
fn parse_with_opts_strict_accepts_trailing_whitespace() {
    unsafe {
        let s = cstr("{} \t\r\n");
        let item = cJSON_ParseWithOpts(s.as_ptr(), std::ptr::null_mut(), 1);
        assert!(!item.is_null(), "strict mode must allow trailing whitespace");
        cJSON_Delete(item);
    }
}

#[test]
fn parse_with_opts_lax_incomplete_value_still_errors() {
    unsafe {
        // No closing bracket — even in lax mode this can't be a valid prefix.
        let s = cstr("[1, 2");
        let mut parse_end: *const c_char = std::ptr::null();
        let item = cJSON_ParseWithOpts(s.as_ptr(), &mut parse_end, 0);
        assert!(item.is_null());
    }
}

// Silence unused-import warnings — these constants are referenced inside
// `unsafe` blocks but Rust's dead-code analysis doesn't see them.
#[allow(dead_code)]
fn _silence_unused() {
    let _ = (CJSON_ARRAY, CJSON_NUMBER, CJSON_OBJECT, CJSON_STRING);
}
