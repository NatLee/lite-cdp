//! 登入和權限。
//!
//! - 第一次啟動時沒有任何帳號，由第一個打開網頁的人建立管理員（`POST /api/auth/setup`），沒有預設密碼。
//! - 密碼用 argon2 雜湊；session 是 HttpOnly、SameSite=Lax 的 cookie，資料庫只存 token 的 SHA-256。
//! - 寫入請求要是同源送來的（看 `Origin`）。

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{FromRequestParts, Request, State},
    http::{header, request::Parts, HeaderMap, Method},
    middleware::Next,
    response::Response,
};
use rand_core::{OsRng, RngCore};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::{
    config::{origin_of, Config},
    db,
    error::{AppError, AppResult},
    state::AppState,
};

pub const COOKIE_NAME: &str = "cdp_session";
const SESSION_DAYS: i64 = 30;
const MAX_FAILURES: u32 = 5;
const LOCK_WINDOW: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct CurrentUser {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub role: String,
}

impl CurrentUser {
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}

/// 同一個帳號連續登入失敗太多次時，暫時擋住。
#[derive(Default)]
pub struct LoginLimiter {
    failures: Mutex<HashMap<String, (u32, Instant)>>,
}

impl LoginLimiter {
    pub fn check(&self, username: &str) -> AppResult<()> {
        let mut map = self.failures.lock().unwrap();
        match map.get(username) {
            Some((_, since)) if since.elapsed() > LOCK_WINDOW => {
                map.remove(username);
                Ok(())
            }
            Some((count, _)) if *count >= MAX_FAILURES => Err(AppError::TooManyRequests(
                "登入失敗太多次，請 5 分鐘後再試".into(),
            )),
            _ => Ok(()),
        }
    }

    pub fn record_failure(&self, username: &str) {
        let mut map = self.failures.lock().unwrap();
        // 不存在的帳號也會被記下來；太多時整個清掉，避免被灌爆
        if map.len() > 10_000 {
            map.clear();
        }
        let entry = map
            .entry(username.to_owned())
            .or_insert((0, Instant::now()));
        entry.0 += 1;
    }

    pub fn clear(&self, username: &str) {
        self.failures.lock().unwrap().remove(username);
    }
}

pub async fn hash_password(password: String) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|e| anyhow!("密碼雜湊失敗：{e}"))
    })
    .await?
}

pub async fn verify_password(password: String, hash: String) -> bool {
    tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash)
            .map(|parsed| {
                Argon2::default()
                    .verify_password(password.as_bytes(), &parsed)
                    .is_ok()
            })
            .unwrap_or(false)
    })
    .await
    .unwrap_or(false)
}

pub fn validate_password(password: &str) -> AppResult<()> {
    let len = password.chars().count();
    if len < 8 {
        return Err(AppError::bad("密碼至少要 8 個字元"));
    }
    if len > 200 {
        return Err(AppError::bad("密碼太長"));
    }
    Ok(())
}

pub fn validate_username(name: &str) -> AppResult<()> {
    let ok = (3..=32).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
    if ok {
        Ok(())
    } else {
        Err(AppError::bad("帳號要 3 到 32 個字元，只能用英數字和 _ . -"))
    }
}

pub fn clean_display_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err(AppError::bad("顯示名稱要 1 到 40 個字元"));
    }
    Ok(name.to_owned())
}

pub fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    OsRng.fill_bytes(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub async fn create_user(
    db: &SqlitePool,
    username: &str,
    display_name: &str,
    password: &str,
    role: &str,
) -> AppResult<i64> {
    validate_username(username)?;
    validate_password(password)?;
    let display_name = clean_display_name(display_name)?;
    if !matches!(role, "admin" | "member") {
        return Err(AppError::bad("角色只能是 admin 或 member"));
    }
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(db)
        .await?;
    if exists.is_some() {
        return Err(AppError::conflict("這個帳號已經有人用了"));
    }
    let hash = hash_password(password.to_owned()).await?;
    let id = sqlx::query(
        "INSERT INTO users (username, display_name, password_hash, role, created_at)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(username)
    .bind(display_name)
    .bind(hash)
    .bind(role)
    .bind(db::now())
    .execute(db)
    .await?
    .last_insert_rowid();
    Ok(id)
}

pub async fn set_password(db: &SqlitePool, user_id: i64, password: &str) -> AppResult<()> {
    validate_password(password)?;
    let hash = hash_password(password.to_owned()).await?;
    sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
        .bind(hash)
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn create_session(db: &SqlitePool, user_id: i64) -> Result<String> {
    let token = random_hex(32);
    let now = chrono::Utc::now();
    sqlx::query("DELETE FROM sessions WHERE expires_at < ?")
        .bind(db::format_time(now))
        .execute(db)
        .await?;
    sqlx::query(
        "INSERT INTO sessions (token_hash, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(sha256_hex(&token))
    .bind(user_id)
    .bind(db::format_time(now + chrono::Duration::days(SESSION_DAYS)))
    .bind(db::format_time(now))
    .execute(db)
    .await?;
    Ok(token)
}

/// 登出：刪掉這個 cookie 對應的 session。
pub async fn delete_session(db: &SqlitePool, headers: &HeaderMap) -> Result<()> {
    if let Some(token) = read_cookie(headers, COOKIE_NAME) {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(sha256_hex(&token))
            .execute(db)
            .await?;
    }
    Ok(())
}

/// 刪掉某個人的所有 session（改密碼、停用時）；`keep` 是要留下的那一個。
pub async fn delete_sessions_of(db: &SqlitePool, user_id: i64, keep: Option<&str>) -> Result<()> {
    sqlx::query("DELETE FROM sessions WHERE user_id = ? AND token_hash != ?")
        .bind(user_id)
        .bind(keep.map(sha256_hex).unwrap_or_default())
        .execute(db)
        .await?;
    Ok(())
}

async fn user_for_token(db: &SqlitePool, token: &str) -> Result<Option<CurrentUser>> {
    let user = sqlx::query_as::<_, CurrentUser>(
        "SELECT u.id, u.username, u.display_name, u.role
         FROM sessions s JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = ? AND s.expires_at > ? AND u.disabled = 0",
    )
    .bind(sha256_hex(token))
    .bind(db::now())
    .fetch_optional(db)
    .await?;
    Ok(user)
}

/// 有登入就回使用者，沒有就回 `None`（不當成錯誤）。
pub async fn optional_user(db: &SqlitePool, headers: &HeaderMap) -> Result<Option<CurrentUser>> {
    match read_cookie(headers, COOKIE_NAME) {
        Some(token) => user_for_token(db, &token).await,
        None => Ok(None),
    }
}

pub fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.to_owned())
}

pub fn session_cookie(token: &str, secure: bool) -> String {
    format!(
        "{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{}",
        SESSION_DAYS * 24 * 3600,
        if secure { "; Secure" } else { "" }
    )
}

pub fn clear_cookie(secure: bool) -> String {
    format!(
        "{COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{}",
        if secure { "; Secure" } else { "" }
    )
}

/// Read current host account authority for a server-authenticated invocation.
pub async fn active_user(db: &SqlitePool, user_id: i64) -> Result<Option<CurrentUser>> {
    Ok(sqlx::query_as::<_, CurrentUser>(
        "SELECT id, username, display_name, role FROM users WHERE id=? AND disabled=0",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?)
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let unauthorized = || AppError::Unauthorized("請先登入".into());
        if let Some(principal) = parts.extensions.get::<access_kit::TrustedPrincipal>() {
            return active_user(&state.db, principal.user_id())
                .await?
                .ok_or_else(unauthorized);
        }
        let token = read_cookie(&parts.headers, COOKIE_NAME).ok_or_else(unauthorized)?;
        user_for_token(&state.db, &token)
            .await?
            .ok_or_else(unauthorized)
    }
}

/// 只有管理員能用的 API。
pub struct Admin(pub CurrentUser);

impl FromRequestParts<AppState> for Admin {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = CurrentUser::from_request_parts(parts, state).await?;
        if user.is_admin() {
            Ok(Admin(user))
        } else {
            Err(AppError::forbidden("只有管理員可以做這件事"))
        }
    }
}

/// 寫入請求是不是同源送來的。瀏覽器送跨站請求時會帶 `Origin`；命令列工具不帶，放行。
pub fn same_origin(headers: &HeaderMap, config: &Config) -> bool {
    let text = |name: header::HeaderName| headers.get(name).and_then(|v| v.to_str().ok());
    if text(header::HeaderName::from_static("sec-fetch-site")) == Some("cross-site") {
        return false;
    }
    let Some(origin) = text(header::ORIGIN) else {
        return true;
    };
    let Some(origin) = origin_of(origin) else {
        return false;
    };
    if config.public_origin.as_deref() == Some(origin.as_str()) {
        return true;
    }
    // 沒設定 CDP_PUBLIC_URL（或經由別的網址連進來）時，和 Host 比對
    let host = text(header::HOST).unwrap_or_default().to_ascii_lowercase();
    origin.split_once("://").map(|(_, h)| h) == Some(host.as_str())
}

pub async fn origin_guard(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let safe = matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    if !safe && !same_origin(req.headers(), &state.config) {
        return Err(AppError::forbidden("這個請求不是從本站送出的"));
    }
    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (k, v) in pairs {
            map.insert(*k, HeaderValue::from_static(v));
        }
        map
    }

    fn config(public_url: Option<&str>) -> Config {
        Config {
            bind: String::new(),
            database_path: "x.db".into(),
            static_dir: "x".into(),
            public_url: public_url.map(str::to_owned),
            public_origin: public_url.and_then(origin_of),
            cookie_secure: false,
            trust_proxy: false,
            rate_limit: 0,
            retention_days: 0,
        }
    }

    #[test]
    fn same_origin_check() {
        let c = config(None);
        assert!(same_origin(&headers(&[]), &c), "命令列工具沒有 Origin");
        assert!(same_origin(
            &headers(&[
                ("origin", "http://localhost:8585"),
                ("host", "localhost:8585")
            ]),
            &c
        ));
        assert!(!same_origin(
            &headers(&[
                ("origin", "https://evil.example"),
                ("host", "localhost:8585")
            ]),
            &c
        ));
        assert!(!same_origin(
            &headers(&[("sec-fetch-site", "cross-site"), ("host", "localhost:8585")]),
            &c
        ));
        let c = config(Some("https://cdp.example.com/"));
        assert!(same_origin(
            &headers(&[
                ("origin", "https://cdp.example.com"),
                ("host", "internal:8787")
            ]),
            &c
        ));
    }

    #[test]
    fn cookies_are_http_only_and_lax() {
        let c = session_cookie("abc", true);
        assert!(c.contains("HttpOnly") && c.contains("SameSite=Lax") && c.ends_with("; Secure"));
        assert!(!session_cookie("abc", false).contains("Secure"));
        assert!(clear_cookie(false).contains("Max-Age=0"));
        assert_eq!(
            read_cookie(
                &headers(&[("cookie", "a=1; cdp_session=tok; b=2")]),
                COOKIE_NAME
            ),
            Some("tok".into())
        );
    }

    #[test]
    fn usernames_and_passwords_are_validated() {
        assert!(validate_username("nat.lee-01").is_ok());
        assert!(validate_username("ab").is_err());
        assert!(validate_username("有中文").is_err());
        assert!(validate_password("1234567").is_err());
        assert!(validate_password("12345678").is_ok());
    }

    #[test]
    fn limiter_locks_after_repeated_failures() {
        let limiter = LoginLimiter::default();
        for _ in 0..MAX_FAILURES {
            assert!(limiter.check("amy").is_ok());
            limiter.record_failure("amy");
        }
        assert!(limiter.check("amy").is_err());
        assert!(limiter.check("bob").is_ok());
        limiter.clear("amy");
        assert!(limiter.check("amy").is_ok());
    }

    #[tokio::test]
    async fn hash_and_verify() {
        let hash = hash_password("correct horse".into()).await.unwrap();
        assert!(hash.starts_with("$argon2"));
        assert!(verify_password("correct horse".into(), hash.clone()).await);
        assert!(!verify_password("wrong".into(), hash).await);
    }
}
