use std::{fs, str::FromStr, time::Duration};

use anyhow::{Context, Result};
use chrono::Utc;
use serde::Serialize;
use sqlx::{sqlite::{SqliteConnectOptions, SqlitePoolOptions}, SqlitePool};

use crate::{config::Config, error::AppResult, models::{Block, Bundle, BundleRow, NotificationTargetView, Vault, VaultSnapshot}};

pub async fn connect(config: &Config) -> Result<SqlitePool> {
    if let Some(parent) = config.database_path.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create database directory {}", parent.display()))?;
    }
    let options = SqliteConnectOptions::from_str(&config.database_url)?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}

pub async fn snapshot(pool: &SqlitePool, vault: Vault) -> AppResult<VaultSnapshot> {
    let blocks = sqlx::query_as::<_, Block>(
        "SELECT * FROM blocks WHERE vault_id = ? ORDER BY position, created_at",
    )
    .bind(&vault.id)
    .fetch_all(pool)
    .await?;
    let rows = sqlx::query_as::<_, BundleRow>(
        "SELECT * FROM bundles WHERE vault_id = ? ORDER BY created_at",
    )
    .bind(&vault.id)
    .fetch_all(pool)
    .await?;
    let bundles = rows
        .into_iter()
        .filter_map(|row| Bundle::try_from(row).ok())
        .collect();
    let target = sqlx::query_as::<_, (String, String)>(
        "SELECT kind, updated_at FROM notification_targets WHERE vault_id = ?",
    )
    .bind(&vault.id)
    .fetch_optional(pool)
    .await?
    .map(|(kind, updated_at)| NotificationTargetView {
        kind,
        masked_url: "https://••••••••".into(),
        updated_at,
    });
    Ok(VaultSnapshot { vault, blocks, bundles, notification_target: target })
}

pub async fn revision<T: Serialize>(
    pool: &SqlitePool,
    vault_id: &str,
    resource_type: &str,
    resource_id: Option<&str>,
    action: &str,
    before: Option<&T>,
    after: Option<&T>,
    source: &str,
) -> AppResult<()> {
    let before = before.map(serde_json::to_string).transpose().map_err(anyhow::Error::from)?;
    let after = after.map(serde_json::to_string).transpose().map_err(anyhow::Error::from)?;
    sqlx::query("INSERT INTO revisions (vault_id, resource_type, resource_id, action, before_json, after_json, source, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(vault_id)
        .bind(resource_type)
        .bind(resource_id)
        .bind(action)
        .bind(before)
        .bind(after)
        .bind(source)
        .bind(Utc::now().to_rfc3339())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_used(pool: &SqlitePool, vault_id: &str) -> AppResult<()> {
    sqlx::query("UPDATE vaults SET ever_used = 1, updated_at = ? WHERE id = ?")
        .bind(Utc::now().to_rfc3339())
        .bind(vault_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn prune_revisions(pool: &SqlitePool, vault_id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM revisions WHERE vault_id = ? AND id NOT IN (SELECT id FROM revisions WHERE vault_id = ? ORDER BY id DESC LIMIT 100)")
        .bind(vault_id)
        .bind(vault_id)
        .execute(pool)
        .await?;
    Ok(())
}
