//! Deterministic, site-scoped identity and event ingestion.
//!
//! Random browser IDs remain separate until an explicit stable `user_id` joins them.
//! Legacy email-only identity remains supported. Distinct stable subjects never merge by email.
//! Identified account switches redirect future events without moving the prior person's history.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::labels;

/// 一次最多收幾個事件。
pub const MAX_EVENTS: usize = 50;
const MAX_URL: usize = 2000;
const MAX_PROPS_BYTES: usize = 2048;
const MAX_TRAITS: usize = 20;

#[derive(Deserialize)]
pub struct Payload {
    pub key: String,
    pub anon_id: String,
    pub session_id: Option<String>,
    #[serde(default)]
    pub events: Vec<RawEvent>,
    pub identify: Option<RawIdentify>,
    pub consent: Option<bool>,
    pub test_token: Option<String>,
}

#[derive(Deserialize)]
pub struct RawEvent {
    pub event_id: Option<String>,
    pub happened_at: Option<String>,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub referrer: Option<String>,
    pub props: Option<Value>,
}

#[derive(Deserialize)]
pub struct RawIdentify {
    pub user_id: Option<String>,
    pub proof: Option<IdentityProof>,
    pub email: Option<String>,
    pub name: Option<String>,
    pub account: Option<String>,
    pub traits: Option<Map<String, Value>>,
}

#[derive(Deserialize)]
pub struct IdentityProof {
    pub expires_at: i64,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize)]
pub struct Event {
    pub event_id: String,
    pub happened_at: Option<String>,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub name: String,
    pub url: String,
    pub path: String,
    pub title: String,
    pub referrer: String,
    pub referrer_host: String,
    /// JSON 物件
    pub props: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Identify {
    pub user_id: String,
    pub verified: bool,
    /// 小寫；空字串是沒帶 email
    pub email: String,
    pub name: Option<String>,
    pub account: Option<String>,
    pub traits: Map<String, Value>,
}

#[derive(Debug)]
pub struct Batch {
    pub anon_id: String,
    pub session_id: String,
    pub events: Vec<Event>,
    pub identify: Option<Identify>,
}

/// 最多留 `max` 個字元。
pub fn truncate(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

fn clean_text(text: Option<&str>, max: usize) -> String {
    let text: String = text
        .unwrap_or_default()
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    truncate(text.trim(), max)
}

fn valid_id(id: &str, min: usize) -> bool {
    (min..=64).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
}

pub fn looks_like_email(email: &str) -> bool {
    let Some((local, domain)) = email.split_once('@') else {
        return false;
    };
    !local.is_empty()
        && email.len() <= 254
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !email.contains(char::is_whitespace)
        && !domain.contains('@')
}

/// `https://Shop.example.com/Cart/?a=1#top` → (`shop.example.com`, `/Cart`)
///
/// 路徑去掉參數、錨點和結尾的斜線，同一頁才不會被分成好幾筆。
pub fn split_url(url: &str) -> Option<(String, String)> {
    let (scheme, rest) = url.trim().split_once("://")?;
    if !matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https") {
        return None;
    }
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let host = rest[..end].to_ascii_lowercase();
    if host.is_empty() || host.contains('@') || host.contains(char::is_whitespace) {
        return None;
    }
    let path = rest[end..].split(['?', '#']).next().unwrap_or_default();
    let path = path.trim_end_matches('/');
    let path = if path.is_empty() { "/" } else { path };
    Some((host, path.to_owned()))
}

fn bare_host(host: &str) -> &str {
    let host = host.split(':').next().unwrap_or(host);
    host.strip_prefix("www.").unwrap_or(host)
}

/// 從別的網站連進來時，回那個網站的網域；站內換頁或沒有來源時回空字串。
pub fn referrer_host(referrer: &str, page_host: &str) -> String {
    match split_url(referrer) {
        Some((host, _)) if bare_host(&host) != bare_host(page_host) => bare_host(&host).to_owned(),
        _ => String::new(),
    }
}

/// 事件屬性：只收物件，太大的整個不要。
fn clean_props(props: Option<Value>) -> String {
    match props {
        Some(Value::Object(map)) => {
            let text = Value::Object(map).to_string();
            if text.len() <= MAX_PROPS_BYTES {
                text
            } else {
                "{}".to_owned()
            }
        }
        _ => "{}".to_owned(),
    }
}

/// 特徵只收文字、數字和布林，巢狀的不要。
fn clean_traits(traits: Option<Map<String, Value>>) -> Map<String, Value> {
    let mut out = Map::new();
    for (key, value) in traits.unwrap_or_default() {
        let key = clean_text(Some(&key), 40);
        if key.is_empty() || out.len() >= MAX_TRAITS {
            continue;
        }
        let value = match value {
            Value::String(s) => Value::String(clean_text(Some(&s), 200)),
            Value::Number(_) | Value::Bool(_) => value,
            _ => continue,
        };
        out.insert(key, value);
    }
    out
}

impl RawEvent {
    fn clean(self) -> Result<Event, String> {
        let kind = match self.kind.as_str() {
            "pageview" => "pageview",
            "click" => "click",
            "scroll" => "scroll",
            "custom" => "custom",
            other => return Err(format!("不認得的事件種類：{}", truncate(other, 40))),
        };
        let name = clean_text(self.name.as_deref(), 120);
        if kind != "pageview" && name.is_empty() {
            return Err(format!("{kind} 事件要有 name"));
        }
        let url = self.url.unwrap_or_default();
        let url = url.trim();
        if url.len() > MAX_URL {
            return Err("url 太長了".into());
        }
        let (host, path) = match split_url(url) {
            Some(parts) => parts,
            // 從伺服器送的自訂事件可以沒有網址
            None if kind == "custom" && url.is_empty() => Default::default(),
            None => return Err("url 要是 http:// 或 https:// 開頭的網址".into()),
        };
        let referrer = truncate(self.referrer.as_deref().unwrap_or_default().trim(), MAX_URL);
        let event_id = self.event_id.unwrap_or_default();
        if !event_id.is_empty() && !valid_id(&event_id, 8) {
            return Err("event_id 要 8 到 64 個英數字、- 或 _".into());
        }
        let happened_at = self
            .happened_at
            .map(|raw| -> Result<String, String> {
                let at = chrono::DateTime::parse_from_rfc3339(&raw)
                    .map_err(|_| "happened_at 必須是 RFC3339 時間".to_owned())?
                    .with_timezone(&chrono::Utc);
                let age = chrono::Utc::now().signed_duration_since(at);
                if age > chrono::Duration::days(7) || age < chrono::Duration::minutes(-5) {
                    return Err("happened_at 超過 7 天或在未來 5 分鐘以外".into());
                }
                Ok(crate::db::format_time(at))
            })
            .transpose()?;
        Ok(Event {
            event_id,
            happened_at,
            kind,
            name,
            url: url.to_owned(),
            path,
            title: clean_text(self.title.as_deref(), 300),
            referrer_host: referrer_host(&referrer, &host),
            referrer,
            props: clean_props(self.props),
        })
    }
}

impl RawIdentify {
    fn clean(self) -> Result<Identify, String> {
        let email = self.email.unwrap_or_default().trim().to_lowercase();
        if !email.is_empty() && !looks_like_email(&email) {
            return Err("email 的格式不對".into());
        }
        let text = |v: Option<String>, max| {
            v.map(|v| clean_text(Some(&v), max))
                .filter(|v| !v.is_empty())
        };
        let user_id = self.user_id.unwrap_or_default().trim().to_owned();
        if user_id.len() > 128 || user_id.contains(char::is_control) {
            return Err("user_id 最多 128 bytes，不能包含控制字元".into());
        }
        Ok(Identify {
            user_id,
            verified: false,
            email,
            name: text(self.name, 80),
            account: text(self.account, 80),
            traits: clean_traits(self.traits),
        })
    }
}

impl Payload {
    pub fn clean(self) -> Result<Batch, String> {
        if !valid_id(&self.anon_id, 8) {
            return Err("anon_id 要 8 到 64 個字元，只能用英數字、- 和 _".into());
        }
        let session_id = self.session_id.unwrap_or_default();
        if !session_id.is_empty() && !valid_id(&session_id, 1) {
            return Err("session_id 最多 64 個字元，只能用英數字、- 和 _".into());
        }
        if self.events.len() > MAX_EVENTS {
            return Err(format!("一次最多送 {MAX_EVENTS} 個事件"));
        }
        let events = self
            .events
            .into_iter()
            .map(RawEvent::clean)
            .collect::<Result<Vec<_>, _>>()?;
        let identify = self.identify.map(RawIdentify::clean).transpose()?;
        if events.is_empty() && identify.is_none() {
            return Err("沒有事件，也沒有 identify".into());
        }
        Ok(Batch {
            anon_id: self.anon_id,
            session_id,
            events,
            identify,
        })
    }
}

/// 爬蟲和監控工具的 User-Agent；它們送來的不存。
pub fn is_bot(user_agent: &str) -> bool {
    let ua = user_agent.to_ascii_lowercase();
    [
        "bot",
        "spider",
        "crawl",
        "slurp",
        "headless",
        "lighthouse",
        "pingdom",
        "uptime",
    ]
    .iter()
    .any(|word| ua.contains(word))
}

/// 從 User-Agent 看是什麼瀏覽器、什麼系統；只求大致對。
pub fn describe_user_agent(ua: &str) -> (&'static str, &'static str) {
    let has = |word: &str| ua.contains(word);
    let os = if has("iPhone") || has("iPad") || has("iPod") {
        "iOS"
    } else if has("Android") {
        "Android"
    } else if has("Windows") {
        "Windows"
    } else if has("Mac OS X") || has("Macintosh") {
        "macOS"
    } else if has("CrOS") {
        "ChromeOS"
    } else if has("Linux") {
        "Linux"
    } else {
        ""
    };
    // 順序有差：Edge 和 Opera 的 UA 裡也有 Chrome，Chrome 的也有 Safari
    let browser = if has("Edg/") || has("EdgA/") || has("EdgiOS/") {
        "Edge"
    } else if has("OPR/") || has("Opera") {
        "Opera"
    } else if has("Firefox/") || has("FxiOS/") {
        "Firefox"
    } else if has("Chrome/") || has("CriOS/") {
        "Chrome"
    } else if has("Safari/") {
        "Safari"
    } else {
        ""
    };
    (browser, os)
}

/// 存完之後的結果。
#[derive(Debug, PartialEq, Eq)]
pub struct Stored {
    pub person_id: i64,
    pub events: usize,
}

struct Person {
    id: i64,
    email: String,
    user_id: String,
}

async fn new_person(
    tx: &mut Transaction<'_, Sqlite>,
    site_id: i64,
    now: &str,
) -> sqlx::Result<i64> {
    Ok(
        sqlx::query("INSERT INTO people (site_id, first_seen, last_seen) VALUES (?, ?, ?)")
            .bind(site_id)
            .bind(now)
            .bind(now)
            .execute(&mut **tx)
            .await?
            .last_insert_rowid(),
    )
}

/// 把匿名的人 `from` 併進 `into`，然後刪掉。
async fn merge_people(tx: &mut Transaction<'_, Sqlite>, from: i64, into: i64) -> sqlx::Result<()> {
    for table in ["events", "devices"] {
        sqlx::query(&format!(
            "UPDATE {table} SET person_id = ? WHERE person_id = ?"
        ))
        .bind(into)
        .bind(from)
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query(
        "INSERT INTO person_labels (person_id, label_id, hits, last_at)
         SELECT ?, label_id, hits, last_at FROM person_labels WHERE person_id = ?
         ON CONFLICT (person_id, label_id) DO UPDATE SET
            hits = hits + excluded.hits, last_at = max(last_at, excluded.last_at)",
    )
    .bind(into)
    .bind(from)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE people SET
            pageviews = pageviews + (SELECT pageviews FROM people WHERE id = ?2),
            first_seen = min(first_seen, (SELECT first_seen FROM people WHERE id = ?2)),
            last_seen = max(last_seen, (SELECT last_seen FROM people WHERE id = ?2))
         WHERE id = ?1",
    )
    .bind(into)
    .bind(from)
    .execute(&mut **tx)
    .await?;
    sqlx::query("DELETE FROM people WHERE id = ?")
        .bind(from)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// 套用 identify，回傳之後的事件要算在誰身上，以及這個瀏覽器是不是剛被認出來。
async fn apply_identify(
    tx: &mut Transaction<'_, Sqlite>,
    site_id: i64,
    device_id: i64,
    current: Person,
    identify: &Identify,
    now: &str,
) -> sqlx::Result<(i64, bool)> {
    let mut person_id = current.id;
    let mut newly = false;
    if !identify.user_id.is_empty() {
        let known: Option<i64> =
            sqlx::query_scalar("SELECT id FROM people WHERE site_id=? AND user_id=?")
                .bind(site_id)
                .bind(&identify.user_id)
                .fetch_optional(&mut **tx)
                .await?;
        let may_upgrade = current.user_id.is_empty()
            && (current.email.is_empty()
                || (!identify.email.is_empty() && current.email == identify.email));
        person_id = match known {
            Some(id) => {
                if id != current.id && may_upgrade {
                    merge_people(tx, current.id, id).await?;
                }
                id
            }
            None if may_upgrade => current.id,
            None => new_person(tx, site_id, now).await?,
        };
        newly = current.user_id != identify.user_id;
        sqlx::query("UPDATE people SET user_id=?, email=CASE WHEN ? != '' THEN ? ELSE email END, identified_at=coalesce(identified_at,?), identity_verified=max(identity_verified,?) WHERE id=?")
            .bind(&identify.user_id).bind(&identify.email).bind(&identify.email).bind(now).bind(identify.verified).bind(person_id).execute(&mut **tx).await?;
    } else if !identify.email.is_empty() && identify.email != current.email {
        newly = true;
        let known: Option<i64> =
            sqlx::query_scalar("SELECT id FROM people WHERE site_id=? AND email=? AND user_id=''")
                .bind(site_id)
                .bind(&identify.email)
                .fetch_optional(&mut **tx)
                .await?;
        let anonymous = current.email.is_empty() && current.user_id.is_empty();
        person_id = match (known, anonymous) {
            (Some(id), true) => {
                merge_people(tx, current.id, id).await?;
                id
            }
            (Some(id), false) => id,
            (None, true) => current.id,
            (None, false) => new_person(tx, site_id, now).await?,
        };
        sqlx::query(
            "UPDATE people SET email=?, identified_at=coalesce(identified_at,?) WHERE id=?",
        )
        .bind(&identify.email)
        .bind(now)
        .bind(person_id)
        .execute(&mut **tx)
        .await?;
    }
    if person_id != current.id {
        sqlx::query("UPDATE devices SET person_id=? WHERE id=?")
            .bind(person_id)
            .bind(device_id)
            .execute(&mut **tx)
            .await?;
    }

    if identify.name.is_some() || identify.account.is_some() || !identify.traits.is_empty() {
        let stored: String = sqlx::query_scalar("SELECT traits FROM people WHERE id = ?")
            .bind(person_id)
            .fetch_one(&mut **tx)
            .await?;
        let mut traits: Map<String, Value> = serde_json::from_str(&stored).unwrap_or_default();
        for (key, value) in &identify.traits {
            traits.insert(key.clone(), value.clone());
        }
        // 太多的話，舊的先丟
        while traits.len() > MAX_TRAITS {
            let Some(first) = traits.keys().next().cloned() else {
                break;
            };
            traits.shift_remove(&first);
        }
        sqlx::query(
            "UPDATE people SET name = coalesce(?, name), account = coalesce(?, account), traits = ?
             WHERE id = ?",
        )
        .bind(&identify.name)
        .bind(&identify.account)
        .bind(Value::Object(traits).to_string())
        .bind(person_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok((person_id, newly))
}

struct NewEvent<'a> {
    site_id: i64,
    person_id: i64,
    device_id: i64,
    session_id: &'a str,
    now: &'a str,
}

async fn insert_event(
    tx: &mut Transaction<'_, Sqlite>,
    at: &NewEvent<'_>,
    event: &Event,
) -> sqlx::Result<bool> {
    let inserted = sqlx::query(
        "INSERT OR IGNORE INTO events (site_id, person_id, device_id, session_id, type, name, url, path,
            title, referrer, referrer_host, props, created_at, event_id, received_at, happened_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(at.site_id)
    .bind(at.person_id)
    .bind(at.device_id)
    .bind(at.session_id)
    .bind(event.kind)
    .bind(&event.name)
    .bind(&event.url)
    .bind(&event.path)
    .bind(&event.title)
    .bind(&event.referrer)
    .bind(&event.referrer_host)
    .bind(&event.props)
    .bind(event.happened_at.as_deref().unwrap_or(at.now))
    .bind(&event.event_id)
    .bind(at.now)
    .bind(&event.happened_at)
    .execute(&mut **tx)
    .await?;
    Ok(inserted.rows_affected() > 0)
}

/// 把一批資料存進去。`now` 是事件的時間。
pub async fn store(
    db: &SqlitePool,
    site_id: i64,
    batch: &Batch,
    user_agent: &str,
    now: &str,
) -> sqlx::Result<Stored> {
    let mut tx = db.begin().await?;
    let stored = store_in(&mut tx, site_id, batch, user_agent, now).await?;
    tx.commit().await?;
    Ok(stored)
}

/// 和 [`store`] 一樣，但用呼叫的人開好的交易（範例資料一次塞很多筆時用）。
pub async fn store_in(
    tx: &mut Transaction<'_, Sqlite>,
    site_id: i64,
    batch: &Batch,
    user_agent: &str,
    now: &str,
) -> sqlx::Result<Stored> {
    let device: Option<(i64, i64, String, String)> = sqlx::query_as(
        "SELECT d.id, p.id, p.email, p.user_id FROM devices d JOIN people p ON p.id = d.person_id
         WHERE d.site_id = ? AND d.anon_id = ?",
    )
    .bind(site_id)
    .bind(&batch.anon_id)
    .fetch_optional(&mut **tx)
    .await?;
    let (device_id, person) = match device {
        Some((device_id, id, email, user_id)) => {
            sqlx::query("UPDATE devices SET last_seen = ?, user_agent = ? WHERE id = ?")
                .bind(now)
                .bind(user_agent)
                .bind(device_id)
                .execute(&mut **tx)
                .await?;
            (device_id, Person { id, email, user_id })
        }
        None => {
            let id = new_person(tx, site_id, now).await?;
            let device_id = sqlx::query(
                "INSERT INTO devices (site_id, anon_id, person_id, user_agent, first_seen, last_seen)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(site_id)
            .bind(&batch.anon_id)
            .bind(id)
            .bind(user_agent)
            .bind(now)
            .bind(now)
            .execute(&mut **tx)
            .await?
            .last_insert_rowid();
            let email = String::new();
            let user_id = String::new();
            (device_id, Person { id, email, user_id })
        }
    };

    let mut person_id = person.id;
    let mut at = NewEvent {
        site_id,
        person_id,
        device_id,
        session_id: &batch.session_id,
        now,
    };
    if let Some(identify) = &batch.identify {
        let (id, newly) = apply_identify(tx, site_id, device_id, person, identify, now).await?;
        person_id = id;
        at.person_id = id;
        // 網站通常每一頁都會呼叫 identify；只有第一次認出來時記一筆
        if newly {
            let page = batch.events.first();
            let event = Event {
                event_id: String::new(),
                happened_at: None,
                kind: "identify",
                name: if identify.user_id.is_empty() {
                    identify.email.clone()
                } else {
                    identify.user_id.clone()
                },
                url: page.map(|e| e.url.clone()).unwrap_or_default(),
                path: page.map(|e| e.path.clone()).unwrap_or_default(),
                title: page.map(|e| e.title.clone()).unwrap_or_default(),
                referrer: String::new(),
                referrer_host: String::new(),
                props: "{}".into(),
            };
            insert_event(tx, &at, &event).await?;
        }
    }

    let rules = labels::site_rules(tx, site_id).await?;
    let mut hits: Vec<(i64, i64)> = Vec::new();
    let mut pageviews = 0;
    let mut accepted = 0;
    for event in &batch.events {
        if !insert_event(tx, &at, event).await? {
            continue;
        }
        accepted += 1;
        if event.kind == "pageview" {
            pageviews += 1;
        }
        for label_id in labels::matching(&rules, event) {
            match hits.iter_mut().find(|(id, _)| *id == label_id) {
                Some((_, count)) => *count += 1,
                None => hits.push((label_id, 1)),
            }
        }
    }
    for (label_id, count) in hits {
        sqlx::query(
            "INSERT INTO person_labels (person_id, label_id, hits, last_at) VALUES (?, ?, ?, ?)
             ON CONFLICT (person_id, label_id) DO UPDATE SET
                hits = hits + excluded.hits, last_at = excluded.last_at",
        )
        .bind(person_id)
        .bind(label_id)
        .bind(count)
        .bind(now)
        .execute(&mut **tx)
        .await?;
    }

    sqlx::query(
        "UPDATE people SET last_seen = max(last_seen, ?), pageviews = pageviews + ? WHERE id = ?",
    )
    .bind(now)
    .bind(pageviews)
    .bind(person_id)
    .execute(&mut **tx)
    .await?;

    Ok(Stored {
        person_id,
        events: accepted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn payload(value: Value) -> Result<Batch, String> {
        serde_json::from_value::<Payload>(value)
            .map_err(|e| e.to_string())?
            .clean()
    }

    #[test]
    fn urls_are_split_into_host_and_path() {
        assert_eq!(
            split_url("https://Shop.example.com/Cart/?a=1#top"),
            Some(("shop.example.com".into(), "/Cart".into()))
        );
        assert_eq!(
            split_url("http://localhost:3000"),
            Some(("localhost:3000".into(), "/".into()))
        );
        assert_eq!(
            split_url("https://example.com?utm_source=x"),
            Some(("example.com".into(), "/".into()))
        );
        assert_eq!(split_url("javascript:alert(1)"), None);
        assert_eq!(split_url("/pricing"), None);
        assert_eq!(split_url("https://user@example.com/"), None);
    }

    #[test]
    fn referrer_host_ignores_the_site_itself() {
        let host = "www.example.com";
        assert_eq!(
            referrer_host("https://www.google.com/search?q=x", host),
            "google.com"
        );
        assert_eq!(referrer_host("https://example.com/blog", host), "");
        assert_eq!(referrer_host("https://www.example.com/", host), "");
        assert_eq!(referrer_host("", host), "");
        assert_eq!(referrer_host("android-app://com.example", host), "");
    }

    #[test]
    fn events_are_validated() {
        let batch = payload(json!({
            "key": "k",
            "anon_id": "a1b2c3d4e5",
            "session_id": "s1",
            "events": [
                { "type": "pageview", "url": "https://www.example.com/pricing/?plan=team",
                  "title": "  價格\n方案 ", "referrer": "https://t.co/abc" },
                { "type": "custom", "name": "signup", "props": { "plan": "team" } },
            ],
        }))
        .unwrap();
        assert_eq!(batch.events[0].path, "/pricing");
        assert_eq!(batch.events[0].title, "價格 方案");
        assert_eq!(batch.events[0].referrer_host, "t.co");
        assert_eq!(batch.events[1].props, r#"{"plan":"team"}"#);
        assert_eq!(batch.events[1].path, "");

        let bad = [
            json!({ "key": "k", "anon_id": "short", "events": [{ "type": "pageview", "url": "https://example.com" }] }),
            json!({ "key": "k", "anon_id": "has space 123", "events": [{ "type": "pageview", "url": "https://example.com" }] }),
            json!({ "key": "k", "anon_id": "a1b2c3d4e5", "events": [] }),
            json!({ "key": "k", "anon_id": "a1b2c3d4e5", "events": [{ "type": "pageview", "url": "/pricing" }] }),
            json!({ "key": "k", "anon_id": "a1b2c3d4e5", "events": [{ "type": "identify", "name": "x" }] }),
            json!({ "key": "k", "anon_id": "a1b2c3d4e5", "events": [{ "type": "custom" }] }),
            json!({ "key": "k", "anon_id": "a1b2c3d4e5", "identify": { "email": "not-an-email" } }),
        ];
        for body in bad {
            assert!(payload(body.clone()).is_err(), "{body}");
        }
    }

    #[test]
    fn identify_is_normalised() {
        let batch = payload(json!({
            "key": "k",
            "anon_id": "a1b2c3d4e5",
            "identify": {
                "email": " Amy@Example.COM ",
                "name": "  ",
                "account": "M-001",
                "traits": { "plan": "team", "seats": 5, "vip": true, "nested": { "a": 1 }, "list": [1] },
            },
        }))
        .unwrap();
        let identify = batch.identify.unwrap();
        assert_eq!(identify.email, "amy@example.com");
        assert_eq!(identify.name, None);
        assert_eq!(identify.account.as_deref(), Some("M-001"));
        assert_eq!(
            Value::Object(identify.traits),
            json!({ "plan": "team", "seats": 5, "vip": true })
        );
    }

    #[test]
    fn oversized_props_are_dropped() {
        assert_eq!(clean_props(Some(json!({ "a": "x".repeat(3000) }))), "{}");
        assert_eq!(clean_props(Some(json!([1, 2]))), "{}");
        assert_eq!(clean_props(None), "{}");
    }

    #[test]
    fn user_agents() {
        let chrome = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
        let safari_ios = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";
        let edge = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0";
        assert_eq!(describe_user_agent(chrome), ("Chrome", "macOS"));
        assert_eq!(describe_user_agent(safari_ios), ("Safari", "iOS"));
        assert_eq!(describe_user_agent(edge), ("Edge", "Windows"));
        assert_eq!(describe_user_agent("curl/8.0"), ("", ""));
        assert!(is_bot(
            "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)"
        ));
        assert!(!is_bot(chrome));
    }
}
