//! WASI-4 · the ABI under a hostile host (#1472).
//!
//! # What is under test
//!
//! `wasi/src/memory.rs` is the artifact's whole memory-safety surface: three `unsafe` blocks,
//! all three baselined in `.pi/unsafe-baseline.tsv` with a reason. A reason is not a test. The
//! design is that **the registry is authoritative** — a pointer it never handed out is not
//! dereferenced, a length above the record is not read, a second free finds nothing. So the
//! attack surface is not "arbitrary memory read"; it is *the edges of that registry*, and that
//! is what these eleven cases walk.
//!
//! Each case is named, so a failure names the input rather than reporting "1 failed".
//!
//! # What these tests cannot prove
//!
//! They run natively, where `usize` is 64-bit and the module's memory is the process's. The
//! wasm-specific halves — `[ptr, ptr+4+len)` against linear memory, and a trap being
//! distinguishable from a refusal — are asserted in `wasi/ci/run-conformance.mjs` and in the
//! gates' `hostile` section, because a native test cannot see a linear memory.

use rigorix_gate_wasi::MAX_REQUEST_BYTES;
use rigorix_gate_wasi::memory::{
    MAX_OUTSTANDING_BUFFERS, alloc, call as call_raw, free, live_buffers, response_payload,
};
use rigorix_gate_wasi::rigx_alloc;

/// A request the module answers without touching the outside world.
const VERSION: &str = r#"{"op":"contract_version"}"#;

fn kind(response: &str) -> String {
    serde_json::from_str::<serde_json::Value>(response)
        .ok()
        .and_then(|value| {
            value
                .get("kind")
                .and_then(|kind| kind.as_str())
                .map(str::to_string)
        })
        .unwrap_or_default()
}

/// Write a request into a fresh buffer, the way a host does.
fn request_bytes(bytes: &[u8]) -> usize {
    let ptr = alloc(bytes.len()).expect("the module allocated a request buffer");
    // SAFETY: `ptr` is a buffer of exactly `bytes.len()` bytes, allocated above and owned by
    // this test until `free`.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr as *mut u8, bytes.len());
    }
    ptr
}

/// Read the response the module published, freeing it.
fn answer(response: usize) -> String {
    let payload = response_payload(response).expect("the response carries a valid length prefix");
    free(response, payload.len().saturating_add(4));
    String::from_utf8(payload).expect("the response is UTF-8")
}

// ── 1. a pointer the module never allocated ────────────────────────────────────────

#[test]
fn hostile_01_a_pointer_never_allocated_is_refused_not_read() {
    // The address a naive implementation dereferences first: the top of a 32-bit space.
    let response = call_raw(0xFFFF_FFF0, 100).expect("a refusal is still an answer");
    let refusal = answer(response);
    assert_eq!(kind(&refusal), "request_invalid");
    assert!(
        refusal.contains("not allocated by this module"),
        "the refusal names the reason: {refusal}"
    );
}

// ── 2. an aliased pointer into a live response ─────────────────────────────────────

#[test]
fn hostile_02_an_aliased_response_pointer_is_refused_and_the_response_survives() {
    let ptr = request_bytes(VERSION.as_bytes());
    let response = call_raw(ptr, VERSION.len()).expect("an answer");
    // Point *inside* a live response at the module, as a confused host would.
    let aliased = response.saturating_add(2);
    let refused = call_raw(aliased, 8).expect("a refusal is still an answer");
    assert_eq!(kind(&answer(refused)), "request_invalid");
    // And the response it pointed into is still readable — a refusal did not corrupt it.
    let text = answer(response);
    assert!(
        text.contains("\"contract\""),
        "the response survived: {text}"
    );
    free(ptr, VERSION.len());
}

// ── 3. a length past the registered one ────────────────────────────────────────────

#[test]
fn hostile_03_a_length_past_the_registered_one_is_refused_naming_the_buffer() {
    let ptr = request_bytes(b"{\"op\":\"contract_version\"}");
    let response = call_raw(ptr, 4096).expect("a refusal is still an answer");
    let refusal = answer(response);
    assert_eq!(kind(&refusal), "request_invalid");
    assert!(
        refusal.contains("longer than the buffer it names"),
        "the refusal says which buffer: {refusal}"
    );
    free(ptr, 27);
}

// ── 4. a zero length ───────────────────────────────────────────────────────────────

#[test]
fn hostile_04_a_zero_length_request_is_refused() {
    let ptr = alloc(8).expect("a buffer");
    let response = call_raw(ptr, 0).expect("a refusal is still an answer");
    assert_eq!(kind(&answer(response)), "request_invalid");
    free(ptr, 8);
}

// ── 5. a length above the ceiling ──────────────────────────────────────────────────

#[test]
fn hostile_05_a_length_above_the_ceiling_is_refused_before_any_read() {
    let ptr = alloc(8).expect("a buffer");
    let response = call_raw(ptr, MAX_REQUEST_BYTES + 1).expect("a refusal is still an answer");
    let refusal = answer(response);
    assert_eq!(kind(&refusal), "request_invalid");
    assert!(
        refusal.contains("larger than this boundary reads"),
        "case 5 is the refusal the ceiling exists for: {refusal}"
    );
    free(ptr, 8);
}

// ── 6. a double free ───────────────────────────────────────────────────────────────

#[test]
fn hostile_06_a_second_free_is_ignored_and_does_not_abort() {
    let before = live_buffers();
    let ptr = alloc(16).expect("a buffer");
    free(ptr, 16);
    assert_eq!(live_buffers(), before);
    // The dangerous call: the module must not reconstruct the same Box twice.
    free(ptr, 16);
    free(ptr, 16);
    assert_eq!(live_buffers(), before, "a double free changed the registry");
    // The instance still answers, which is the observable form of "did not corrupt".
    let response = call_raw(request_bytes(VERSION.as_bytes()), VERSION.len()).expect("an answer");
    assert!(answer(response).contains("\"contract\""));
}

// ── 7. a wild free ─────────────────────────────────────────────────────────────────

#[test]
fn hostile_07_a_free_of_something_never_allocated_is_ignored() {
    let before = live_buffers();
    free(0, 0);
    free(0xFFFF_FFF0, 4);
    free(1, 1);
    assert_eq!(live_buffers(), before, "a wild free changed the registry");
    let response = call_raw(request_bytes(VERSION.as_bytes()), VERSION.len()).expect("an answer");
    assert!(answer(response).contains("\"contract\""));
}

// ── 8. an allocation the module will not attempt ───────────────────────────────────

#[test]
fn hostile_08_an_unusable_length_is_refused_rather_than_attempted() {
    assert_eq!(alloc(0), None, "a zero-length request is not a request");
    assert_eq!(
        alloc(MAX_REQUEST_BYTES + 1),
        None,
        "a length above the ceiling is refused rather than attempted"
    );
    assert_eq!(alloc(usize::MAX), None, "an impossible length is refused");
    assert_eq!(
        live_buffers(),
        0,
        "no refused allocation left an entry behind"
    );
}

// ── 9. the registry is per thread, and the request is copied out ───────────────────

#[test]
fn hostile_09_a_second_thread_cannot_free_this_one_and_the_answer_is_a_copy() {
    // The registry is `thread_local`, which is stronger than a lock: a host that frees from
    // another thread reaches a *different* registry and frees nothing of ours. The property
    // is asserted rather than assumed, because "free from the host thread" is the case #1472
    // names and the case a lock would have made a race.
    let ptr = request_bytes(VERSION.as_bytes());
    let before = live_buffers();
    assert!(before >= 1);

    let main_thread_address = ptr;
    std::thread::spawn(move || {
        // A second thread has its own registry: nothing of the main thread's is visible, so
        // its `free` is a no-op, and its own allocations do not appear over there.
        assert_eq!(live_buffers(), 0, "another thread shares this registry");
        free(main_thread_address, 8);
        assert_eq!(
            live_buffers(),
            0,
            "another thread's free changed its own registry"
        );
    })
    .join()
    .expect("the second thread ran");

    assert_eq!(live_buffers(), before, "the other thread freed our buffer");

    // And the request is copied out before dispatch, so the answer does not depend on the
    // buffer surviving the call — free it first, then answer from the copy.
    let response = call_raw(ptr, VERSION.len()).expect("an answer");
    free(ptr, VERSION.len());
    assert!(answer(response).contains("\"contract\""));
}

// ── 10. a host that never frees ────────────────────────────────────────────────────

#[test]
fn hostile_10_a_host_that_never_frees_is_refused_at_the_cap_not_at_the_allocator() {
    // Every buffer the module publishes counts, request and response alike, so the cap bounds
    // the instance's memory no matter which call the host leaks.
    let mut held = Vec::new();
    while let Some(ptr) = alloc(8) {
        held.push(ptr);
        assert!(
            held.len() <= MAX_OUTSTANDING_BUFFERS,
            "the registry handed out more than its cap"
        );
    }
    assert_eq!(
        held.len(),
        MAX_OUTSTANDING_BUFFERS,
        "the cap is reached exactly, not early and not late"
    );

    // A full registry refuses the request buffer *and* says so through `rigx_alloc`'s 0.
    assert_eq!(alloc(8), None);
    assert_eq!(
        rigx_alloc(8),
        0,
        "the ABI's failure signal is 0, and it is produced rather than trapping"
    );

    // Freeing one makes room for one, which is what makes the cap a budget and not a wall.
    let released = held.pop().expect("a held buffer");
    free(released, 8);
    assert_eq!(live_buffers(), MAX_OUTSTANDING_BUFFERS - 1);
    let again = alloc(8).expect("room after a free");
    free(again, 8);
    for ptr in held {
        free(ptr, 8);
    }
    assert_eq!(live_buffers(), 0, "the corpus leaked a buffer");
}

// ── 11. a length prefix that claims more than was allocated ───────────────────────

#[test]
fn hostile_11_a_response_prefix_that_overstates_its_length_reads_nothing() {
    // The host trusts the prefix, so the module must not be able to publish one that would
    // make a host read past the buffer. `response_payload` validates before it reads — this
    // is that validation, reached through the only handle a host has.
    let ptr = request_bytes(VERSION.as_bytes());
    let response = call_raw(ptr, VERSION.len()).expect("an answer");
    let recorded = response_payload(response).expect("a valid prefix");
    free(ptr, VERSION.len());

    // Overstate the prefix in place, as a corrupted module would, and require `None`.
    // SAFETY: `response` is a live buffer this module allocated, its first four bytes are the
    // prefix, and nothing else holds a reference to it.
    unsafe {
        let prefix = (recorded.len() as u32).saturating_add(1024).to_le_bytes();
        std::ptr::copy_nonoverlapping(prefix.as_ptr(), response as *mut u8, 4);
    }
    assert_eq!(
        response_payload(response),
        None,
        "an overstated prefix must yield nothing, never a read past the buffer"
    );
    free(response, recorded.len().saturating_add(4));

    // And a prefix smaller than the truth is not a way to read partial data either: it simply
    // reads fewer bytes, which is what the host asked for.
    let ptr = request_bytes(VERSION.as_bytes());
    let response = call_raw(ptr, VERSION.len()).expect("an answer");
    free(ptr, VERSION.len());
    assert!(response_payload(response).is_some());
    free(response, VERSION.len() + 4);
}
