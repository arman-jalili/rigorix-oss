//! Integration test: verify against a mock JWKS — valid → Verified;
//! tampered → Unverified.
//!
//! Covers identity module acceptance criterion #5:
//! "verify against mock JWKS: valid → Verified; tampered → Unverified".
//!
//! Uses `wiremock` as the mock IdP JWKS endpoint and real RS256 cryptography
//! (rsa crate): the test generates a keypair, signs tokens with the private
//! key, serves the public key as a JWKS document, and drives `JwksVerifier`.

use base64::Engine as _;
// RUSTSEC-2023-0071 (rsa/Marvin): the mock IdP signs with ring (BoringSSL),
// public-key verification only — no private-key timing surface exists.
use ring::rand::SystemRandom;
use ring::signature::{RSA_PKCS1_SHA256, RsaKeyPair};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use rigorix_engine::identity::application::dto::AttestInput;
use rigorix_engine::identity::application::service::{
    IdentityAttestationService, VerificationOutcome,
};
use rigorix_engine::identity::application::service_impl::IdentityAttestationServiceImpl;
use rigorix_engine::identity::domain::{IdentityClaim, IdentitySource};
use rigorix_engine::identity::infrastructure::verifier::{JwksVerifier, TokenVerifier};

/// A self-contained mock IdP: keypair + JWKS serving + JWT signing.
struct MockIdp {
    key_pair: RsaKeyPair,
    kid: String,
}

impl MockIdp {
    /// Embedded 2048-bit RSA test key (PKCS#8 DER). ring cannot generate
    /// RSA keys, so the mock uses a fixed key generated at dev time.
    const PKCS8_DER_HEX: &'static str = "308204bd020100300d06092a864886f70d0101010500048204a7308204a30201000282010100c3f935361c82ba2349bb21bc2e6238339891ccf3be6b600ed0c276f3f43f83a5abf7488e3e95427721af0d690a2db03deee284458649105d19d4b82459a989c1dda63c5a131b8be9de84ee6b17e0e8b8384ebdae4efe2aa290eea28f8b260c7c61371aeb6f0d3070fea24eae0271dfbd1b3f863bc4efc9f105e37432cfa59e715e935e759145082ca88cd0beaf5487ce864f1fdb908b244e29d696941df5254fc5831177c829bd5200b09300c07cc91431d9b9b9941f096cb453130ec4bd3da4577e2b72206970895faed41846edf9339611e29ecf512c4324cf15522c037164699ca42722293cd71efec5b9e38f73a4f7aa6911f85c665b29db96a45904c3330203010001028201006c1b78b02e102b90b5e6c658621a0ab8e3cc628c7f9a0a36821114bfc518988df70c85f8fa2b2aac3f67aaf52c0942351827db21e34f40f8aafc3eccc6ad90f1e24d06f405a067918f1033d9de25531bf4ebf3154a3c49d6be2ef67c4b1da53ba4015b174f7eae1f5748c0309be6a7af516dbca61220d97cdd6bb939227551ae6eb5aa730492b738c64f2ba0903c296a60427e9739612d7e65f31b004787161fc44b11ad3bd7d490a0dfc55a81ff75a208528dbf9137b0f6d001272ef438dab58224d616fd1b6a345bd205b930a80f02b1b65f7f0f11c06c94cfaa0a0e0cb0c7cd75cc4a77681bb4652c9324f93d35bba89b395f165004d3793de6a39c0d07f102818100f2fa45b0fc6594d12dc5296eb5c0533a864aed2093dea451287c828e7273ecffa626f6744d6618ca70310c9dc0d4885645a3f0802aa4f6274d76dad4100e474bb6ee9c17a1654f12907e7235d126c85f898f2d3db5c9d1e74f17ed58829aac5d4613c781852966794a9bba442801060374231f9b567925ea72606c9b2a57b9d902818100ce7a062ce61f50d57b55259aae3d2591d418013c713eea3fc1403d6c10956cb13a86132e0a3434ec220d7d9ab2090d922b5e5de91d10537a6ae75b5443f0dbd511ece84904d0f0997fc22f08c973e45ae6d6ceeadefdc5bba00a23bff70a4d9150e9a031c51dc83e50197c22dffca4fbf16b6a8ed0f7f8445ffa741244fbd1eb028180218b9c038b551aeea63b0a3556b26ecb2daf3a7dbcec88130c5be44a7652baedb0aac06bde23b2588094c5012296351c7410e62b4bb7eaa41275ce5068c70fe0cc28b5342dfc26a6917c63983a7ff839f86be3fb1915fbfccb56aa5605f204c9fbdacf387a81f4bbda2915d6430fa11ce8f3d07149c7000d162d69d1224f6a4102818022b264b944ce7c61f380c279f4cfb7b182c7a9e5834e4445046f8c22cdc29e6a45e063f6b7a6404272127c49f3a30bd1c551ed4c10233f33f22500b6ef57d9493be2e8c1e47a4c042f70ed4077c1eedccbafcb43b2c748641827bc0c3532590893653f133e019c35c47613e3346a9b3aef3dd2c13f227c68d90c18573d9a679302818100abaac5257d9ba393320f96c8612ad34d4c7e5ce1acc325759a642b0a24cdda151d6e5b313781d40c2ce07cebd084d55d3549b5797ebd550a06dffa9acb9cb02354f5498f2c5c2b808a349a807cba2fca0098a65df14660ea9304eae721b6f23342fbf6a99a48005dc89388a408f092d88d6a1a3a415a8a157c989362c1e34a2d";

    fn new() -> Self {
        let der: Vec<u8> = (0..Self::PKCS8_DER_HEX.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&Self::PKCS8_DER_HEX[i..i + 2], 16).expect("der hex"))
            .collect();
        let key_pair = RsaKeyPair::from_pkcs8(&der).expect("embedded test key is valid");
        Self {
            key_pair,
            kid: "mock-idp-key-1".to_string(),
        }
    }

    /// Sign a JWT (RS256) with the given payload claims.
    fn sign(&self, claims: serde_json::Value) -> String {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            serde_json::to_vec(&serde_json::json!({
                "alg": "RS256",
                "kid": self.kid,
                "typ": "JWT",
            }))
            .expect("header json"),
        );
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&claims).expect("claims json"));
        let signing_content = format!("{header}.{payload}");

        let mut signature = vec![0u8; self.key_pair.public().modulus_len()];
        self.key_pair
            .sign(
                &RSA_PKCS1_SHA256,
                &SystemRandom::new(),
                signing_content.as_bytes(),
                &mut signature,
            )
            .expect("rsa signing");

        let sig = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&signature);
        format!("{signing_content}.{sig}")
    }

    /// JWKS document exposing the public key under `kid`.
    fn jwks_json(&self) -> serde_json::Value {
        serde_json::json!({
            "keys": [{
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": self.kid,
                "n": "w_k1NhyCuiNJuyG8LmI4M5iRzPO-a2AO0MJ28_Q_g6Wr90iOPpVCdyGvDWkKLbA97uKERYZJEF0Z1LgkWamJwd2mPFoTG4vp3oTuaxfg6Lg4Tr2uTv4qopDuoo-LJgx8YTca628NMHD-ok6uAnHfvRs_hjvE78nxBeN0Ms-lnnFek151kUUILKiM0L6vVIfOhk8f25CLJE4p1paUHfUlT8WDEXfIKb1SALCTAMB8yRQx2bm5lB8JbLRTEw7EvT2kV34rciBpcIlfrtQYRu35M5YR4p7PUSxDJM8VUiwDcWRpnKQnIik81x7-xbnjj3Ok96ppEfhcZlsp25akWQTDMw",
                "e": "AQAB",
            }]
        })
    }
}

fn sample_claim() -> IdentityClaim {
    IdentityClaim {
        subject: "user@org".to_string(),
        issuer: "https://mock-idp.example.com".to_string(),
        authority: Some("admin".to_string()),
        source: IdentitySource::IdpToken,
        auth_method: None,
        issued_at: chrono::Utc::now(),
        expires_at: None,
        token_ref: None,
    }
}

#[tokio::test]
async fn test_jwks_verify_valid_token_returns_verified() {
    let idp = MockIdp::new();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/jwks.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(idp.jwks_json()))
        .mount(&server)
        .await;

    let verifier = JwksVerifier::new(format!("{}/.well-known/jwks.json", server.uri()));
    let token = idp.sign(serde_json::json!({
        "sub": "user@org",
        "iss": "https://mock-idp.example.com",
        "exp": chrono::Utc::now().timestamp() + 600,
        "roles": ["admin"],
    }));

    let outcome = verifier
        .verify(&token, &sample_claim())
        .await
        .expect("verify must not error");
    assert_eq!(outcome, VerificationOutcome::Verified);
}

#[tokio::test]
async fn test_jwks_verify_tampered_token_returns_unverified() {
    let idp = MockIdp::new();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/jwks.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(idp.jwks_json()))
        .mount(&server)
        .await;

    let verifier = JwksVerifier::new(format!("{}/.well-known/jwks.json", server.uri()));
    let mut token = idp.sign(serde_json::json!({
        "sub": "user@org",
        "iss": "https://mock-idp.example.com",
        "exp": chrono::Utc::now().timestamp() + 600,
    }));

    // Tamper: swap in DIFFERENT (valid) claims — the JSON still parses, but the
    // signature no longer matches the presented payload.
    let segments: Vec<&str> = token.split('.').collect();
    let tampered_payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&serde_json::json!({
            "sub": "attacker@evil",
            "iss": "https://mock-idp.example.com",
            "exp": chrono::Utc::now().timestamp() + 600,
        }))
        .expect("tampered claims json"),
    );
    token = format!("{}.{}.{}", segments[0], tampered_payload, segments[2]);

    let outcome = verifier
        .verify(&token, &sample_claim())
        .await
        .expect("verify must not error (best-effort)");
    assert!(matches!(
        outcome,
        VerificationOutcome::Unverified { ref reason } if reason.contains("signature mismatch")
    ));
}

#[tokio::test]
async fn test_jwks_verify_unreachable_idp_returns_unverified_no_error() {
    // Point at a port with no listener — the IdP is unreachable.
    let verifier = JwksVerifier::new("http://127.0.0.1:1/.well-known/jwks.json".to_string());

    let token = {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(r#"{"alg":"RS256","kid":"mock-idp-key-1"}"#);
        format!("{header}.payload.signature")
    };

    let outcome = verifier
        .verify(&token, &sample_claim())
        .await
        .expect("unreachable IdP must NOT error");
    assert!(
        matches!(outcome, VerificationOutcome::Unverified { .. }),
        "unreachable IdP degrades to Unverified"
    );
}

#[tokio::test]
async fn test_attest_with_jwks_verified_token_keeps_idp_token_source() {
    let idp = MockIdp::new();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/jwks.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(idp.jwks_json()))
        .mount(&server)
        .await;

    let service = IdentityAttestationServiceImpl::with_verifier(Box::new(JwksVerifier::new(
        format!("{}/.well-known/jwks.json", server.uri()),
    )));
    let token = idp.sign(serde_json::json!({
        "sub": "user@org",
        "iss": "https://mock-idp.example.com",
        "exp": chrono::Utc::now().timestamp() + 600,
        "roles": ["admin"],
    }));

    let claim = service
        .attest(AttestInput {
            token: Some(token),
            principal: None,
            issuer: Some("https://mock-idp.example.com".to_string()),
            auth_method: None,
        })
        .await
        .expect("attest must not error");

    // Verified against the mock JWKS → the IdpToken source is retained and the
    // presented roles remain as evidence.
    assert_eq!(claim.source, IdentitySource::IdpToken);
    assert_eq!(claim.authority.as_deref(), Some("admin"));
}

#[tokio::test]
async fn test_attest_with_tampered_token_degrades_to_unverified() {
    let idp = MockIdp::new();
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/jwks.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(idp.jwks_json()))
        .mount(&server)
        .await;

    let service = IdentityAttestationServiceImpl::with_verifier(Box::new(JwksVerifier::new(
        format!("{}/.well-known/jwks.json", server.uri()),
    )));
    let mut token = idp.sign(serde_json::json!({
        "sub": "user@org",
        "iss": "https://mock-idp.example.com",
        "exp": chrono::Utc::now().timestamp() + 600,
        "roles": ["admin"],
    }));

    // Tamper: swap in DIFFERENT (valid) claims — the JSON still parses, but the
    // signature no longer matches the presented payload.
    let segments: Vec<&str> = token.split('.').collect();
    let tampered_payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        serde_json::to_vec(&serde_json::json!({
            "sub": "attacker@evil",
            "iss": "https://mock-idp.example.com",
            "exp": chrono::Utc::now().timestamp() + 600,
        }))
        .expect("tampered claims json"),
    );
    token = format!("{}.{}.{}", segments[0], tampered_payload, segments[2]);

    let claim = service
        .attest(AttestInput {
            token: Some(token),
            principal: None,
            issuer: Some("https://mock-idp.example.com".to_string()),
            auth_method: None,
        })
        .await
        .expect("attest must not error (best-effort)");

    assert_eq!(
        claim.source,
        IdentitySource::Unverified,
        "tampered token degrades to the explicit Unverified marker"
    );
    assert_eq!(claim.authority, None, "unverified roles are not evidence");
}
