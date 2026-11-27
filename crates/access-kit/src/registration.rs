//! Account registration policy and review. Hosts own accounts and session authorization.
use crate::{
    credentials,
    crypto::{now, random, time},
    http::{no_store, parse_json},
    AccessError, AccessKit, Principal, RegistrationUser, Role,
};
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use axum::{
    body::Bytes,
    extract::{ConnectInfo, Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
    Router,
};
use rand_core::OsRng;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{Row, SqliteConnection};
use std::{collections::HashMap, net::SocketAddr};

pub(crate) fn router() -> Router<AccessKit> {
    Router::new()
        .route(
            "/api/auth/registration-policy",
            get(policy_get).patch(policy_patch),
        )
        .route("/api/auth/register", post(register))
        .route("/api/auth/registrations", get(list))
        .route("/api/auth/registrations/{id}/review", post(review))
        .route("/api/auth/registration-events", get(events))
}
fn error(status: StatusCode, code: &'static str, message: &str) -> AccessError {
    AccessError::new(status, code, message)
}
pub(crate) async fn policy_value(kit: &AccessKit) -> Result<Value, AccessError> {
    let mut c = kit.0.db.acquire().await?;
    let setup = kit.0.host.registration_count(&mut c).await? == 0;
    let mode: String = sqlx::query_scalar("SELECT mode FROM access_registration_policy WHERE id=1")
        .fetch_one(&mut *c)
        .await?;
    let enabled = !setup && mode != "closed";
    let providers = [
        ("google", kit.0.config.google.is_some()),
        ("github", kit.0.config.github.is_some()),
    ]
    .into_iter()
    .filter(|(_, configured)| enabled && *configured)
    .map(|(name, _)| name)
    .collect::<Vec<_>>();
    Ok(
        json!({"mode":if setup{"closed"}else{&mode},"password_enabled":enabled,"providers":providers,"setup_required":setup}),
    )
}
async fn policy_get(State(kit): State<AccessKit>) -> Result<Response, AccessError> {
    Ok(no_store(policy_value(&kit).await?))
}
async fn admin(kit: &AccessKit, h: &HeaderMap, write: bool) -> Result<Principal, AccessError> {
    if write && !h.contains_key("origin") {
        return Err(AccessError::denied());
    }
    let s = credentials::cookie(kit, h, write).await?;
    if s.principal.role != Role::Admin {
        return Err(AccessError::denied());
    }
    Ok(s.principal)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyBody {
    mode: String,
}
async fn policy_patch(
    State(kit): State<AccessKit>,
    h: HeaderMap,
    b: Bytes,
) -> Result<Response, AccessError> {
    let actor = admin(&kit, &h, true).await?;
    let body: PolicyBody = parse_json(&h, &b)?;
    if !matches!(body.mode.as_str(), "closed" | "approval" | "open") {
        return Err(AccessError::bad(
            "invalid_mode",
            "Invalid registration mode",
        ));
    }
    let mut tx = kit.0.db.begin_with("BEGIN IMMEDIATE").await?;
    if !kit
        .0
        .host
        .registration_admin(&mut tx, &h, actor.user_id)
        .await?
    {
        return Err(AccessError::denied());
    }
    sqlx::query("UPDATE access_registration_policy SET mode=? WHERE id=1")
        .bind(&body.mode)
        .execute(&mut *tx)
        .await?;
    event(
        &mut tx,
        Some(&actor),
        "policy_changed",
        None,
        Some(&body.mode),
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(no_store(policy_value(&kit).await?))
}
pub(crate) async fn require_registration(
    kit: &AccessKit,
    c: &mut SqliteConnection,
) -> Result<String, AccessError> {
    let count = kit.0.host.registration_count(c).await?;
    if count == 0 {
        return Err(error(
            StatusCode::FORBIDDEN,
            "setup_required",
            "Create the first administrator before registering",
        ));
    }
    let mode: String = sqlx::query_scalar("SELECT mode FROM access_registration_policy WHERE id=1")
        .fetch_one(&mut *c)
        .await?;
    if mode == "closed" {
        return Err(error(
            StatusCode::FORBIDDEN,
            "registration_closed",
            "Registration is closed",
        ));
    }
    if count >= 500 {
        return Err(error(
            StatusCode::CONFLICT,
            "account_limit",
            "Account limit reached",
        ));
    }
    Ok(if mode == "approval" {
        "pending"
    } else {
        "active"
    }
    .to_owned())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterBody {
    username: String,
    password: String,
    display_name: Option<String>,
}
pub(crate) async fn hash_password(password: String) -> Result<String, AccessError> {
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes(), &SaltString::generate(&mut OsRng))
            .map(|v| v.to_string())
            .map_err(|_| AccessError::unavailable())
    })
    .await
    .map_err(|_| AccessError::unavailable())?
}
async fn register(
    State(kit): State<AccessKit>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    h: HeaderMap,
    b: Bytes,
) -> Result<Response, AccessError> {
    if !h.contains_key("origin") || !credentials::same_origin(&kit, &h) {
        return Err(AccessError::denied());
    }
    if kit.0.host.cookie_principal(&h).await?.is_some() {
        return Err(error(
            StatusCode::CONFLICT,
            "already_signed_in",
            "Sign out before registering another account",
        ));
    }
    kit.public_rate(
        format!(
            "registration-peer:{}",
            peer.map(|p| p.0 .0.ip().to_string())
                .unwrap_or_else(|| "unknown-peer".into())
        ),
        5,
    )?;
    let body: RegisterBody = parse_json(&h, &b)?;
    let username = body.username.trim().to_ascii_lowercase();
    if !(3..=64).contains(&username.len())
        || !username
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
    {
        return Err(AccessError::bad(
            "invalid_username",
            "Username must contain 3 to 64 ASCII letters, digits, _, . or -",
        ));
    }
    if !(12..=256).contains(&body.password.chars().count()) {
        return Err(AccessError::bad(
            "invalid_password",
            "Password must contain 12 to 256 characters",
        ));
    }
    let display = body
        .display_name
        .unwrap_or_else(|| username.clone())
        .trim()
        .to_owned();
    if display.is_empty() || display.chars().count() > 100 {
        return Err(AccessError::bad(
            "invalid_display_name",
            "Display name must contain 1 to 100 characters",
        ));
    }
    // Refuse closed requests before expensive hashing, then recheck under the write lock.
    {
        let mut c = kit.0.db.acquire().await?;
        require_registration(&kit, &mut c).await?;
    }
    let hash = hash_password(body.password).await?;
    let (id, status) = create(&kit, "password", None, username, display, hash).await?;
    let mut response = no_store(json!({"status":status,"user_id":id}));
    *response.status_mut() = StatusCode::CREATED;
    if status == "active" {
        response
            .headers_mut()
            .insert("set-cookie", kit.0.host.create_session(id).await?);
    }
    Ok(response)
}
pub(crate) async fn create(
    kit: &AccessKit,
    provider: &str,
    subject: Option<&str>,
    username: String,
    display: String,
    hash: String,
) -> Result<(i64, String), AccessError> {
    let mut tx = kit.0.db.begin_with("BEGIN IMMEDIATE").await?;
    let status = require_registration(kit, &mut tx).await?;
    if let Some(subject) = subject {
        let exists: Option<i64> = sqlx::query_scalar(
            "SELECT user_id FROM access_identities WHERE provider=? AND subject=?",
        )
        .bind(provider)
        .bind(subject)
        .fetch_optional(&mut *tx)
        .await?;
        if exists.is_some() {
            return Err(error(
                StatusCode::CONFLICT,
                "identity_conflict",
                "Identity is already bound",
            ));
        }
    }
    let user = RegistrationUser {
        username: username.clone(),
        display_name: display.clone(),
        password_hash: hash,
        account_status: status.clone(),
        password_login_enabled: provider == "password",
    };
    let id = kit.0.host.registration_create(&mut tx, user).await?;
    if let Some(subject) = subject {
        sqlx::query(
            "INSERT INTO access_identities(id,user_id,provider,subject,linked_at)VALUES(?,?,?,?,?)",
        )
        .bind(random())
        .bind(id)
        .bind(provider)
        .bind(subject)
        .bind(now())
        .execute(&mut *tx)
        .await?;
    }
    let application=sqlx::query("INSERT INTO access_registrations(user_id,username,display_name,provider,status,created_at)VALUES(?,?,?,?,?,?)").bind(id).bind(username).bind(display).bind(provider).bind(&status).bind(now()).execute(&mut *tx).await?.last_insert_rowid();
    event(
        &mut tx,
        None,
        "registered",
        Some(application),
        Some(if status == "pending" {
            "approval"
        } else {
            "open"
        }),
        None,
    )
    .await?;
    tx.commit().await?;
    Ok((id, status))
}
pub(crate) async fn account_error(kit: &AccessKit, id: i64) -> Result<(), AccessError> {
    let mut c = kit.0.db.acquire().await?;
    if let Some(account) = kit.0.host.registration_account(&mut c, id).await? {
        match account.status.as_str() {
            "pending" => {
                return Err(error(
                    StatusCode::FORBIDDEN,
                    "account_pending",
                    "Account is awaiting administrator approval",
                ))
            }
            "rejected" => {
                return Err(error(
                    StatusCode::FORBIDDEN,
                    "account_rejected",
                    "Account registration was rejected",
                ))
            }
            _ => {}
        }
    }
    Ok(())
}
async fn event(
    c: &mut SqliteConnection,
    actor: Option<&Principal>,
    action: &str,
    registration: Option<i64>,
    mode: Option<&str>,
    reason: Option<&str>,
) -> Result<(), AccessError> {
    sqlx::query("INSERT INTO access_registration_events(actor_id,actor_username,action,registration_id,mode,reason,created_at)VALUES(?,?,?,?,?,?,?)").bind(actor.map(|a|a.user_id)).bind(actor.map(|a|a.username.as_str())).bind(action).bind(registration).bind(mode).bind(reason).bind(now()).execute(c).await?;
    Ok(())
}
fn application(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<i64,_>("id"),"user_id":r.get::<i64,_>("user_id"),"username":r.get::<String,_>("username"),"display_name":r.get::<String,_>("display_name"),"provider":r.get::<String,_>("provider"),"status":r.get::<String,_>("status"),"created_at":time(r.get("created_at")),"reviewed_at":r.get::<Option<i64>,_>("reviewed_at").map(time),"reviewed_by":r.get::<Option<i64>,_>("reviewed_by"),"rejection_reason":r.get::<Option<String>,_>("rejection_reason")})
}
fn pagination(q: &HashMap<String, String>) -> Result<(i64, i64), AccessError> {
    let limit = q
        .get("limit")
        .map(|v| v.parse::<i64>())
        .transpose()
        .map_err(|_| AccessError::bad("invalid_limit", "Invalid limit"))?
        .unwrap_or(50);
    let before = q
        .get("before")
        .map(|v| v.parse::<i64>())
        .transpose()
        .map_err(|_| AccessError::bad("invalid_cursor", "Invalid cursor"))?
        .unwrap_or(i64::MAX);
    if !(1..=100).contains(&limit) || before <= 0 {
        return Err(AccessError::bad("invalid_pagination", "Invalid pagination"));
    }
    Ok((limit, before))
}
async fn list(
    State(kit): State<AccessKit>,
    h: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Response, AccessError> {
    admin(&kit, &h, false).await?;
    let (limit, before) = pagination(&q)?;
    let status = q.get("status").map(String::as_str).unwrap_or("pending");
    if !matches!(status, "pending" | "active" | "rejected") {
        return Err(AccessError::bad(
            "invalid_status",
            "Invalid registration status",
        ));
    }
    let rows = sqlx::query(
        "SELECT * FROM access_registrations WHERE status=? AND id<? ORDER BY id DESC LIMIT ?",
    )
    .bind(status)
    .bind(before)
    .bind(limit + 1)
    .fetch_all(&kit.0.db)
    .await?;
    let mut items = rows.into_iter().map(application).collect::<Vec<_>>();
    let more = items.len() > limit as usize;
    items.truncate(limit as usize);
    let next = if more {
        items.last().and_then(|v| v["id"].as_i64())
    } else {
        None
    };
    Ok(no_store(json!({"items":items,"next_cursor":next})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewBody {
    decision: String,
    reason: Option<String>,
}
async fn review(
    State(kit): State<AccessKit>,
    Path(id): Path<i64>,
    h: HeaderMap,
    b: Bytes,
) -> Result<Response, AccessError> {
    let actor = admin(&kit, &h, true).await?;
    let body: ReviewBody = parse_json(&h, &b)?;
    let reason = body
        .reason
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if !matches!(body.decision.as_str(), "approve" | "reject")
        || reason.is_some_and(|s| s.chars().count() > 500)
        || (body.decision == "reject" && reason.is_none())
    {
        return Err(AccessError::bad(
            "invalid_review",
            "Reject requires a reason of at most 500 characters",
        ));
    }
    let mut tx = kit.0.db.begin_with("BEGIN IMMEDIATE").await?;
    if !kit
        .0
        .host
        .registration_admin(&mut tx, &h, actor.user_id)
        .await?
    {
        return Err(AccessError::denied());
    }
    let row = sqlx::query("SELECT user_id,status FROM access_registrations WHERE id=?")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "not_found", "Registration not found"))?;
    if row.get::<String, _>("status") != "pending" {
        return Err(error(
            StatusCode::CONFLICT,
            "registration_already_reviewed",
            "Registration has already been reviewed",
        ));
    }
    let status = if body.decision == "approve" {
        "active"
    } else {
        "rejected"
    };
    if !kit
        .0
        .host
        .registration_set_status(&mut tx, row.get("user_id"), status)
        .await?
    {
        return Err(error(
            StatusCode::CONFLICT,
            "account_unavailable",
            "Registration account is unavailable",
        ));
    }
    sqlx::query("UPDATE access_registrations SET status=?,reviewed_at=?,reviewed_by=?,rejection_reason=? WHERE id=? AND status='pending'").bind(status).bind(now()).bind(actor.user_id).bind(if status=="rejected"{reason}else{None}).bind(id).execute(&mut *tx).await?;
    event(
        &mut tx,
        Some(&actor),
        if status == "active" {
            "approved"
        } else {
            "rejected"
        },
        Some(id),
        None,
        reason,
    )
    .await?;
    let row = sqlx::query("SELECT * FROM access_registrations WHERE id=?")
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(no_store(application(row)))
}
async fn events(
    State(kit): State<AccessKit>,
    h: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> Result<Response, AccessError> {
    admin(&kit, &h, false).await?;
    let (limit, before) = pagination(&q)?;
    let rows =
        sqlx::query("SELECT * FROM access_registration_events WHERE id<? ORDER BY id DESC LIMIT ?")
            .bind(before)
            .bind(limit + 1)
            .fetch_all(&kit.0.db)
            .await?;
    let mut items=rows.into_iter().map(|r|json!({"id":r.get::<i64,_>("id"),"actor_id":r.get::<Option<i64>,_>("actor_id"),"actor_username":r.get::<Option<String>,_>("actor_username"),"action":r.get::<String,_>("action"),"registration_id":r.get::<Option<i64>,_>("registration_id"),"mode":r.get::<Option<String>,_>("mode"),"reason":r.get::<Option<String>,_>("reason"),"created_at":time(r.get("created_at"))})).collect::<Vec<_>>();
    let more = items.len() > limit as usize;
    items.truncate(limit as usize);
    let next = if more {
        items.last().and_then(|v| v["id"].as_i64())
    } else {
        None
    };
    Ok(no_store(json!({"items":items,"next_cursor":next})))
}
