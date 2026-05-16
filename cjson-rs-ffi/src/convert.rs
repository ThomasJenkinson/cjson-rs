//! Conversion between the safe Rust `Value` and the C-ABI `cJSON` tree.
//!
//! All nodes and strings are allocated via `libc::malloc` so that C
//! consumers can free them with stdlib `free()` (matching cJSON's
//! documented memory model). Strings are stored as NUL-terminated C
//! strings.

use crate::types::{
    cJSON, CJSON_ARRAY, CJSON_FALSE, CJSON_NULL, CJSON_NUMBER, CJSON_OBJECT, CJSON_STRING,
    CJSON_TRUE,
};
use cjson_rs::Value;
use std::ffi::{c_char, c_int};
use std::mem::size_of;
use std::ptr;

/// Allocate a zero-initialised cJSON node via `malloc`.
///
/// Returns a freshly-malloc'd, zeroed pointer. Caller owns it.
pub(crate) unsafe fn alloc_node() -> *mut cJSON {
    let ptr = crate::alloc_hooks::hook_malloc(size_of::<cJSON>()) as *mut cJSON;
    if ptr.is_null() {
        return ptr;
    }
    libc::memset(ptr as *mut libc::c_void, 0, size_of::<cJSON>());
    ptr
}

/// Copy a `&str` into a freshly-malloc'd, NUL-terminated C string.
///
/// Returns `null` on allocation failure. Caller owns the returned pointer.
pub(crate) unsafe fn alloc_cstr(s: &str) -> *mut c_char {
    let len = s.len();
    let buf = crate::alloc_hooks::hook_malloc(len + 1) as *mut c_char;
    if buf.is_null() {
        return ptr::null_mut();
    }
    ptr::copy_nonoverlapping(s.as_ptr() as *const c_char, buf, len);
    *buf.add(len) = 0;
    buf
}

/// Convert a `Value` into a freshly-allocated cJSON tree.
///
/// Returns `null` on allocation failure.
pub(crate) unsafe fn value_to_cjson(value: &Value) -> *mut cJSON {
    let node = alloc_node();
    if node.is_null() {
        return ptr::null_mut();
    }

    match value {
        Value::Null => {
            (*node).type_ = CJSON_NULL;
        }
        Value::Bool(true) => {
            (*node).type_ = CJSON_TRUE;
        }
        Value::Bool(false) => {
            (*node).type_ = CJSON_FALSE;
        }
        Value::Number(n) => {
            (*node).type_ = CJSON_NUMBER;
            (*node).valuedouble = *n;
            (*node).valueint = clamp_to_int(*n);
        }
        Value::String(s) => {
            (*node).type_ = CJSON_STRING;
            (*node).valuestring = alloc_cstr(s);
            if (*node).valuestring.is_null() {
                crate::alloc_hooks::hook_free(node as *mut libc::c_void);
                return ptr::null_mut();
            }
        }
        Value::Array(items) => {
            (*node).type_ = CJSON_ARRAY;
            if !append_children(node, items.iter().map(|v| (None, v))) {
                free_cjson(node);
                return ptr::null_mut();
            }
        }
        Value::Object(members) => {
            (*node).type_ = CJSON_OBJECT;
            if !append_children(node, members.iter().map(|(k, v)| (Some(k.as_str()), v))) {
                free_cjson(node);
                return ptr::null_mut();
            }
        }
    }

    node
}

/// Build the doubly-linked-list of children for an array or object node.
///
/// Each child is allocated via `value_to_cjson`. Object members carry
/// a key in `child->string`. Returns `false` on allocation failure
/// (caller is responsible for freeing the partial tree).
unsafe fn append_children<'a, I>(parent: *mut cJSON, items: I) -> bool
where
    I: Iterator<Item = (Option<&'a str>, &'a Value)>,
{
    let mut prev: *mut cJSON = ptr::null_mut();
    for (key, value) in items {
        let child = value_to_cjson(value);
        if child.is_null() {
            return false;
        }
        if let Some(k) = key {
            (*child).string = alloc_cstr(k);
            if (*child).string.is_null() {
                free_cjson(child);
                return false;
            }
        }
        (*child).prev = prev;
        if prev.is_null() {
            (*parent).child = child;
        } else {
            (*prev).next = child;
        }
        prev = child;
    }
    true
}

/// Recursively free a cJSON tree (matches `cJSON_Delete` behaviour).
///
/// Honours `CJSON_IS_REFERENCE`: a reference array/object owns neither
/// its children nor its `valuestring`; only the wrapper node itself is
/// freed. Honours `CJSON_STRING_IS_CONST`: the `string` (key) is not
/// freed when this flag is set.
pub(crate) unsafe fn free_cjson(node: *mut cJSON) {
    if node.is_null() {
        return;
    }
    let is_reference = ((*node).type_ & crate::types::CJSON_IS_REFERENCE) != 0;

    // Only walk into children if we own them.
    if !is_reference {
        let mut cur = (*node).child;
        while !cur.is_null() {
            let next = (*cur).next;
            free_cjson(cur);
            cur = next;
        }
        if !(*node).valuestring.is_null() {
            crate::alloc_hooks::hook_free((*node).valuestring as *mut libc::c_void);
        }
    }
    if !(*node).string.is_null() && ((*node).type_ & crate::types::CJSON_STRING_IS_CONST) == 0 {
        crate::alloc_hooks::hook_free((*node).string as *mut libc::c_void);
    }
    crate::alloc_hooks::hook_free(node as *mut libc::c_void);
}

/// Attach `child` to `array`'s linked-list of children (append at tail).
///
/// `array` must be a CJSON_ARRAY or CJSON_OBJECT node. `child` becomes
/// owned by `array` — caller must not free it independently.
pub(crate) unsafe fn attach_child(parent: *mut cJSON, child: *mut cJSON) {
    if (*parent).child.is_null() {
        (*parent).child = child;
        (*child).prev = ptr::null_mut();
        (*child).next = ptr::null_mut();
        return;
    }
    // Walk to the tail of the existing list.
    let mut tail = (*parent).child;
    while !(*tail).next.is_null() {
        tail = (*tail).next;
    }
    (*tail).next = child;
    (*child).prev = tail;
    (*child).next = ptr::null_mut();
}

/// Unlink `item` from its parent's doubly-linked-list of children.
/// Does NOT free the item; caller decides what to do with it.
pub(crate) unsafe fn detach(parent: *mut cJSON, item: *mut cJSON) {
    if item.is_null() {
        return;
    }
    let prev = (*item).prev;
    let next = (*item).next;
    if !prev.is_null() {
        (*prev).next = next;
    } else if !parent.is_null() {
        (*parent).child = next;
    }
    if !next.is_null() {
        (*next).prev = prev;
    }
    (*item).next = ptr::null_mut();
    (*item).prev = ptr::null_mut();
}

/// Find the n-th child of `parent` (0-indexed), or NULL.
pub(crate) unsafe fn child_at_index(parent: *const cJSON, which: c_int) -> *mut cJSON {
    if parent.is_null() || which < 0 {
        return ptr::null_mut();
    }
    let mut cur = (*parent).child;
    let mut i: c_int = 0;
    while !cur.is_null() {
        if i == which {
            return cur;
        }
        cur = (*cur).next;
        i += 1;
    }
    ptr::null_mut()
}

/// Recursively clone a cJSON tree. If `recurse` is false, only the root
/// node is cloned (its children pointer is left NULL).
pub(crate) unsafe fn duplicate(item: *const cJSON, recurse: bool) -> *mut cJSON {
    if item.is_null() {
        return ptr::null_mut();
    }
    let node = alloc_node();
    if node.is_null() {
        return ptr::null_mut();
    }
    let type_tag = (*item).type_ & !crate::types::CJSON_IS_REFERENCE
                                & !crate::types::CJSON_STRING_IS_CONST;
    (*node).type_ = type_tag;
    (*node).valuedouble = (*item).valuedouble;
    (*node).valueint = (*item).valueint;

    if !(*item).valuestring.is_null() {
        if let Ok(s) = std::ffi::CStr::from_ptr((*item).valuestring).to_str() {
            (*node).valuestring = alloc_cstr(s);
            if (*node).valuestring.is_null() {
                hook_free_node(node);
                return ptr::null_mut();
            }
        }
    }
    if !(*item).string.is_null() {
        if let Ok(s) = std::ffi::CStr::from_ptr((*item).string).to_str() {
            (*node).string = alloc_cstr(s);
            if (*node).string.is_null() {
                hook_free_node(node);
                return ptr::null_mut();
            }
        }
    }

    if recurse {
        let mut src_child = (*item).child;
        while !src_child.is_null() {
            let dst_child = duplicate(src_child, true);
            if dst_child.is_null() {
                hook_free_node(node);
                return ptr::null_mut();
            }
            attach_child(node, dst_child);
            src_child = (*src_child).next;
        }
    }
    node
}

unsafe fn hook_free_node(n: *mut cJSON) {
    free_cjson(n);
}

/// In-place minification: strip insignificant whitespace and comments
/// from a NUL-terminated JSON string. Mirrors cJSON_Minify's behaviour.
pub(crate) unsafe fn minify_in_place(json: *mut c_char) {
    if json.is_null() {
        return;
    }
    let mut read = json;
    let mut write = json;
    while *read != 0 {
        let b = *read as u8;
        match b {
            b' ' | b'\t' | b'\n' | b'\r' => {
                read = read.add(1);
            }
            b'"' => {
                // Copy the whole string literal verbatim (including escapes).
                *write = *read;
                write = write.add(1);
                read = read.add(1);
                while *read != 0 && *read as u8 != b'"' {
                    if *read as u8 == b'\\' && *read.add(1) != 0 {
                        *write = *read;
                        *write.add(1) = *read.add(1);
                        write = write.add(2);
                        read = read.add(2);
                    } else {
                        *write = *read;
                        write = write.add(1);
                        read = read.add(1);
                    }
                }
                if *read != 0 {
                    *write = *read;
                    write = write.add(1);
                    read = read.add(1);
                }
            }
            b'/' if *read.add(1) as u8 == b'/' => {
                // Line comment: skip to newline.
                while *read != 0 && *read as u8 != b'\n' {
                    read = read.add(1);
                }
            }
            b'/' if *read.add(1) as u8 == b'*' => {
                // Block comment: skip to */.
                read = read.add(2);
                while *read != 0
                    && !(*read as u8 == b'*' && *read.add(1) as u8 == b'/')
                {
                    read = read.add(1);
                }
                if *read != 0 {
                    read = read.add(2);
                }
            }
            _ => {
                *write = *read;
                write = write.add(1);
                read = read.add(1);
            }
        }
    }
    *write = 0;
}

/// Set `child.string` from `key` (malloc'd copy), then attach to `parent`.
///
/// Returns `false` on allocation failure (caller owns `child` and must
/// free it if this returns false).
pub(crate) unsafe fn attach_named_child(
    parent: *mut cJSON,
    key: *const c_char,
    child: *mut cJSON,
) -> bool {
    if parent.is_null() || key.is_null() || child.is_null() {
        return false;
    }
    // Copy the key string into a fresh malloc'd buffer.
    let key_str = match std::ffi::CStr::from_ptr(key).to_str() {
        Ok(s) => s,
        Err(_) => return false,
    };
    let key_buf = alloc_cstr(key_str);
    if key_buf.is_null() {
        return false;
    }
    // Free any pre-existing key.
    if !(*child).string.is_null()
        && ((*child).type_ & crate::types::CJSON_STRING_IS_CONST) == 0
    {
        crate::alloc_hooks::hook_free((*child).string as *mut libc::c_void);
    }
    (*child).type_ &= !crate::types::CJSON_STRING_IS_CONST;
    (*child).string = key_buf;
    attach_child(parent, child);
    true
}

/// Allocate an empty cJSON node of the given type.
pub(crate) unsafe fn new_node(t: c_int) -> *mut cJSON {
    let n = alloc_node();
    if n.is_null() {
        return n;
    }
    (*n).type_ = t;
    n
}


/// Saturating cast f64 → c_int, matching cJSON's `valueint` behaviour.
pub(crate) fn clamp_to_int(n: f64) -> c_int {
    if n.is_nan() {
        return 0;
    }
    if n >= c_int::MAX as f64 {
        return c_int::MAX;
    }
    if n <= c_int::MIN as f64 {
        return c_int::MIN;
    }
    n as c_int
}
