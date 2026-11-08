//! The three exports — and the crate's entire `unsafe` surface.
//!
//! # The protocol
//!
//! ```text
//! request  : the host allocates, writes, and passes (ptr, len)
//! response : [len: u32 LE][payload], pointer returned; the host reads len, copies, frees
//! ```
//!
//! A four-byte length prefix rather than a second return value: a non-JS host returns one
//! `i32` from a call, and an ABI that needs two needs a side channel or a struct — both of
//! which are a second dialect.
//!
//! # Ownership
//!
//! Whoever allocated the buffer frees it. The host frees its request after `rigx_call`
//! returns; the host frees the response after copying it out. Nothing here keeps a pointer
//! into host memory beyond the registry's record of what it handed out.
//!
//! # Why a registry, when a pointer is enough
//!
//! `rigx_free(ptr, len)` on untrusted input is the classic double-free/invalid-free pair.
//! The registry is the module's own record of what it handed out, so a pointer it never
//! allocated is ignored and a wrong length frees what the module actually allocated rather
//! than what the caller claimed. One `BTreeMap` in a single-threaded target — hence no lock
//! and no atomics.
//!
//! # Why the `usize` functions exist beside the `u32` ones
//!
//! The ABI is `u32` because `wasm32` pointers are. A **native** test cannot express that: a
//! host pointer does not fit in 32 bits, so a `u32`-only API is untestable off the target
//! and the interesting half of this file — the registry, the length prefix, the refusal
//! path — would be verified only by the conformance driver. So the protocol is written over
//! `usize` and the three exports are the identity conversion (`u32 == usize` on wasm32) plus
//! the fact that an address above `u32::MAX` is refused rather than truncated.

use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::abi::{MAX_REQUEST_BYTES, dispatch};

thread_local! {
    /// `pointer -> length`, for the buffers this module has handed out.
    ///
    /// A `BTreeMap`: the target is single threaded and the map is tiny, so determinism is
    /// worth more than a hash function.
    static BUFFERS: RefCell<BTreeMap<usize, usize>> = const { RefCell::new(BTreeMap::new()) };
}

/// Allocate a buffer of `len` zeroed bytes and hand back its pointer.
///
/// A length of zero, or one above [`MAX_REQUEST_BYTES`], is not a request: the answer is
/// `None`, which the exports turn into `0`.
#[allow(unsafe_code)]
pub fn alloc(len: usize) -> Option<usize> {
    if len == 0 || len > MAX_REQUEST_BYTES {
        return None;
    }
    publish(vec![0u8; len])
}

/// Release a buffer this module handed out.
///
/// A pointer this module did not hand out, or a length that disagrees with its own record,
/// is ignored: the registry is the truth, so a host cannot make the module free memory it
/// does not own.
#[allow(unsafe_code)]
pub fn free(ptr: usize, _len: usize) {
    let removed = BUFFERS.with(|buffers| match buffers.try_borrow_mut() {
        Ok(mut live) => Some(live.remove(&ptr)),
        // A borrow that is already out is a defect in this module, not a host error — and a
        // defect must not become a trap on a target with no unwinder (ADR-012). Nothing is
        // freed, which leaks one buffer rather than aborting a live instance.
        Err(_) => None,
    });
    let Some(Some(len)) = removed else {
        return;
    };
    // SAFETY: `ptr` came from `publish`, which leaked a `Box<[u8]>` of exactly `len` bytes
    // and recorded that length here; removing the entry means this is the only free path for
    // it, so the same buffer cannot be reconstructed twice.
    unsafe {
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            ptr as *mut u8,
            len,
        )));
    }
}

/// Answer a request stored at `(ptr, len)`; `None` means no response could be produced.
///
/// The request buffer is not freed here: the host allocated it and owns it.
#[allow(unsafe_code)]
pub fn call(ptr: usize, len: usize) -> Option<usize> {
    let request = match read(ptr, len) {
        Ok(bytes) => match std::str::from_utf8(bytes) {
            Ok(text) => text.to_string(),
            Err(_) => {
                return publish_payload(
                    r#"{"kind":"request_invalid","message":"the request is not UTF-8"}"#,
                );
            }
        },
        Err(message) => {
            let refusal =
                serde_json::json!({ "kind": "request_invalid", "message": message }).to_string();
            return publish_payload(&refusal);
        }
    };
    // The request is copied out before dispatch: dispatch runs the whole engine and a host
    // that freed its buffer during the call would otherwise be reading freed memory.
    publish_payload(&dispatch(&request))
}

/// Copy `len` bytes out of a buffer this module handed out.
///
/// # Errors
/// A pointer this module never allocated, or a length beyond what it allocated, is a
/// boundary refusal rather than a read of whatever happens to be at that address.
#[allow(unsafe_code)]
fn read(ptr: usize, len: usize) -> Result<&'static [u8], &'static str> {
    if len == 0 {
        return Err("the request is empty");
    }
    if len > MAX_REQUEST_BYTES {
        return Err("the request is larger than this boundary reads");
    }
    let capacity = BUFFERS.with(|buffers| {
        buffers
            .try_borrow()
            .ok()
            .and_then(|live| live.get(&ptr).copied())
    });
    let Some(capacity) = capacity else {
        return Err("the request pointer was not allocated by this module");
    };
    if len > capacity {
        return Err("the request is longer than the buffer it names");
    }
    // SAFETY: `ptr` is registered, so it came from a leaked `Box<[u8]>` of `capacity` bytes
    // that is still alive (the host frees it, and only through `free`); `len <= capacity`
    // keeps the slice inside it. The `'static` lifetime is the buffer outliving this call by
    // construction — the host owns it and frees it only after the call returns.
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len) };
    Ok(bytes)
}

/// Wrap a payload in its length prefix and hand the host a pointer.
fn publish_payload(payload: &str) -> Option<usize> {
    let bytes = payload.as_bytes();
    let len = u32::try_from(bytes.len()).ok()?;
    let mut buffer = Vec::with_capacity(bytes.len().saturating_add(4));
    buffer.extend_from_slice(&len.to_le_bytes());
    buffer.extend_from_slice(bytes);
    publish(buffer)
}

/// Leak a buffer into the registry and return its pointer.
#[allow(unsafe_code)]
fn publish(bytes: Vec<u8>) -> Option<usize> {
    let len = bytes.len();
    let boxed: Box<[u8]> = bytes.into_boxed_slice();
    let ptr = Box::into_raw(boxed).cast::<u8>() as usize;
    if ptr == 0 {
        return None;
    }
    BUFFERS.with(|buffers| {
        if let Ok(mut live) = buffers.try_borrow_mut() {
            live.insert(ptr, len);
        }
    });
    Some(ptr)
}

/// The response's payload, read the way a host reads it — for tests and for the driver that
/// proves the same bytes cross `node:wasi`.
#[doc(hidden)]
#[allow(unsafe_code)]
pub fn response_payload(ptr: usize) -> Option<Vec<u8>> {
    let capacity = BUFFERS.with(|buffers| {
        buffers
            .try_borrow()
            .ok()
            .and_then(|live| live.get(&ptr).copied())
    })?;
    if capacity < 4 {
        return None;
    }
    // SAFETY: as in `read` — the pointer is registered with this capacity and still alive.
    unsafe {
        let header = std::slice::from_raw_parts(ptr as *const u8, 4);
        // `try_into` on the slice rather than four indexes: the lint that forbids indexing on
        // this path is the same rule that says a boundary must not panic, and a `[u8; 4]` that
        // cannot be built is a `None` here rather than a trap.
        let Ok(four) = <[u8; 4]>::try_from(header) else {
            return None;
        };
        let len = u32::from_le_bytes(four) as usize;
        if len.saturating_add(4) > capacity {
            return None;
        }
        Some(std::slice::from_raw_parts((ptr as *const u8).add(4), len).to_vec())
    }
}

/// How many buffers this module is currently holding for the host.
///
/// Exposed to the crate's own tests only: it is how a test distinguishes "the host freed its
/// request" from "the module leaked one per call", which no single call can show.
#[doc(hidden)]
pub fn live_buffers() -> usize {
    BUFFERS.with(|buffers| buffers.try_borrow().map(|live| live.len()).unwrap_or(0))
}

// ── the ABI, as thin as it can be ────────────────────────────────────────────────
//
// On `wasm32`, `usize` is `u32`, so each of these is the identity conversion plus the
// registry's own checks. `0` is the only failure signal the ABI has: a refusal buffer would
// be a second protocol, and an errno would be an import.

/// Allocate a request buffer. Returns `0` when the length is unusable.
#[unsafe(no_mangle)]
// SAFETY: the attribute asserts the symbol name is unique in the linked module, which
// is the whole of this crate's export surface — three names, declared here and nowhere else.
#[allow(unsafe_code)]
pub extern "C" fn rigx_alloc(len: u32) -> u32 {
    let Ok(len) = usize::try_from(len) else {
        return 0;
    };
    narrow(alloc(len))
}

/// Release a buffer this module handed out. A pointer it did not hand out is ignored.
#[unsafe(no_mangle)]
// SAFETY: the attribute asserts the symbol name is unique in the linked module, which
// is the whole of this crate's export surface — three names, declared here and nowhere else.
#[allow(unsafe_code)]
pub extern "C" fn rigx_free(ptr: u32, len: u32) {
    free(ptr as usize, len as usize);
}

/// The one entry: read the request at `(ptr, len)` and answer with a response buffer.
///
/// Returns `0` when no response could be produced, otherwise a pointer whose first four
/// bytes are the payload length.
#[unsafe(no_mangle)]
// SAFETY: the attribute asserts the symbol name is unique in the linked module, which
// is the whole of this crate's export surface — three names, declared here and nowhere else.
#[allow(unsafe_code)]
pub extern "C" fn rigx_call(ptr: u32, len: u32) -> u32 {
    narrow(call(ptr as usize, len as usize))
}

/// An address above `u32::MAX` is refused rather than truncated into a different buffer.
fn narrow(pointer: Option<usize>) -> u32 {
    pointer
        .and_then(|pointer| u32::try_from(pointer).ok())
        .unwrap_or(0)
}
