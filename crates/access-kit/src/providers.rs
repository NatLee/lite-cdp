use crate::{
    authorization::{self, session_binding},
    credentials,
    crypto::{equal, hash, now, random, time},
    AccessError, AccessKit,
};
use axum::{
    http::{HeaderMap, StatusCode},
    response::Response,
};
use oauth2::{
    basic::BasicClient, AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken,
    PkceCodeChallenge, PkceCodeVerifier, RedirectUrl, TokenResponse as _, TokenUrl,
};
use openidconnect::{
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
    AccessTokenHash, IssuerUrl, Nonce, Scope, TokenResponse as _,
};
use serde_json::{json, Value};
use sqlx::Row;
fn provider<'a>(
    kit: &'a AccessKit,
    name: &str,
) -> Result<&'a crate::config::ProviderCredentials, AccessError> {
    match name {
        "github" => kit.0.config.github.as_ref(),
        "google" => kit.0.config.google.as_ref(),
        _ => None,
    }
    .ok_or_else(|| AccessError::bad("provider_disabled", "Provider is not configured"))
}
fn endpoint(kit: &AccessKit, name: &str, part: &str) -> String {
    if let Some(base) = &kit.0.config.mock_base {
        return format!("{}/{name}/{part}", base.origin().ascii_serialization());
    }
    match (name, part) {
        ("github", "authorize") => "https://github.com/login/oauth/authorize",
        ("github", "token") => "https://github.com/login/oauth/access_token",
        ("github", "user") => "https://api.github.com/user",
        ("google", "issuer") => "https://accounts.google.com",
        _ => "",
    }
    .to_owned()
}
struct BoundedHttp {
    http: reqwest::Client,
    mock: Option<String>,
}
impl BoundedHttp {
    fn new(kit: &AccessKit) -> Self {
        Self {
            http: kit.0.http.clone(),
            mock: kit
                .0
                .config
                .mock_base
                .as_ref()
                .map(|b| b.origin().ascii_serialization()),
        }
    }
}
impl<'a> oauth2::AsyncHttpClient<'a> for BoundedHttp {
    type Error = AccessError;
    type Future = std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<oauth2::HttpResponse, AccessError>> + Send + 'a,
        >,
    >;
    fn call(&'a self, request: oauth2::HttpRequest) -> Self::Future {
        let http = self.http.clone();
        let mock = self.mock.clone();
        Box::pin(async move { provider_request(http, mock, request).await })
    }
}
async fn provider_request(
    http: reqwest::Client,
    mock: Option<String>,
    request: oauth2::HttpRequest,
) -> Result<oauth2::HttpResponse, AccessError> {
    let url =
        url::Url::parse(&request.uri().to_string()).map_err(|_| AccessError::unavailable())?;
    let allowed = if let Some(base) = mock {
        url.origin().ascii_serialization() == base
    } else {
        url.scheme() == "https"
            && url.port_or_known_default() == Some(443)
            && matches!(
                url.host_str(),
                Some(
                    "github.com"
                        | "api.github.com"
                        | "accounts.google.com"
                        | "oauth2.googleapis.com"
                        | "www.googleapis.com"
                )
            )
    };
    if !allowed || !url.username().is_empty() || url.password().is_some() {
        return Err(AccessError::unavailable());
    }
    let mut response = http
        .request(request.method().clone(), url)
        .headers(request.headers().clone())
        .body(request.into_body())
        .send()
        .await
        .map_err(|_| AccessError::unavailable())?;
    if response.content_length().is_some_and(|n| n > 262144) {
        return Err(AccessError::unavailable());
    }
    let status = response.status();
    let headers = response.headers().clone();
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AccessError::unavailable())?
    {
        if bytes.len() + chunk.len() > 262144 {
            return Err(AccessError::unavailable());
        }
        bytes.extend_from_slice(&chunk);
    }
    let mut result = axum::http::Response::builder().status(status);
    *result.headers_mut().ok_or_else(AccessError::unavailable)? = headers;
    result.body(bytes).map_err(|_| AccessError::unavailable())
}
async fn google_metadata(kit: &AccessKit) -> Result<CoreProviderMetadata, AccessError> {
    let issuer = IssuerUrl::new(endpoint(kit, "google", "issuer"))
        .map_err(|_| AccessError::unavailable())?;
    CoreProviderMetadata::discover_async(issuer, &BoundedHttp::new(kit))
        .await
        .map_err(|_| AccessError::unavailable())
}
pub(crate) async fn start(
    kit: &AccessKit,
    name: &str,
    h: &HeaderMap,
    link: bool,
    signup: bool,
    next: Option<&str>,
) -> Result<Response, AccessError> {
    let p = provider(kit, name)?;
    if signup {
        let mut c = kit.0.db.acquire().await?;
        crate::registration::require_registration(kit, &mut c).await?;
    }
    if next.is_some_and(|n| !authorization::valid_next(n) && n != kit.0.host.paths().connections) {
        return Err(AccessError::bad(
            "invalid_next",
            "Only a local authorization continuation is allowed",
        ));
    }
    let session = if link {
        Some(credentials::cookie(kit, h, true).await?)
    } else {
        None
    };
    let browser = authorization::browser(h).unwrap_or_else(random);
    let bh = hash(&browser);
    kit.public_rate(format!("provider:{bh}"), 10)?;
    let pending:i64=sqlx::query_scalar("SELECT COUNT(*) FROM access_provider_flows WHERE expires_at>? AND used_at IS NULL AND browser_hash=?").bind(now()).bind(&bh).fetch_one(&kit.0.db).await?;
    let all: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM access_provider_flows WHERE expires_at>? AND used_at IS NULL",
    )
    .bind(now())
    .fetch_one(&kit.0.db)
    .await?;
    if pending >= 8 || all >= 2048 {
        return Err(AccessError::bad(
            "flow_limit",
            "Too many pending sign-in requests",
        ));
    }
    let state = random();
    let nonce = random();
    let verifier = random();
    let challenge =
        PkceCodeChallenge::from_code_verifier_sha256(&PkceCodeVerifier::new(verifier.clone()));
    let callback = kit.0.config.url(&format!("/oauth/callback/{name}"));
    let authorization_url = if name == "github" {
        let client = BasicClient::new(ClientId::new(p.id.clone()))
            .set_client_secret(ClientSecret::new(p.secret.clone()))
            .set_auth_type(oauth2::AuthType::RequestBody)
            .set_auth_uri(
                AuthUrl::new(endpoint(kit, name, "authorize"))
                    .map_err(|_| AccessError::unavailable())?,
            )
            .set_token_uri(
                TokenUrl::new(endpoint(kit, name, "token"))
                    .map_err(|_| AccessError::unavailable())?,
            )
            .set_redirect_uri(RedirectUrl::new(callback).map_err(|_| AccessError::unavailable())?);
        client
            .authorize_url(|| CsrfToken::new(state.clone()))
            .set_pkce_challenge(challenge)
            .url()
            .0
    } else {
        let client = CoreClient::from_provider_metadata(
            google_metadata(kit).await?,
            ClientId::new(p.id.clone()),
            Some(ClientSecret::new(p.secret.clone())),
        )
        .set_auth_type(oauth2::AuthType::RequestBody)
        .set_redirect_uri(RedirectUrl::new(callback).map_err(|_| AccessError::unavailable())?);
        let state_copy = state.clone();
        let nonce_copy = nonce.clone();
        client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                move || CsrfToken::new(state_copy),
                move || Nonce::new(nonce_copy),
            )
            .add_scope(Scope::new("openid".into()))
            .set_pkce_challenge(challenge)
            .url()
            .0
    };
    sqlx::query("INSERT INTO access_provider_flows(state_hash,provider,purpose,user_id,session_binding,browser_hash,verifier,nonce,next_path,expires_at)VALUES(?,?,?,?,?,?,?,?,?,?)").bind(hash(&state)).bind(name).bind(if link{"link"}else if signup{"signup"}else{"login"}).bind(session.as_ref().map(|s|s.principal.user_id)).bind(session.as_ref().map(session_binding)).bind(bh).bind(verifier).bind(nonce).bind(next.unwrap_or(kit.0.host.paths().connections)).bind(now()+300).execute(&kit.0.db).await?;
    let mut response = if link {
        crate::http::no_store(json!({"authorization_url":authorization_url.as_str()}))
    } else {
        authorization::redirect(authorization_url.as_str())?
    };
    response
        .headers_mut()
        .insert("set-cookie", authorization::browser_cookie(kit, &browser)?);
    Ok(response)
}
async fn subject(
    kit: &AccessKit,
    name: &str,
    code: &str,
    verifier: String,
    nonce: String,
) -> Result<String, AccessError> {
    let p = provider(kit, name)?;
    let callback = kit.0.config.url(&format!("/oauth/callback/{name}"));
    if name == "github" {
        let client = BasicClient::new(ClientId::new(p.id.clone()))
            .set_client_secret(ClientSecret::new(p.secret.clone()))
            .set_auth_type(oauth2::AuthType::RequestBody)
            .set_auth_uri(
                AuthUrl::new(endpoint(kit, name, "authorize"))
                    .map_err(|_| AccessError::unavailable())?,
            )
            .set_token_uri(
                TokenUrl::new(endpoint(kit, name, "token"))
                    .map_err(|_| AccessError::unavailable())?,
            )
            .set_redirect_uri(RedirectUrl::new(callback).map_err(|_| AccessError::unavailable())?);
        let token = client
            .exchange_code(AuthorizationCode::new(code.to_owned()))
            .set_pkce_verifier(PkceCodeVerifier::new(verifier))
            .request_async(&BoundedHttp::new(kit))
            .await
            .map_err(|_| AccessError::unavailable())?;
        let request = axum::http::Request::builder()
            .uri(endpoint(kit, name, "user"))
            .header(
                "authorization",
                format!("Bearer {}", token.access_token().secret()),
            )
            .header("accept", "application/json")
            .body(Vec::new())
            .map_err(|_| AccessError::unavailable())?;
        let response = provider_request(
            kit.0.http.clone(),
            kit.0
                .config
                .mock_base
                .as_ref()
                .map(|b| b.origin().ascii_serialization()),
            request,
        )
        .await?;
        if response.status() != StatusCode::OK {
            return Err(AccessError::unavailable());
        }
        let user: Value =
            serde_json::from_slice(response.body()).map_err(|_| AccessError::unavailable())?;
        user["id"]
            .as_u64()
            .filter(|id| *id > 0)
            .map(|id| id.to_string())
            .ok_or_else(AccessError::unavailable)
    } else {
        let client = CoreClient::from_provider_metadata(
            google_metadata(kit).await?,
            ClientId::new(p.id.clone()),
            Some(ClientSecret::new(p.secret.clone())),
        )
        .set_auth_type(oauth2::AuthType::RequestBody)
        .set_redirect_uri(RedirectUrl::new(callback).map_err(|_| AccessError::unavailable())?);
        let token = client
            .exchange_code(AuthorizationCode::new(code.to_owned()))
            .map_err(|_| AccessError::unavailable())?
            .set_pkce_verifier(PkceCodeVerifier::new(verifier))
            .request_async(&BoundedHttp::new(kit))
            .await
            .map_err(|_| AccessError::unavailable())?;
        let id_token = token.id_token().ok_or_else(AccessError::unavailable)?;
        let verifier = client.id_token_verifier();
        let claims = id_token
            .claims(&verifier, &Nonce::new(nonce))
            .map_err(|_| {
                AccessError::bad("access_denied", "Provider identity could not be verified")
            })?;
        if let Some(expected) = claims.access_token_hash() {
            let actual = AccessTokenHash::from_token(
                token.access_token(),
                id_token
                    .signing_alg()
                    .map_err(|_| AccessError::unavailable())?,
                id_token
                    .signing_key(&verifier)
                    .map_err(|_| AccessError::unavailable())?,
            )
            .map_err(|_| AccessError::unavailable())?;
            if actual != *expected {
                return Err(AccessError::denied());
            }
        }
        let sub = claims.subject().as_str();
        if sub.is_empty() || sub.len() > 255 {
            return Err(AccessError::denied());
        }
        Ok(sub.to_owned())
    }
}
pub(crate) async fn callback(
    kit: &AccessKit,
    name: &str,
    h: &HeaderMap,
    params: &std::collections::HashMap<String, String>,
) -> Result<Response, AccessError> {
    let state = params
        .get("state")
        .filter(|v| v.len() <= 256)
        .ok_or_else(|| AccessError::bad("state_invalid", "Invalid sign-in state"))?;
    let row=sqlx::query("SELECT * FROM access_provider_flows WHERE state_hash=? AND provider=? AND expires_at>? AND used_at IS NULL").bind(hash(state)).bind(name).bind(now()).fetch_optional(&kit.0.db).await?.ok_or_else(||AccessError::bad("state_invalid","Expired or replayed sign-in state"))?;
    let link = row.get::<String, _>("purpose") == "link";
    let signup = row.get::<String, _>("purpose") == "signup";
    let fail_path = if link {
        kit.0.host.paths().connections
    } else {
        kit.0.host.paths().login
    };
    let result=async{
  let b=authorization::browser(h).ok_or_else(||AccessError::bad("state_invalid","Browser binding mismatch"))?;if !equal(&row.get::<String,_>("browser_hash"),&hash(&b)){return Err(AccessError::bad("state_invalid","Browser binding mismatch"));}
  if link{let s=credentials::cookie(kit,h,false).await.map_err(|_|AccessError::bad("state_invalid","Sign-in session changed"))?;if Some(s.principal.user_id)!=row.get::<Option<i64>,_>("user_id")||!equal(&row.get::<Option<String>,_>("session_binding").unwrap_or_default(),&session_binding(&s)){return Err(AccessError::bad("state_invalid","Sign-in session changed"));}}
  let changed=sqlx::query("UPDATE access_provider_flows SET used_at=?,verifier='',nonce='' WHERE state_hash=? AND used_at IS NULL AND expires_at>?").bind(now()).bind(hash(state)).bind(now()).execute(&kit.0.db).await?.rows_affected();if changed!=1{return Err(AccessError::bad("state_invalid","Sign-in state was already used"));}
  if params.contains_key("error"){return Err(AccessError::denied());}
  let code=params.get("code").filter(|v|!v.is_empty()&&v.len()<=2048).ok_or_else(||AccessError::bad("access_denied","Provider denied sign-in"))?;
  let subject=subject(kit,name,code,row.get("verifier"),row.get("nonce")).await?;
  if link{
   let user:i64=row.get("user_id");if kit.0.host.active_principal(user).await?.is_none(){return Err(AccessError::bad("account_unavailable","Account is not active"));}
   let exists=sqlx::query("SELECT user_id,subject FROM access_identities WHERE provider=? AND (subject=? OR user_id=?)").bind(name).bind(&subject).bind(user).fetch_optional(&kit.0.db).await?;
   if let Some(existing)=exists{if existing.get::<i64,_>("user_id")!=user||existing.get::<String,_>("subject")!=subject{return Err(AccessError::bad("identity_conflict","Identity is already bound"));}}else{
    sqlx::query("INSERT INTO access_identities(id,user_id,provider,subject,linked_at)VALUES(?,?,?,?,?)").bind(random()).bind(user).bind(name).bind(subject).bind(now()).execute(&kit.0.db).await.map_err(|_|AccessError::bad("identity_conflict","Identity is already bound"))?;
   }
   authorization::redirect(kit.0.host.paths().connections)
  }else if signup {
   let username=format!("{name}_{}", &random()[..16]);
   let display=format!("{name} {}", &random()[..6]);
   let password=crate::registration::hash_password(random()).await?;
   let (user,status)=crate::registration::create(kit,name,Some(&subject),username,display,password).await?;
   if status=="pending" { authorization::redirect(&format!("{}?registration=pending",kit.0.host.paths().login)) } else {
    let cookie=kit.0.host.create_session(user).await?;
    let mut response=authorization::redirect(&row.get::<String,_>("next_path"))?;
    response.headers_mut().insert("set-cookie",cookie);Ok(response)
   }
  }else{
   let user:Option<i64>=sqlx::query_scalar("SELECT user_id FROM access_identities WHERE provider=? AND subject=?").bind(name).bind(subject).fetch_optional(&kit.0.db).await?;let user=user.ok_or_else(||AccessError::bad("identity_not_linked","Sign in with the original account and link this provider first"))?;
   crate::registration::account_error(kit,user).await?;
   if kit.0.host.active_principal(user).await?.is_none(){return Err(AccessError::bad("account_unavailable","Account is not active"));}
   let cookie=kit.0.host.create_session(user).await?;sqlx::query("UPDATE access_identities SET last_login_at=? WHERE user_id=? AND provider=?").bind(now()).bind(user).bind(name).execute(&kit.0.db).await?;
   let mut response=authorization::redirect(&row.get::<String,_>("next_path"))?;response.headers_mut().insert("set-cookie",cookie);Ok(response)
  }
 }.await;
    match result {
        Ok(r) => Ok(r),
        Err(e) => failure_redirect(kit, fail_path, e.code()),
    }
}
pub(crate) fn failure_redirect(
    _kit: &AccessKit,
    path: &str,
    code: &str,
) -> Result<Response, AccessError> {
    let safe = match code {
        "identity_not_linked"
        | "identity_conflict"
        | "state_invalid"
        | "access_denied"
        | "account_unavailable"
        | "provider_disabled"
        | "account_pending"
        | "account_rejected"
        | "registration_closed"
        | "setup_required" => code,
        _ => "provider_unavailable",
    };
    authorization::redirect(&format!("{path}?access_error={safe}"))
}
pub(crate) async fn identities(kit: &AccessKit, user: i64) -> Result<Vec<Value>, AccessError> {
    let rows=sqlx::query("SELECT id,provider,subject,linked_at,last_login_at FROM access_identities WHERE user_id=? ORDER BY provider").bind(user).fetch_all(&kit.0.db).await?;
    Ok(rows.into_iter().map(|r|{let sub:String=r.get("subject");let suffix=sub.chars().rev().take(4).collect::<String>().chars().rev().collect::<String>();json!({"id":r.get::<String,_>("id"),"provider":r.get::<String,_>("provider"),"subject_mask":format!("…{suffix}"),"linked_at":time(r.get("linked_at")),"last_login_at":credentials::optional_time(r.get("last_login_at"))})}).collect())
}
pub(crate) async fn unlink(kit: &AccessKit, user: i64, id: &str) -> Result<(), AccessError> {
    let mut tx = kit.0.db.begin_with("BEGIN IMMEDIATE").await?;
    let exists: Option<String> =
        sqlx::query_scalar("SELECT id FROM access_identities WHERE id=? AND user_id=?")
            .bind(id)
            .bind(user)
            .fetch_optional(&mut *tx)
            .await?;
    if exists.is_none() {
        return Err(AccessError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "Identity not found",
        ));
    }
    let account = kit
        .0
        .host
        .registration_account(&mut tx, user)
        .await?
        .ok_or_else(AccessError::unauthorized)?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM access_identities WHERE user_id=?")
        .bind(user)
        .fetch_one(&mut *tx)
        .await?;
    if !account.password_login_enabled && count <= 1 {
        return Err(AccessError::bad(
            "last_login_method",
            "Add a password or another identity before unlinking the last login method",
        ));
    }
    sqlx::query("DELETE FROM access_identities WHERE id=? AND user_id=?")
        .bind(id)
        .bind(user)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
