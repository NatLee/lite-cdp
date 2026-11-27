use crate::{
    crypto::{now, random},
    types::{AuthContext, Transport},
    AccessError,
};
use sqlx::{Row, SqlitePool};

pub(crate) async fn reserve(
    db: &SqlitePool,
    auth: &AuthContext,
    transport: Transport,
    operation: &str,
) -> Result<String, AccessError> {
    let id = auth.request_id.clone();
    let at = now();
    let day = day(at);
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::query("INSERT INTO access_events(request_id,at,kind,transport,operation,user_id,client_id,credential_id,grant_id,status,outcome) VALUES(?,?,'operation',?,?,?,?,?,?,0,'pending')").bind(&id).bind(at).bind(transport.as_str()).bind(operation).bind(auth.principal.user_id).bind(&auth.client_id).bind(&auth.credential_id).bind(&auth.grant_id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO access_daily(day,user_id,transport,attempts,pending) VALUES(?,?,?,1,1) ON CONFLICT(day,user_id,transport) DO UPDATE SET attempts=attempts+1,pending=pending+1").bind(day).bind(auth.principal.user_id).bind(transport.as_str()).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(id)
}
pub(crate) async fn finish(
    db: &SqlitePool,
    id: &str,
    status: u16,
    duration: i64,
    interrupted: bool,
) -> Result<(), AccessError> {
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await?;
    let row = sqlx::query(
        "SELECT at,user_id,transport FROM access_events WHERE request_id=? AND outcome='pending'",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(row) = row {
        let success = (200..300).contains(&status) && !interrupted;
        let changed=sqlx::query("UPDATE access_events SET status=?,duration_ms=?,outcome=? WHERE request_id=? AND outcome='pending'").bind(i64::from(status)).bind(duration).bind(if interrupted{"interrupted"}else if success{"success"}else{"failure"}).bind(id).execute(&mut *tx).await?.rows_affected();
        if changed == 1 {
            sqlx::query("UPDATE access_daily SET successes=successes+?,failures=failures+?,pending=pending-1 WHERE day=? AND user_id=? AND transport=?").bind(i64::from(success)).bind(i64::from(!success)).bind(day(row.get("at"))).bind(row.get::<i64,_>("user_id")).bind(row.get::<String,_>("transport")).execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}
pub(crate) async fn security(
    db: &SqlitePool,
    transport: Transport,
    operation: &str,
    status: u16,
    user: Option<i64>,
) -> Result<(), AccessError> {
    sqlx::query("INSERT INTO access_events(request_id,at,kind,transport,operation,user_id,status,outcome) VALUES(?,?,'security',?,?,?,?,'failure')").bind(random()).bind(now()).bind(transport.as_str()).bind(operation).bind(user).bind(i64::from(status)).execute(db).await?;
    Ok(())
}
pub(crate) fn day(at: i64) -> String {
    chrono::DateTime::from_timestamp(at, 0)
        .unwrap_or_default()
        .format("%Y-%m-%d")
        .to_string()
}
pub(crate) async fn maintain(db: &SqlitePool) -> Result<(), AccessError> {
    let rows=sqlx::query("SELECT request_id FROM access_events WHERE outcome='pending' AND at<=? ORDER BY at LIMIT 100").bind(now()-600).fetch_all(db).await?;
    for row in rows {
        finish(db, &row.get::<String, _>("request_id"), 503, 600_000, true).await?;
    }
    sqlx::query("DELETE FROM access_events WHERE id IN(SELECT id FROM access_events WHERE at<? AND outcome!='pending' LIMIT 1000)").bind(now()-90*86400).execute(db).await?;
    for table in ["access_provider_flows", "access_consents", "access_codes"] {
        sqlx::query(&format!("DELETE FROM {table} WHERE rowid IN(SELECT rowid FROM {table} WHERE expires_at<? LIMIT 1000)")).bind(now()).execute(db).await?;
    }
    Ok(())
}
