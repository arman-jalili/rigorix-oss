//! Integration test: verify against a mock JWKS — valid → Verified;
//! tampered → Unverified.
//!
//! Covers identity module acceptance criterion #5:
//! "verify against mock JWKS: valid → Verified; tampered → Unverified".
//!
//! Uses `wiremock` as the mock IdP JWKS endpoint and real RS256 cryptography
//! (rsa crate): the test generates a keypair, signs tokens with the private
//! key, serves the public key as a JWKS document, and drives `JwksVerifier`.

use async_trait::async_trait;
use base64::Engine as _;
use rsa::RsaPrivateKey;
use rsa::pkcs1v15::SigningKey;
use rsa::sha2::Sha256;
use rsa::signature::{SignatureEncoding, Signer};
use rsa::traits::PublicKeyParts;
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
    private_key: RsaPrivateKey,
    kid: String,
}

impl MockIdp {
    fn new() -> Self {
        let mut rng = rsa::rand_core::OsRng;
        let private_key = RsaPrivateKey::new(&mut rng, 2048).expect("rsa keygen");
        Self {
            private_key,
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

        let signing_key = SigningKey::<Sha256>::new(self.private_key.clone());
        let signature = signing_key.sign(signing_content.as_bytes()).to_bytes();

        format!(
            "{signing_content}.{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(signature)
        )
    }

    /// JWKS document exposing the public key under `kid`.
    fn jwks_json(&self) -> serde_json::Value {
        let public = self.private_key.to_public_key();
        serde_json::json!({
            "keys": [{
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": self.kid,
                "n": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public.n().to_bytes_be()),
                "e": base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public.e().to_bytes_be()),
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
    let token = "unused"; // header parse happens first... use a well-formed header

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
