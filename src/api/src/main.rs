use anyhow::{bail, Result};
use lite_cdp::{app, auth, build_state, demo, retention, Config};
use std::net::SocketAddr;

use tokio::net::TcpListener;

const USAGE: &str = "用法：
  lite-cdp                          啟動伺服器
  lite-cdp reset-password <帳號>    重設某個帳號的密碼，新密碼會印在畫面上
  lite-cdp demo                     在空的資料庫裡建立範例帳號、網站和訪客，密碼會印在畫面上
                                         （要自己指定密碼的話設 CDP_DEMO_PASSWORD）

設定用環境變數，見 README。";

#[tokio::main]
async fn main() -> Result<()> {
    lite_cdp::logging::init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None => serve(Config::from_env()?).await,
        Some("reset-password") => match args.get(1) {
            Some(username) => reset_password(Config::from_env()?, username).await,
            None => bail!("{USAGE}"),
        },
        Some("demo") => seed_demo(Config::from_env()?).await,
        Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            Ok(())
        }
        Some(other) => bail!("不認得的指令：{other}\n\n{USAGE}"),
    }
}

async fn serve(config: Config) -> Result<()> {
    tracing::info!(
        "Lite CDP v{}｜資料庫 {}",
        env!("CARGO_PKG_VERSION"),
        config.database_path.display()
    );
    let bind = config.bind.clone();
    let state = build_state(config).await?;
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await?;
    if users == 0 {
        tracing::info!("還沒有任何帳號：打開網頁建立第一個管理員");
    }
    if state.config.retention_days > 0 {
        tracing::info!("事件只留 {} 天", state.config.retention_days);
    }
    retention::spawn(state.clone());
    let listener = TcpListener::bind(&bind).await?;
    tracing::info!("網頁介面：http://{}", listener.local_addr()?);
    // 要知道連進來的 IP，收追蹤資料的流量限制才有辦法算
    let service = app(state).into_make_service_with_connect_info::<SocketAddr>();
    axum::serve(listener, service)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("已停止");
    Ok(())
}

async fn reset_password(config: Config, username: &str) -> Result<()> {
    let state = build_state(config).await?;
    let id: Option<i64> = sqlx::query_scalar("SELECT id FROM users WHERE username = ?")
        .bind(username)
        .fetch_optional(&state.db)
        .await?;
    let Some(id) = id else {
        bail!("找不到帳號：{username}");
    };
    let password = auth::random_hex(10);
    auth::set_password(&state.db, id, &password)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    auth::delete_sessions_of(&state.db, id, None).await?;
    println!("{username} 的新密碼：{password}");
    println!("請登入後到「設定」改成自己的密碼。");
    Ok(())
}

async fn seed_demo(config: Config) -> Result<()> {
    let state = build_state(config).await?;
    let password = match lite_cdp::config::var("CDP_DEMO_PASSWORD") {
        Some(p) => p,
        None => auth::random_hex(8),
    };
    let summary = demo::seed(&state, &password).await?;
    println!(
        "建立了 {} 個帳號、{} 個網站、{} 個訪客、{} 筆事件。",
        summary.users, summary.sites, summary.people, summary.events
    );
    println!(
        "管理員帳號：demo　其他帳號：{}",
        demo::MEMBER_USERNAMES.join("、")
    );
    println!("密碼（所有範例帳號都一樣）：{password}");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
