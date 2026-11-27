//! Local OAuth/OIDC demo only; private key is an intentionally public test fixture.
#[path = "../tests/support/mock_provider.rs"]
mod mock_provider;
#[tokio::main]
async fn main() {
    let address = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:19090".into());
    let socket: std::net::SocketAddr = address.parse().expect("loopback socket address");
    assert!(
        socket.ip().is_loopback(),
        "mock provider must remain loopback-only"
    );
    let listener = tokio::net::TcpListener::bind(socket).await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    println!("Local test provider: {origin}");
    axum::serve(listener, mock_provider::MockProvider::router(&origin))
        .await
        .unwrap();
}
