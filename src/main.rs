use axum::{routing::get, Router};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let app = create_app();
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8520").await?;
    println!("db-api-rs listening on :8520");
    axum::serve(listener, app).await?;
    Ok(())
}

fn create_app() -> Router {
    Router::new().route("/health", get(|| async { "OK" }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_health_check() {
        let app = create_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(&body[..], b"OK");
    }
}
