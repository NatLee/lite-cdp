//! Upgrade an existing pre-policy database without changing historical identities or events.
use sqlx::sqlite::SqlitePoolOptions;

#[tokio::test]
async fn existing_email_identity_and_events_survive_additive_policy_upgrade() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    for migration in [
        include_str!("../migrations/0001_init.sql"),
        include_str!("../migrations/0002_access.sql"),
        include_str!("../migrations/0003_access_client_registration.sql"),
        include_str!("../migrations/0004_registration.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&pool).await.unwrap();
    }
    sqlx::raw_sql("INSERT INTO sites(id,name,key,created_at) VALUES(1,'old site','old-key','2026-01-01');
    INSERT INTO people(id,site_id,email,pageviews,first_seen,last_seen) VALUES(1,1,'old@example.com',1,'2026-01-01','2026-01-01');
    INSERT INTO devices(id,site_id,anon_id,person_id,first_seen,last_seen) VALUES(1,1,'old-anonymous',1,'2026-01-01','2026-01-01');
    INSERT INTO events(site_id,person_id,device_id,type,url,path,created_at) VALUES(1,1,1,'pageview','https://old.example/','/','2026-01-01');")
        .execute(&pool).await.unwrap();
    sqlx::raw_sql(include_str!("../migrations/0005_collection_identity.sql"))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!("../migrations/0006_identity_order.sql"))
        .execute(&pool)
        .await
        .unwrap();
    let ordering: (i64, String, i64) = sqlx::query_as(
        "SELECT identity_revision,identity_mutation_id,identity_at_ms FROM devices WHERE id=1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ordering, (0, String::new(), 0));
    let old: (String, String, i64) =
        sqlx::query_as("SELECT email,user_id,pageviews FROM people WHERE id=1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(old, ("old@example.com".into(), "".into(), 1));
    let site: (String, String) =
        sqlx::query_as("SELECT collection_policy,identity_secret FROM sites")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(site.0, "{}");
    assert_eq!(site.1.len(), 64);
    let event: (String, String, String) =
        sqlx::query_as("SELECT event_id,created_at,received_at FROM events")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(event, ("".into(), "2026-01-01".into(), "2026-01-01".into()));
    assert!(sqlx::query("INSERT INTO people(site_id,email,first_seen,last_seen) VALUES(1,'old@example.com','now','now')").execute(&pool).await.is_err());
    sqlx::query("INSERT INTO people(site_id,email,user_id,first_seen,last_seen) VALUES(1,'old@example.com','stable-member','now','now')").execute(&pool).await.unwrap();
    let devices: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM devices WHERE person_id=1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(devices, 1);
}
