//! 設定都來自環境變數，全部都有預設值。

use std::{env, path::PathBuf};

use anyhow::{Context, Result};

pub const DEFAULT_BIND: &str = "0.0.0.0:8585";

#[derive(Clone, Debug)]
pub struct Config {
    pub bind: String,
    pub database_path: PathBuf,
    pub static_dir: PathBuf,
    /// 外部看到的網址（例如 `https://cdp.example.com`），沒有結尾的斜線；
    /// 追蹤程式碼裡的網址會用到。
    pub public_url: Option<String>,
    /// `public_url` 的 origin；用來判斷寫入請求是不是同源。
    pub public_origin: Option<String>,
    /// 登入 cookie 要不要加 `Secure`。沒設定時看 `CDP_PUBLIC_URL` 是不是 https。
    pub cookie_secure: bool,
    /// 前面有反向代理時設成 true，才會從 `X-Forwarded-For` 讀訪客的 IP。
    pub trust_proxy: bool,
    /// 同一個 IP 每分鐘最多送幾次追蹤資料；0 是不限制。
    pub rate_limit: u32,
    /// 事件留幾天；0 是不刪。
    pub retention_days: u32,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let data_dir = PathBuf::from(var("CDP_DATA_DIR").unwrap_or_else(|| "data".into()));
        let legacy = var("CDP_PUBLIC_URL");
        let access = var("ACCESS_PUBLIC_URL");
        if let (Some(left), Some(right)) = (&legacy, &access) {
            anyhow::ensure!(
                origin_of(left) == origin_of(right),
                "CDP_PUBLIC_URL and ACCESS_PUBLIC_URL must have the same origin"
            );
        }
        let public_url = legacy
            .or(access)
            .map(|u| u.trim().trim_end_matches('/').to_owned());
        let cookie_secure = match var("CDP_COOKIE_SECURE") {
            Some(v) => truthy(&v),
            None => public_url
                .as_deref()
                .is_some_and(|u| u.starts_with("https://")),
        };
        Ok(Self {
            bind: var("CDP_BIND").unwrap_or_else(|| DEFAULT_BIND.into()),
            database_path: var("CDP_DATABASE_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|| data_dir.join("lite-cdp.db")),
            static_dir: PathBuf::from(
                var("CDP_STATIC_DIR").unwrap_or_else(|| "src/web/dist".into()),
            ),
            public_origin: public_url.as_deref().and_then(origin_of),
            public_url,
            cookie_secure,
            trust_proxy: var("CDP_TRUST_PROXY").is_some_and(|v| truthy(&v)),
            rate_limit: parse_var("CDP_RATE_LIMIT", 600)?,
            retention_days: parse_var("CDP_RETENTION_DAYS", 0)?,
        })
    }
}

fn truthy(v: &str) -> bool {
    matches!(v.trim(), "1" | "true" | "yes")
}

/// `https://host:port/path` → `https://host:port`
pub fn origin_of(url: &str) -> Option<String> {
    let url = url.trim();
    let (scheme, rest) = url.split_once("://")?;
    if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") {
        return None;
    }
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .filter(|h| !h.is_empty() && !h.contains(char::is_whitespace) && !h.contains('@'))?;
    Some(format!(
        "{}://{}",
        scheme.to_ascii_lowercase(),
        host.to_ascii_lowercase()
    ))
}

/// 讀環境變數；沒設定或只有空白都當成沒設定。
pub fn var(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn parse_var<T: std::str::FromStr>(name: &str, default: T) -> Result<T>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    match var(name) {
        Some(v) => v
            .trim()
            .parse()
            .with_context(|| format!("{name} 的值不合法：{v}")),
        None => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_drops_the_path() {
        assert_eq!(
            origin_of("https://WWW.example.com:8443/contact?x=1"),
            Some("https://www.example.com:8443".into())
        );
        assert_eq!(origin_of("not a url"), None);
        assert_eq!(origin_of("javascript://alert(1)"), None);
        assert_eq!(origin_of("https://user@evil.example"), None);
    }
}
