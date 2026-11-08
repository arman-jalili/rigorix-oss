//! WASI-1 · the boundary, the seeds, and the five goldens (#1465).
//!
//! # What these tests are, and are not
//!
//! They run **natively**, against the same `dispatch` the artifact calls. That makes them a
//! test of the protocol and of the engine seam, not of the artifact: the artifact's own
//! claims — an empty import section, the size ceiling, `node:wasi` loading it — are checked
//! by `wasi/ci/`, because a native test cannot see an import section.
//!
//! The goldens are the *browser crate's* vectors, replayed through this boundary. That is
//! the point of "one implementation, two bindings": if the two artifacts ever diverged, the
//! five recorded digests would be the thing that noticed.

use std::path::PathBuf;

use rigorix_gate_wasi::MAX_REQUEST_BYTES;
use rigorix_gate_wasi::memory::{alloc, call as call_raw, free, live_buffers, response_payload};

// ── the pointer protocol ───────────────────────────────────────────────────────────

/// Hand a request over exactly as a host would: allocate, copy, call, read, free.
fn call(request: &str) -> String {
    let bytes = request.as_bytes();
    let ptr = alloc(bytes.len()).expect("the module refused to allocate a request buffer");
    // SAFETY: `ptr` is a buffer of exactly `bytes.len()` bytes this module just allocated,
    // and the host owns it until `free`.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr as *mut u8, bytes.len());
    }

    let response = call_raw(ptr, bytes.len()).expect("the module produced no response");
    free(ptr, bytes.len());
    let payload = response_payload(response).expect("a response carries a length prefix");
    free(response, payload.len().saturating_add(4));
    String::from_utf8(payload).expect("the response is UTF-8")
}

fn kind(response: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(response)
        .unwrap_or_else(|error| panic!("a response is JSON, not {response}: {error}"));
    value
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

// ── S1 · an unknown operation is a refusal, not a trap ─────────────────────────────

#[test]
fn s1_an_unknown_operation_is_request_invalid() {
    let response = call(r#"{"op":"verify"}"#);
    assert_eq!(kind(&response), "request_invalid");
    assert!(
        response.contains("verify"),
        "the refusal names the operation it did not know: {response}"
    );
}

#[test]
fn s1_a_payload_with_no_operation_is_request_invalid() {
    assert_eq!(kind(&call(r#"{}"#)), "request_invalid");
    assert_eq!(kind(&call(r#"{"op":42}"#)), "request_invalid");
}

// ── S2 · a run without a session gives the browser crate's refusal ─────────────────

#[test]
fn s2_attempt_run_before_open_session_refuses_with_handle_invalid() {
    let response = call(
        r#"{"op":"attempt_run","handle":{"handle":"nope"},"action":{"name":"a","params":{}}}"#,
    );
    assert_eq!(
        kind(&response),
        "handle_invalid",
        "the same refusal the browser artifact gives, not a panic: {response}"
    );
}

// ── a trap is not a refusal ────────────────────────────────────────────────────────

#[test]
fn a_truncated_request_refuses_and_the_instance_keeps_answering() {
    let truncated = r#"{"op":"attempt_run","handle":{"hand"#;
    assert_eq!(kind(&call(truncated)), "request_invalid");

    // The instance is still alive: a second call answers, and answers correctly.
    let version = call(r#"{"op":"contract_version"}"#);
    let value: serde_json::Value = serde_json::from_str(&version).expect("a version object");
    assert!(value.get("contract").is_some(), "{version}");
    assert_eq!(value["gate_evaluation"], true);
}

#[test]
fn an_empty_request_refuses_rather_than_reading_nothing() {
    let ptr = alloc(8).expect("a buffer");
    let response = call_raw(ptr, 0).expect("an empty request still gets an answer");
    free(ptr, 8);
    let payload = response_payload(response).expect("a length prefix");
    free(response, payload.len().saturating_add(4));
    assert_eq!(
        kind(&String::from_utf8(payload).expect("UTF-8")),
        "request_invalid"
    );
}

// ── the buffer registry is what makes free safe ───────────────────────────────────

#[test]
fn a_pointer_the_module_never_allocated_is_ignored() {
    // A host that frees something it made up must not make the module free arbitrary
    // memory. Nothing observable happens, which is the assertion.
    free(0xdead_beef, 16);
    free(1, 1);
    assert_eq!(kind(&call(r#"{"op":"contract_version"}"#)), "");
}

#[test]
fn a_response_is_released_by_freeing_its_own_pointer() {
    let before = live_buffers();
    let ptr = alloc(4).expect("a buffer");
    assert_eq!(live_buffers(), before + 1);
    free(ptr, 4);
    assert_eq!(live_buffers(), before);
}

#[test]
fn alloc_refuses_a_length_it_will_not_read() {
    assert_eq!(alloc(0), None, "a zero-length request is not a request");
    assert_eq!(
        alloc(MAX_REQUEST_BYTES + 1),
        None,
        "a length above the ceiling is refused rather than attempted"
    );
}

// ── the five goldens, through this boundary ───────────────────────────────────────

fn vectors() -> Vec<serde_json::Value> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../wasm/tests/conformance/vectors");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("{} is not readable: {error}", dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    paths.sort();
    assert!(
        paths.len() >= 5,
        "the vector directory holds {} vectors; this artifact claims five",
        paths.len()
    );
    paths
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{} is not readable: {error}", path.display()));
            serde_json::from_str(&text)
                .unwrap_or_else(|error| panic!("{} is not a vector: {error}", path.display()))
        })
        .collect()
}

fn hex_to_base64(hex: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes: Vec<u8> = hex
        .as_bytes()
        .chunks(2)
        .filter_map(|pair| {
            let hi = *pair.first()?;
            let lo = *pair.get(1)?;
            let digit = |c: u8| match c {
                b'0'..=b'9' => Some(c - b'0'),
                b'a'..=b'f' => Some(c - b'a' + 10),
                b'A'..=b'F' => Some(c - b'A' + 10),
                _ => None,
            };
            Some(digit(hi)? * 16 + digit(lo)?)
        })
        .collect();
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            *chunk.first().unwrap_or(&0),
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for index in 0..4 {
            if index <= chunk.len() {
                let position = (n >> (18 - 6 * index)) & 0b11_1111;
                out.push(char::from(ALPHABET[position as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[test]
fn every_committed_vector_replays_through_this_boundary_to_its_recorded_digest() {
    let vectors = vectors();
    assert_eq!(
        vectors.len(),
        5,
        "five vectors were recorded; this must see five"
    );

    for vector in &vectors {
        let id = vector["id"].as_str().unwrap_or("<unnamed>");
        let opened = call(
            &serde_json::json!({
                "op": "open_session",
                "scenario": vector["scenario"],
                "fixtures": vector["fixtures"],
                "clock": vector["clock"],
                "seed": vector["seed"],
            })
            .to_string(),
        );
        let handle = serde_json::from_str::<serde_json::Value>(&opened)
            .unwrap_or_else(|error| panic!("{id}: the handle is not JSON: {error}"))
            .get("handle")
            .cloned()
            .unwrap_or_else(|| panic!("{id}: open_session did not answer with a handle: {opened}"));

        let ran = call(
            &serde_json::json!({
                "op": "attempt_run",
                "handle": { "handle": handle },
                "action": vector["scenario"]["action"],
            })
            .to_string(),
        );
        let outcome: serde_json::Value = serde_json::from_str(&ran)
            .unwrap_or_else(|error| panic!("{id}: the outcome is not JSON: {error}"));

        let expected = &vector["expected_decision"];
        // A vector marked `seeded_divergence` records a decision the run must **not**
        // reproduce — that is what the marker means (`ConformanceReport::is_pass`:
        // `matched != seeded_divergence`). Comparing every vector for equality would fail on
        // the one vector whose record is a divergence.
        let matched = outcome["decision_digest"] == expected["sha256"]
            && outcome["decision_canonical"]
                == hex_to_base64(expected["hex"].as_str().unwrap_or_default());
        let seeded = vector["seeded_divergence"].as_bool().unwrap_or(false);
        assert_eq!(
            matched, !seeded,
            "{id} ({seeded}): digest {} — expected {}",
            outcome["decision_digest"], expected["sha256"]
        );

        let sealed = call(
            &serde_json::json!({
                "op": "export_artifact_set",
                "handle": { "handle": handle },
            })
            .to_string(),
        );
        assert_ne!(
            kind(&sealed),
            "request_invalid",
            "{id}: sealing a session must not be a boundary refusal: {sealed}"
        );
    }
}

#[test]
fn the_recorded_capacity_refusal_still_names_its_operands() {
    // vec-0002 is the capacity golden. The point of the boundary is that a WASI host sees
    // the arithmetic without parsing the engine's payload — if this ever regresses to a
    // bare `capacity_exceeded`, the artifact has lost the thing it was built to deliver.
    let vector = vectors()
        .into_iter()
        .find(|v| v["id"] == "vec-0002")
        .expect("vec-0002 is committed");

    let opened = call(
        &serde_json::json!({
            "op": "open_session",
            "scenario": vector["scenario"],
            "fixtures": vector["fixtures"],
            "clock": vector["clock"],
            "seed": vector["seed"],
        })
        .to_string(),
    );
    let handle = opened
        .split("{\"handle\":\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_default()
        .to_string();

    let ran = call(
        &serde_json::json!({
            "op": "attempt_run",
            "handle": { "handle": handle },
            "action": vector["scenario"]["action"],
        })
        .to_string(),
    );
    let outcome: serde_json::Value = serde_json::from_str(&ran).expect("an outcome object");
    let step = &outcome["steps"][0];

    assert_eq!(step["kind"], "capacity_exceeded");
    assert_eq!(step["detail"]["kind"], "capacity");
    assert_eq!(step["detail"]["overage"], 7_420);
    assert_eq!(step["detail"]["hard_limit"], 1_420_000);
}
