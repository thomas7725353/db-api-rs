mod model;
mod repository;

use axum::{routing::get, Router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let app = Router::new().route("/health", get(|| async { "OK" }));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8520").await?;
    println!("db-api-rs listening on :8520");
    axum::serve(listener, app).await?;
    Ok(())
}
