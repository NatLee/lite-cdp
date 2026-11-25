//! 標籤的設定。看得到這個網站的人都可以改。

use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    auth::CurrentUser,
    db,
    error::{AppError, AppResult},
    labels::{self, Rule},
    routes::sites::ensure_visible,
    segments::Rules,
    state::AppState,
};

/// 標籤可以選的顏色（前端 Badge 的色調）。
pub const COLORS: [&str; 8] = [
    "neutral", "brand", "green", "amber", "red", "sky", "violet", "pink",
];
const MAX_RULES: usize = 20;

/// 貼在人或頁面旁邊的小標籤。
#[derive(Serialize, Clone, sqlx::FromRow)]
pub struct LabelChip {
    pub id: i64,
    pub name: String,
    pub color: String,
}

impl LabelChip {
    pub async fn of_site(state: &AppState, site_id: i64) -> AppResult<Vec<Self>> {
        Ok(
            sqlx::query_as("SELECT id, name, color FROM labels WHERE site_id = ? ORDER BY name")
                .bind(site_id)
                .fetch_all(&state.db)
                .await?,
        )
    }
}

#[derive(sqlx::FromRow)]
struct LabelRow {
    id: i64,
    name: String,
    color: String,
    description: String,
    min_hits: i64,
    created_at: String,
    people: i64,
}

#[derive(Serialize)]
pub struct Label {
    id: i64,
    name: String,
    color: String,
    description: String,
    /// 符合規則的事件要有幾次，才算有這個標籤
    min_hits: i64,
    /// 事件符合任何一條就算一次
    rules: Vec<Rule>,
    /// 現在有這個標籤的人數
    people: i64,
    created_at: String,
}

const LABEL_SELECT: &str = "SELECT l.id, l.name, l.color, l.description, l.min_hits, l.created_at,
        (SELECT COUNT(*) FROM person_labels pl
          WHERE pl.label_id = l.id AND pl.hits >= l.min_hits) AS people
     FROM labels l";

async fn rules_by_label(state: &AppState, site_id: i64) -> AppResult<HashMap<i64, Vec<Rule>>> {
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT r.label_id, r.kind, r.value FROM label_rules r
         JOIN labels l ON l.id = r.label_id WHERE l.site_id = ? ORDER BY r.id",
    )
    .bind(site_id)
    .fetch_all(&state.db)
    .await?;
    let mut map: HashMap<i64, Vec<Rule>> = HashMap::new();
    for (label_id, kind, value) in rows {
        map.entry(label_id).or_default().push(Rule { kind, value });
    }
    Ok(map)
}

fn present(row: LabelRow, rules: Vec<Rule>) -> Label {
    Label {
        id: row.id,
        name: row.name,
        color: row.color,
        description: row.description,
        min_hits: row.min_hits,
        rules,
        people: row.people,
        created_at: row.created_at,
    }
}

pub async fn list(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(site_id): Path<i64>,
) -> AppResult<Json<Vec<Label>>> {
    ensure_visible(&state, &user, site_id).await?;
    let rows: Vec<LabelRow> = sqlx::query_as(&format!(
        "{LABEL_SELECT} WHERE l.site_id = ? ORDER BY l.name"
    ))
    .bind(site_id)
    .fetch_all(&state.db)
    .await?;
    let mut rules = rules_by_label(&state, site_id).await?;
    Ok(Json(
        rows.into_iter()
            .map(|row| {
                let r = rules.remove(&row.id).unwrap_or_default();
                present(row, r)
            })
            .collect(),
    ))
}

async fn one(state: &AppState, site_id: i64, id: i64) -> AppResult<Label> {
    let row: Option<LabelRow> =
        sqlx::query_as(&format!("{LABEL_SELECT} WHERE l.site_id = ? AND l.id = ?"))
            .bind(site_id)
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    let row = row.ok_or_else(|| AppError::not_found("找不到這個標籤"))?;
    let rules = rules_by_label(state, site_id)
        .await?
        .remove(&id)
        .unwrap_or_default();
    Ok(present(row, rules))
}

#[derive(Deserialize, Default)]
pub struct LabelBody {
    name: Option<String>,
    color: Option<String>,
    description: Option<String>,
    min_hits: Option<i64>,
    rules: Option<Vec<Rule>>,
}

fn clean_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 40 {
        return Err(AppError::bad("標籤名稱要 1 到 40 個字元"));
    }
    Ok(name.to_owned())
}

fn clean_color(color: &str) -> AppResult<String> {
    if !COLORS.contains(&color) {
        return Err(AppError::bad(format!("顏色只能是：{}", COLORS.join("、"))));
    }
    Ok(color.to_owned())
}

fn clean_description(text: &str) -> AppResult<String> {
    let text = text.trim();
    if text.chars().count() > 200 {
        return Err(AppError::bad("說明最多 200 個字元"));
    }
    Ok(text.to_owned())
}

fn clean_min_hits(value: i64) -> AppResult<i64> {
    if !(1..=1000).contains(&value) {
        return Err(AppError::bad("次數要在 1 到 1000 之間"));
    }
    Ok(value)
}

fn clean_rules(rules: &[Rule]) -> AppResult<Vec<Rule>> {
    let mut out: Vec<Rule> = Vec::new();
    for rule in rules {
        let rule = labels::clean_rule(rule).map_err(AppError::bad)?;
        if !out.contains(&rule) {
            out.push(rule);
        }
    }
    if out.is_empty() {
        return Err(AppError::bad("至少要有一條規則"));
    }
    if out.len() > MAX_RULES {
        return Err(AppError::bad(format!("規則最多 {MAX_RULES} 條")));
    }
    Ok(out)
}

async fn name_taken(state: &AppState, site_id: i64, name: &str, except: i64) -> AppResult<bool> {
    let taken: Option<i64> =
        sqlx::query_scalar("SELECT id FROM labels WHERE site_id = ? AND name = ? AND id != ?")
            .bind(site_id)
            .bind(name)
            .bind(except)
            .fetch_optional(&state.db)
            .await?;
    Ok(taken.is_some())
}

/// 換掉一個標籤的規則，再用現有的事件重算誰有這個標籤。
async fn replace_rules(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    label_id: i64,
    rules: &[Rule],
) -> AppResult<()> {
    sqlx::query("DELETE FROM label_rules WHERE label_id = ?")
        .bind(label_id)
        .execute(&mut **tx)
        .await?;
    for rule in rules {
        sqlx::query("INSERT INTO label_rules (label_id, kind, value) VALUES (?, ?, ?)")
            .bind(label_id)
            .bind(&rule.kind)
            .bind(&rule.value)
            .execute(&mut **tx)
            .await?;
    }
    labels::recompute(&mut *tx, label_id).await?;
    Ok(())
}

pub async fn create(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(site_id): Path<i64>,
    Json(body): Json<LabelBody>,
) -> AppResult<Json<Label>> {
    ensure_visible(&state, &user, site_id).await?;
    let name = clean_name(body.name.as_deref().unwrap_or_default())?;
    if name_taken(&state, site_id, &name, 0).await? {
        return Err(AppError::conflict("已經有同名的標籤"));
    }
    let color = clean_color(body.color.as_deref().unwrap_or("neutral"))?;
    let description = clean_description(body.description.as_deref().unwrap_or_default())?;
    let min_hits = clean_min_hits(body.min_hits.unwrap_or(1))?;
    let rules = clean_rules(body.rules.as_deref().unwrap_or_default())?;

    let mut tx = state.db.begin().await?;
    let id = sqlx::query(
        "INSERT INTO labels (site_id, name, color, description, min_hits, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(site_id)
    .bind(name)
    .bind(color)
    .bind(description)
    .bind(min_hits)
    .bind(db::now())
    .execute(&mut *tx)
    .await?
    .last_insert_rowid();
    replace_rules(&mut tx, id, &rules).await?;
    tx.commit().await?;
    Ok(Json(one(&state, site_id, id).await?))
}

pub async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((site_id, id)): Path<(i64, i64)>,
    Json(body): Json<LabelBody>,
) -> AppResult<Json<Label>> {
    ensure_visible(&state, &user, site_id).await?;
    let current = one(&state, site_id, id).await?;
    let name = match &body.name {
        Some(name) => clean_name(name)?,
        None => current.name,
    };
    if name_taken(&state, site_id, &name, id).await? {
        return Err(AppError::conflict("已經有同名的標籤"));
    }
    let color = match &body.color {
        Some(color) => clean_color(color)?,
        None => current.color,
    };
    let description = match &body.description {
        Some(text) => clean_description(text)?,
        None => current.description,
    };
    let min_hits = clean_min_hits(body.min_hits.unwrap_or(current.min_hits))?;
    let rules = body.rules.as_deref().map(clean_rules).transpose()?;

    let mut tx = state.db.begin().await?;
    sqlx::query(
        "UPDATE labels SET name = ?, color = ?, description = ?, min_hits = ? WHERE id = ?",
    )
    .bind(name)
    .bind(color)
    .bind(description)
    .bind(min_hits)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    // 規則沒變就不用重算
    if let Some(rules) = rules.filter(|rules| *rules != current.rules) {
        replace_rules(&mut tx, id, &rules).await?;
    }
    tx.commit().await?;
    Ok(Json(one(&state, site_id, id).await?))
}

pub async fn remove(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((site_id, id)): Path<(i64, i64)>,
) -> AppResult<Json<Value>> {
    ensure_visible(&state, &user, site_id).await?;
    one(&state, site_id, id).await?;
    // 還有分群在用的話先擋下來，不然那個分群的條件會悄悄變成永遠不成立
    let segments: Vec<(String, String)> =
        sqlx::query_as("SELECT name, rules FROM segments WHERE site_id = ? ORDER BY name")
            .bind(site_id)
            .fetch_all(&state.db)
            .await?;
    let used_by: Vec<String> = segments
        .into_iter()
        .filter(|(_, rules)| {
            serde_json::from_str::<Rules>(rules).is_ok_and(|r| r.label_ids().contains(&id))
        })
        .map(|(name, _)| format!("「{name}」"))
        .collect();
    if !used_by.is_empty() {
        return Err(AppError::conflict(format!(
            "分群{}還在用這個標籤，先把它從條件裡拿掉",
            used_by.join("、")
        )));
    }
    sqlx::query("DELETE FROM labels WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}
