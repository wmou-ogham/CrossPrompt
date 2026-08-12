use std::sync::Arc;

use reqwest::redirect::Policy;
use sqlx::SqlitePool;

use crate::{config::Config, rate_limit::RateLimits};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub pool: SqlitePool,
    pub limits: RateLimits,
    pub http: reqwest::Client,
}

impl AppState {
    pub fn new(config: Config, pool: SqlitePool) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .redirect(Policy::none())
            .timeout(std::time::Duration::from_secs(5))
            .user_agent("CrossPrompt/0.1")
            .build()?;
        Ok(Self { config: Arc::new(config), pool, limits: RateLimits::default(), http })
    }
}

