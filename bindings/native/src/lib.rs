//! Optional C ABI for hosts that cannot call Rust directly. Only the local gamectx crate
//! is used. All pointer lifetimes and buffer validity remain the C caller's responsibility.
#![deny(unsafe_op_in_unsafe_fn)]
use gamectx::kit::{Project, MAX_PROJECT_BYTES, MAX_REQUEST_BYTES};
use std::cell::RefCell;
use std::ffi::{c_char, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

thread_local! { static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) }; }
fn error(message: &str) {
    LAST_ERROR.with(|e| *e.borrow_mut() = message.into());
}
fn guard<T>(f: impl FnOnce() -> T, fallback: T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| {
        error("native operation panicked");
        fallback
    })
}
unsafe fn input<'a>(bytes: *const u8, len: usize, cap: usize) -> Option<&'a str> {
    if bytes.is_null() || len > cap {
        error("invalid input pointer or length");
        return None;
    }
    // SAFETY: the exported API contract requires a readable buffer of len bytes.
    match std::str::from_utf8(unsafe { std::slice::from_raw_parts(bytes, len) }) {
        Ok(s) => Some(s),
        Err(_) => {
            error("input must be UTF-8");
            None
        }
    }
}
/// Creates an immutable project handle; null indicates failure, explained by last_error.
/// # Safety
/// `bytes` must be non-null and readable for `len` bytes for the duration of this call.
#[no_mangle]
pub unsafe extern "C" fn gamectx_project_new(bytes: *const u8, len: usize) -> *mut Project {
    guard(
        || {
            let Some(text) = (unsafe { input(bytes, len, MAX_PROJECT_BYTES) }) else { return ptr::null_mut() };
            match Project::parse(text) {
                Ok(p) => Box::into_raw(Box::new(p)),
                Err(e) => {
                    error(&e.to_string());
                    ptr::null_mut()
                }
            }
        },
        ptr::null_mut(),
    )
}
/// Returns an owned NUL-terminated UTF-8 JSON reply, or null for an ABI-level failure.
/// Invalid JSON/protocol requests return a regular versioned error reply.
/// # Safety
/// `project` must be a live handle from project_new and remain alive for this call;
/// `bytes` must be readable for `len` bytes. Concurrent immutable requests are allowed.
#[no_mangle]
pub unsafe extern "C" fn gamectx_project_request(project: *const Project, bytes: *const u8, len: usize) -> *mut c_char {
    guard(
        || {
            if project.is_null() {
                error("null project handle");
                return ptr::null_mut();
            }
            let Some(text) = (unsafe { input(bytes, len, MAX_REQUEST_BYTES) }) else { return ptr::null_mut() };
            let response = unsafe { &*project }.request(text).to_json();
            CString::new(response).expect("JSON escapes NUL").into_raw()
        },
        ptr::null_mut(),
    )
}
/// Copies the last ABI-level error on the current thread into an owned UTF-8 string.
/// Free with gamectx_string_free. Errors are informational, not a persistent status flag.
#[no_mangle]
pub extern "C" fn gamectx_last_error() -> *mut c_char {
    guard(
        || LAST_ERROR.with(|e| CString::new(e.borrow().replace('\0', " ")).expect("NUL removed").into_raw()),
        ptr::null_mut(),
    )
}
/// Releases a response or last_error string. Null is accepted.
/// # Safety
/// A non-null pointer must be returned by this library and not previously freed.
#[no_mangle]
pub unsafe extern "C" fn gamectx_string_free(value: *mut c_char) {
    if !value.is_null() {
        drop(unsafe { CString::from_raw(value) });
    }
}
/// Releases a project. Null is accepted.
/// # Safety
/// A non-null handle must originate here, be freed once, and have no requests in flight.
#[no_mangle]
pub unsafe extern "C" fn gamectx_project_free(project: *mut Project) {
    if !project.is_null() {
        drop(unsafe { Box::from_raw(project) });
    }
}
/// Creates a persistent session without changing the immutable project ABI.
/// # Safety
/// `bytes` must be non-null and readable for `len` bytes for this call.
#[no_mangle]
pub unsafe extern "C" fn gamectx_session_new(bytes: *const u8, len: usize) -> *mut gamectx::kit::Session {
    guard(
        || {
            let Some(text) = (unsafe { input(bytes, len, MAX_PROJECT_BYTES) }) else { return ptr::null_mut() };
            match gamectx::kit::Session::parse(text) {
                Ok(s) => Box::into_raw(Box::new(s)),
                Err(e) => {
                    error(&e.to_string());
                    ptr::null_mut()
                }
            }
        },
        ptr::null_mut(),
    )
}
/// Runs a request against a session. Character changes are serialized internally.
/// # Safety
/// The session must stay live throughout the call and the buffer must be readable.
#[no_mangle]
pub unsafe extern "C" fn gamectx_session_request(
    session: *const gamectx::kit::Session,
    bytes: *const u8,
    len: usize,
) -> *mut c_char {
    guard(
        || {
            if session.is_null() {
                error("null session handle");
                return ptr::null_mut();
            }
            let Some(text) = (unsafe { input(bytes, len, MAX_REQUEST_BYTES) }) else { return ptr::null_mut() };
            let result = unsafe { &*session }.request(text).to_json();
            CString::new(result).expect("escaped JSON").into_raw()
        },
        ptr::null_mut(),
    )
}
/// Releases a session; null is accepted.
/// # Safety
/// The handle must originate here, be freed once, and have no requests in flight.
#[no_mangle]
pub unsafe extern "C" fn gamectx_session_free(session: *mut gamectx::kit::Session) {
    if !session.is_null() {
        drop(unsafe { Box::from_raw(session) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CStr;
    #[test]
    fn native_lifetime_and_errors() {
        let definition = include_str!("../../../examples/kit/project.json");
        unsafe {
            let p = gamectx_project_new(definition.as_ptr(), definition.len());
            assert!(!p.is_null());
            let req = r#"{"version":1,"id":"a","op":"doctor","args":{}}"#;
            let reply = gamectx_project_request(p, req.as_ptr(), req.len());
            assert!(CStr::from_ptr(reply).to_str().unwrap().contains("\"ok\":true"));
            gamectx_string_free(reply);
            let bad = gamectx_project_request(p, b"{".as_ptr(), 1);
            assert!(CStr::from_ptr(bad).to_str().unwrap().contains("\"ok\":false"));
            gamectx_string_free(bad);
            gamectx_project_free(p);
            assert!(gamectx_project_new(ptr::null(), 0).is_null());
            let e = gamectx_last_error();
            assert!(!CStr::from_ptr(e).to_bytes().is_empty());
            gamectx_string_free(e);
        }
    }
}
