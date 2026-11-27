//! 標籤：用規則描述「做過什麼事的人」，例如看過 `/pricing` 底下的頁面、觸發過 `signup` 事件。
//!
//! 每個人符合每個標籤幾次存在 `person_labels`：收事件時累加（[`matching`]），
//! 規則改了就整個標籤重算（[`recompute`]）。兩邊的比對方式要一樣：
//! 都不分英文大小寫（SQLite 的 `lower()` 只處理 ASCII，所以這裡也只轉 ASCII）。

use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Sqlite, SqliteConnection};

use crate::ingest::Event;

pub const KINDS: [&str; 4] = ["path_prefix", "url_contains", "title_contains", "event"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct Rule {
    pub kind: String,
    pub value: String,
}

/// 檢查並整理一條規則；不合法時回給使用者看的訊息。
pub fn clean_rule(rule: &Rule) -> Result<Rule, String> {
    if !KINDS.contains(&rule.kind.as_str()) {
        return Err(format!("沒有「{}」這種規則", rule.kind));
    }
    let mut value = rule.value.trim().to_owned();
    if value.is_empty() || value.chars().count() > 200 {
        return Err("規則的內容要 1 到 200 個字元".into());
    }
    if rule.kind == "path_prefix" {
        if !value.starts_with('/') {
            return Err(format!("路徑要以 / 開頭：{value}"));
        }
        if value.len() > 1 {
            value = value.trim_end_matches('/').to_owned();
        }
        if value.is_empty() {
            value = "/".into();
        }
    }
    Ok(Rule {
        kind: rule.kind.clone(),
        value,
    })
}

impl Rule {
    /// 這個事件符不符合這條規則。網址、標題的規則只看瀏覽頁面的事件，
    /// 不然同一頁的點擊和捲動都會被算進去。
    pub fn matches(&self, event: &Event) -> bool {
        let value = self.value.to_ascii_lowercase();
        let pageview = event.kind == "pageview";
        match self.kind.as_str() {
            "path_prefix" => {
                let path = event.path.to_ascii_lowercase();
                pageview
                    && (value == "/"
                        || path == value
                        || path
                            .strip_prefix(&value)
                            .is_some_and(|rest| rest.starts_with('/')))
            }
            "url_contains" => pageview && event.url.to_ascii_lowercase().contains(&value),
            "title_contains" => pageview && event.title.to_ascii_lowercase().contains(&value),
            "event" => {
                matches!(event.kind, "custom" | "click") && event.name.to_ascii_lowercase() == value
            }
            _ => false,
        }
    }

    /// 和 [`Rule::matches`] 同樣意思的 SQL 條件（`events` 的別名是 `e`）。
    fn push_sql(&self, qb: &mut QueryBuilder<'_, Sqlite>) {
        let value = self.value.to_ascii_lowercase();
        match self.kind.as_str() {
            "path_prefix" if value == "/" => {
                qb.push("e.type = 'pageview'");
            }
            "path_prefix" => {
                qb.push("(e.type = 'pageview' AND (lower(e.path) = ")
                    .push_bind(value.clone())
                    .push(" OR substr(lower(e.path), 1, ")
                    .push_bind(value.chars().count() as i64 + 1)
                    .push(") = ")
                    .push_bind(format!("{value}/"))
                    .push("))");
            }
            "url_contains" => {
                qb.push("(e.type = 'pageview' AND instr(lower(e.url), ")
                    .push_bind(value)
                    .push(") > 0)");
            }
            "title_contains" => {
                qb.push("(e.type = 'pageview' AND instr(lower(e.title), ")
                    .push_bind(value)
                    .push(") > 0)");
            }
            "event" => {
                qb.push("(e.type IN ('custom', 'click') AND lower(e.name) = ")
                    .push_bind(value)
                    .push(")");
            }
            _ => {
                qb.push("0");
            }
        }
    }
}

/// 一個網站所有標籤的規則：（標籤編號、規則）。
pub async fn site_rules(
    conn: &mut SqliteConnection,
    site_id: i64,
) -> sqlx::Result<Vec<(i64, Rule)>> {
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT r.label_id, r.kind, r.value FROM label_rules r
         JOIN labels l ON l.id = r.label_id WHERE l.site_id = ?",
    )
    .bind(site_id)
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(label_id, kind, value)| (label_id, Rule { kind, value }))
        .collect())
}

/// 這個事件符合哪些標籤（一個標籤只算一次）。
pub fn matching(rules: &[(i64, Rule)], event: &Event) -> Vec<i64> {
    let mut ids: Vec<i64> = Vec::new();
    for (label_id, rule) in rules {
        if !ids.contains(label_id) && rule.matches(event) {
            ids.push(*label_id);
        }
    }
    ids
}

/// 用現有的事件，重算一個標籤每個人各符合幾次。
pub async fn recompute(conn: &mut SqliteConnection, label_id: i64) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM person_labels WHERE label_id = ?")
        .bind(label_id)
        .execute(&mut *conn)
        .await?;
    let rules: Vec<Rule> = sqlx::query_as("SELECT kind, value FROM label_rules WHERE label_id = ?")
        .bind(label_id)
        .fetch_all(&mut *conn)
        .await?;
    if rules.is_empty() {
        return Ok(());
    }
    let mut qb = QueryBuilder::new(
        "INSERT INTO person_labels (person_id, label_id, hits, last_at)
         SELECT e.person_id, ",
    );
    qb.push_bind(label_id)
        .push(
            ", COUNT(*), MAX(e.created_at) FROM events e
             WHERE e.site_id = (SELECT site_id FROM labels WHERE id = ",
        )
        .push_bind(label_id)
        .push(") AND (");
    for (i, rule) in rules.iter().enumerate() {
        if i > 0 {
            qb.push(" OR ");
        }
        rule.push_sql(&mut qb);
    }
    qb.push(") GROUP BY e.person_id");
    qb.build().execute(&mut *conn).await?;
    Ok(())
}

/// 重算一個網站的所有標籤（清掉舊事件之後用）。
pub async fn recompute_site(conn: &mut SqliteConnection, site_id: i64) -> sqlx::Result<()> {
    let ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM labels WHERE site_id = ?")
        .bind(site_id)
        .fetch_all(&mut *conn)
        .await?;
    for id in ids {
        recompute(&mut *conn, id).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(kind: &str, value: &str) -> Rule {
        Rule {
            kind: kind.into(),
            value: value.into(),
        }
    }

    fn pageview(path: &str, title: &str) -> Event {
        Event {
            event_id: String::new(),
            happened_at: None,
            kind: "pageview",
            name: String::new(),
            url: format!("https://www.example.com{path}?ref=mail"),
            path: path.into(),
            title: title.into(),
            referrer: String::new(),
            referrer_host: String::new(),
            props: "{}".into(),
        }
    }

    fn custom(name: &str) -> Event {
        Event {
            event_id: String::new(),
            happened_at: None,
            kind: "custom",
            name: name.into(),
            ..pageview("/cart", "")
        }
    }

    #[test]
    fn path_prefix_matches_whole_segments() {
        let blog = rule("path_prefix", "/blog");
        assert!(blog.matches(&pageview("/blog", "")));
        assert!(blog.matches(&pageview("/Blog/2026/hello", "")));
        assert!(!blog.matches(&pageview("/blogger", "")));
        assert!(!blog.matches(&pageview("/", "")));
        assert!(rule("path_prefix", "/").matches(&pageview("/anything", "")));
        // 同一頁的點擊不算
        let click = Event {
            event_id: String::new(),
            happened_at: None,
            kind: "click",
            name: "了解更多".into(),
            ..pageview("/blog", "")
        };
        assert!(!blog.matches(&click));
    }

    #[test]
    fn contains_and_event_rules_ignore_ascii_case() {
        assert!(rule("url_contains", "REF=mail").matches(&pageview("/x", "")));
        assert!(rule("title_contains", "enterprise").matches(&pageview("/x", "Enterprise 方案")));
        assert!(rule("title_contains", "方案").matches(&pageview("/x", "Enterprise 方案")));
        assert!(!rule("title_contains", "價格").matches(&pageview("/x", "Enterprise 方案")));
        assert!(rule("event", "Add_To_Cart").matches(&custom("add_to_cart")));
        assert!(!rule("event", "add_to_cart").matches(&pageview("/add_to_cart", "")));
    }

    #[test]
    fn rules_are_cleaned() {
        assert_eq!(
            clean_rule(&rule("path_prefix", " /blog/ ")).unwrap().value,
            "/blog"
        );
        assert_eq!(clean_rule(&rule("path_prefix", "/")).unwrap().value, "/");
        assert!(clean_rule(&rule("path_prefix", "blog")).is_err());
        assert!(clean_rule(&rule("event", "  ")).is_err());
        assert!(clean_rule(&rule("regex", ".*")).is_err());
    }

    #[test]
    fn an_event_counts_once_per_label() {
        let rules = vec![
            (1, rule("path_prefix", "/pricing")),
            (1, rule("title_contains", "價格")),
            (2, rule("event", "signup")),
        ];
        assert_eq!(matching(&rules, &pageview("/pricing", "價格")), vec![1]);
        assert_eq!(matching(&rules, &custom("signup")), vec![2]);
        assert!(matching(&rules, &pageview("/about", "關於")).is_empty());
    }
}
