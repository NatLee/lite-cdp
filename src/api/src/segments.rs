//! 分群：一組存起來的條件，打開時才去算現在有哪些人符合。
//!
//! 條件分三組：`must` 每一條都要符合、`should` 至少符合一條（沒填就不看）、
//! `must_not` 一條都不能符合。

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Sqlite};

use crate::db;

pub const KINDS: [&str; 3] = ["all", "member", "anonymous"];
const MAX_CONDITIONS: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "field", content = "value", rename_all = "snake_case")]
pub enum Condition {
    /// 有這個標籤（標籤的編號）
    Label(i64),
    /// 從網域含有這段文字的網站連進來過
    Referrer(String),
    /// email 含有這段文字
    Email(String),
    /// 最近幾天內來過
    SeenWithin(i64),
    /// 最近幾天內第一次來
    FirstSeenWithin(i64),
    /// 至少看過幾頁
    MinPageviews(i64),
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Rules {
    #[serde(default)]
    pub must: Vec<Condition>,
    #[serde(default)]
    pub should: Vec<Condition>,
    #[serde(default)]
    pub must_not: Vec<Condition>,
}

impl Condition {
    fn clean(self) -> Result<Self, String> {
        let text = |value: String, what: &str| {
            let value = value.trim().to_lowercase();
            if value.is_empty() || value.chars().count() > 100 {
                Err(format!("{what}要 1 到 100 個字元"))
            } else {
                Ok(value)
            }
        };
        let number = |value: i64, max: i64, what: &str| {
            if (1..=max).contains(&value) {
                Ok(value)
            } else {
                Err(format!("{what}要在 1 到 {max} 之間"))
            }
        };
        Ok(match self {
            Self::Label(id) => Self::Label(id),
            Self::Referrer(v) => Self::Referrer(text(v, "來源網域")?),
            Self::Email(v) => Self::Email(text(v, "Email ")?),
            Self::SeenWithin(v) => Self::SeenWithin(number(v, 3650, "天數")?),
            Self::FirstSeenWithin(v) => Self::FirstSeenWithin(number(v, 3650, "天數")?),
            Self::MinPageviews(v) => Self::MinPageviews(number(v, 1_000_000, "瀏覽頁數")?),
        })
    }

    fn push_sql(&self, qb: &mut QueryBuilder<'_, Sqlite>, now: DateTime<Utc>) {
        let since = |days: i64| db::format_time(now - Duration::days(days));
        match self {
            Self::Label(id) => {
                qb.push(
                    "EXISTS (SELECT 1 FROM person_labels pl JOIN labels l ON l.id = pl.label_id
                      WHERE pl.person_id = p.id AND pl.hits >= l.min_hits AND pl.label_id = ",
                )
                .push_bind(*id)
                .push(")");
            }
            Self::Referrer(value) => {
                qb.push(
                    "EXISTS (SELECT 1 FROM events e
                      WHERE e.person_id = p.id AND instr(e.referrer_host, ",
                )
                .push_bind(value.clone())
                .push(") > 0)");
            }
            Self::Email(value) => {
                qb.push("instr(p.email, ")
                    .push_bind(value.clone())
                    .push(") > 0");
            }
            Self::SeenWithin(days) => {
                qb.push("p.last_seen >= ").push_bind(since(*days));
            }
            Self::FirstSeenWithin(days) => {
                qb.push("p.first_seen >= ").push_bind(since(*days));
            }
            Self::MinPageviews(count) => {
                qb.push("p.pageviews >= ").push_bind(*count);
            }
        }
    }
}

impl Rules {
    /// 檢查並整理；不合法時回給使用者看的訊息。
    pub fn clean(self) -> Result<Self, String> {
        let group = |conditions: Vec<Condition>| {
            if conditions.len() > MAX_CONDITIONS {
                return Err(format!("每一組最多 {MAX_CONDITIONS} 個條件"));
            }
            let mut out: Vec<Condition> = Vec::new();
            for condition in conditions {
                let condition = condition.clean()?;
                if !out.contains(&condition) {
                    out.push(condition);
                }
            }
            Ok(out)
        };
        Ok(Self {
            must: group(self.must)?,
            should: group(self.should)?,
            must_not: group(self.must_not)?,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.must.is_empty() && self.should.is_empty() && self.must_not.is_empty()
    }

    /// 用到的標籤編號。
    pub fn label_ids(&self) -> Vec<i64> {
        let mut ids: Vec<i64> = [&self.must, &self.should, &self.must_not]
            .into_iter()
            .flatten()
            .filter_map(|c| match c {
                Condition::Label(id) => Some(*id),
                _ => None,
            })
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// 接在 `... FROM people p WHERE ...` 後面的條件（每一段都以 ` AND ` 開頭）。
    pub fn push_where(&self, qb: &mut QueryBuilder<'_, Sqlite>, kind: &str, now: DateTime<Utc>) {
        match kind {
            "member" => {
                qb.push(" AND p.email != ''");
            }
            "anonymous" => {
                qb.push(" AND p.email = ''");
            }
            _ => {}
        }
        for condition in &self.must {
            qb.push(" AND ");
            condition.push_sql(qb, now);
        }
        if !self.should.is_empty() {
            qb.push(" AND (");
            for (i, condition) in self.should.iter().enumerate() {
                if i > 0 {
                    qb.push(" OR ");
                }
                condition.push_sql(qb, now);
            }
            qb.push(")");
        }
        for condition in &self.must_not {
            qb.push(" AND NOT (");
            condition.push_sql(qb, now);
            qb.push(")");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rules_round_trip_as_json() {
        let rules: Rules = serde_json::from_value(json!({
            "must": [{ "field": "label", "value": 3 }, { "field": "seen_within", "value": 14 }],
            "must_not": [{ "field": "email", "value": "@Example.com " }],
        }))
        .unwrap();
        let rules = rules.clean().unwrap();
        assert_eq!(
            rules.must,
            vec![Condition::Label(3), Condition::SeenWithin(14)]
        );
        assert!(rules.should.is_empty());
        assert_eq!(
            rules.must_not,
            vec![Condition::Email("@example.com".into())]
        );
        assert_eq!(rules.label_ids(), vec![3]);
        assert_eq!(
            serde_json::to_value(&rules).unwrap()["must"][0],
            json!({ "field": "label", "value": 3 })
        );
    }

    #[test]
    fn bad_conditions_are_rejected() {
        let parse = |value| serde_json::from_value::<Rules>(value).map(Rules::clean);
        assert!(
            parse(json!({ "must": [{ "field": "seen_within", "value": 0 }] }))
                .unwrap()
                .is_err()
        );
        assert!(
            parse(json!({ "must": [{ "field": "referrer", "value": " " }] }))
                .unwrap()
                .is_err()
        );
        assert!(parse(json!({ "must": [{ "field": "age", "value": 3 }] })).is_err());
        assert!(parse(json!({ "must": [{ "field": "label", "value": "3" }] })).is_err());
        let many: Vec<_> = (1..=11)
            .map(|i| json!({ "field": "min_pageviews", "value": i }))
            .collect();
        assert!(parse(json!({ "should": many })).unwrap().is_err());
    }

    #[test]
    fn duplicates_are_dropped() {
        let rules = Rules {
            must: vec![Condition::Label(1), Condition::Label(1)],
            ..Default::default()
        }
        .clean()
        .unwrap();
        assert_eq!(rules.must, vec![Condition::Label(1)]);
        assert!(!rules.is_empty());
        assert!(Rules::default().is_empty());
    }
}
