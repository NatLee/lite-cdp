//! Public, deliberately non-production signing key and loopback-only OAuth/OIDC fixture.
use axum::{
    extract::{Form, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rsa::{pkcs1v15::SigningKey, traits::PublicKeyParts, RsaPrivateKey};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use signature::{SignatureEncoding, Signer};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
#[derive(Clone)]
pub struct MockProvider {
    pub origin: String,
    key: Arc<RsaPrivateKey>,
    challenge: Arc<Mutex<HashMap<String, (String, String)>>>,
}
impl MockProvider {
    pub fn router(origin: &str) -> Router {
        let key = {
            use rand_core::SeedableRng;
            let mut rng = rand_chacha::ChaCha20Rng::from_seed([42u8; 32]);
            RsaPrivateKey::new(&mut rng, 2048).expect("public deterministic fixture key")
        };
        let state = Self {
            origin: origin.into(),
            key: Arc::new(key),
            challenge: Arc::new(Mutex::new(HashMap::new())),
        };
        Router::new()
            .route(
                "/google/issuer/.well-known/openid-configuration",
                get(metadata),
            )
            .route("/google/jwks", get(jwks))
            .route("/{provider}/authorize", get(authorize))
            .route("/{provider}/token", post(token))
            .route("/github/user", get(user))
            .with_state(state)
    }
}
async fn metadata(State(s): State<MockProvider>) -> Json<Value> {
    Json(
        json!({"issuer":format!("{}/google/issuer",s.origin),"authorization_endpoint":format!("{}/google/authorize",s.origin),"token_endpoint":format!("{}/google/token",s.origin),"jwks_uri":format!("{}/google/jwks",s.origin),"response_types_supported":["code"],"subject_types_supported":["public"],"id_token_signing_alg_values_supported":["RS256"],"token_endpoint_auth_methods_supported":["client_secret_post"],"code_challenge_methods_supported":["S256"]}),
    )
}
async fn jwks(State(s): State<MockProvider>) -> Json<Value> {
    let key = s.key.to_public_key();
    Json(
        json!({"keys":[{"kty":"RSA","kid":"fixture-only","use":"sig","alg":"RS256","n":URL_SAFE_NO_PAD.encode(key.n().to_bytes_be()),"e":URL_SAFE_NO_PAD.encode(key.e().to_bytes_be())}]}),
    )
}
async fn authorize(
    State(s): State<MockProvider>,
    axum::extract::Path(provider): axum::extract::Path<String>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    let Some(redirect) = q.get("redirect_uri") else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(mut url) = url::Url::parse(redirect) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    s.challenge.lock().unwrap().insert(
        provider,
        (
            q.get("code_challenge").cloned().unwrap_or_default(),
            q.get("nonce").cloned().unwrap_or_default(),
        ),
    );
    url.query_pairs_mut()
        .append_pair("code", "good")
        .append_pair("state", q.get("state").map(String::as_str).unwrap_or(""));
    (StatusCode::SEE_OTHER, [("location", url.as_str())]).into_response()
}
async fn token(
    State(s): State<MockProvider>,
    axum::extract::Path(provider): axum::extract::Path<String>,
    Form(q): Form<HashMap<String, String>>,
) -> Response {
    let (challenge, nonce) = s
        .challenge
        .lock()
        .unwrap()
        .get(&provider)
        .cloned()
        .unwrap_or_default();
    let verifier = q.get("code_verifier").cloned().unwrap_or_default();
    if q.get("client_id").map(String::as_str) != Some("mock-client")
        || q.get("client_secret").map(String::as_str) != Some("mock-only-secret")
        || URL_SAFE_NO_PAD.encode(Sha256::digest(verifier)) != challenge
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"invalid_grant"})),
        )
            .into_response();
    }
    if provider == "github" {
        return Json(json!({"access_token":"mock-provider-token","token_type":"Bearer"}))
            .into_response();
    }
    let mode = q.get("code").map(String::as_str).unwrap_or("");
    let now = chrono::Utc::now().timestamp();
    let mut claims = json!({"iss":format!("{}/google/issuer",s.origin),"aud":"mock-client","sub":"google-subject-1234","exp":now+3600,"iat":now,"nonce":nonce,"email":"same-email@example.invalid"});
    match mode {
        "bad_nonce" => claims["nonce"] = json!("wrong"),
        "bad_iss" => claims["iss"] = json!("https://wrong.example"),
        "bad_aud" => claims["aud"] = json!("different-client"),
        "expired" => claims["exp"] = json!(now - 3600),
        _ => {}
    }
    let header = json!({"alg":"RS256","kid":"fixture-only"});
    let data = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(header.to_string()),
        URL_SAFE_NO_PAD.encode(claims.to_string())
    );
    let signature = SigningKey::<Sha256>::new((*s.key).clone()).sign(data.as_bytes());
    let mut jwt = format!("{data}.{}", URL_SAFE_NO_PAD.encode(signature.to_bytes()));
    if mode == "bad_sig" {
        jwt.push('x');
    }
    Json(json!({"access_token":"mock-provider-token","token_type":"Bearer","id_token":jwt}))
        .into_response()
}
async fn user(headers: axum::http::HeaderMap) -> Response {
    if headers.get("authorization").and_then(|h| h.to_str().ok())
        != Some("Bearer mock-provider-token")
    {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    Json(json!({"id":1234,"login":"fixture","email":"same-email@example.invalid"})).into_response()
}
