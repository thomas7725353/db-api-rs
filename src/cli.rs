pub async fn run() -> anyhow::Result<()> {
    crate::app::serve_http().await
}
