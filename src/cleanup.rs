use chrono::{Duration, Utc};

use crate::state::AppState;

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            if let Err(error) = run(&state).await {
                tracing::error!(%error, "cleanup job failed");
            }
            state.limits.prune();
        }
    });
}

async fn run(state: &AppState) -> anyhow::Result<()> {
    let now = Utc::now();
    let empty_cutoff = (now - Duration::days(30)).to_rfc3339();
    let delete_cutoff = (now - Duration::days(7)).to_rfc3339();
    let sessions_now = now.to_rfc3339();
    let creation_cutoff = (now - Duration::days(2)).format("%Y-%m-%d").to_string();
    let mut tx = state.pool.begin().await?;
    let empty = sqlx::query("DELETE FROM vaults WHERE ever_used = 0 AND status = 'active' AND created_at < ? AND NOT EXISTS (SELECT 1 FROM blocks WHERE vault_id = vaults.id) AND NOT EXISTS (SELECT 1 FROM bundles WHERE vault_id = vaults.id) AND NOT EXISTS (SELECT 1 FROM notification_targets WHERE vault_id = vaults.id)")
        .bind(empty_cutoff).execute(&mut *tx).await?.rows_affected();
    let deleted = sqlx::query("DELETE FROM vaults WHERE status = 'deleted' AND deleted_at < ?")
        .bind(delete_cutoff).execute(&mut *tx).await?.rows_affected();
    sqlx::query("DELETE FROM admin_sessions WHERE expires_at < ?").bind(sessions_now).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM creation_limits WHERE bucket < ?").bind(creation_cutoff).execute(&mut *tx).await?;
    tx.commit().await?;
    sqlx::query("PRAGMA wal_checkpoint(PASSIVE)").execute(&state.pool).await?;
    if empty + deleted > 0 { tracing::info!(empty, deleted, "expired data cleaned"); }
    Ok(())
}
