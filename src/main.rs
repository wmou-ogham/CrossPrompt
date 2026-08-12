mod config;
mod db;
mod error;
mod models;
mod rate_limit;
mod security;
mod state;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cross_prompt=info".into()),
        )
        .init();
    let config = config::Config::from_env()?;
    let pool = db::connect(&config).await?;
    let _state = state::AppState::new(config, pool)?;
    tracing::info!("CrossPrompt foundation initialized");
    Ok(())
}
