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
    sqlx::query("DELETE FROM admin_sessions WHERE expires_at < ?").bind(&sessions_now).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM vault_email_sessions WHERE expires_at < ?").bind(&sessions_now).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM email_otp_challenges WHERE expires_at < ? OR consumed_at IS NOT NULL")
        .bind(&sessions_now).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM creation_limits WHERE bucket < ?").bind(creation_cutoff).execute(&mut *tx).await?;
    tx.commit().await?;
    sqlx::query("PRAGMA wal_checkpoint(PASSIVE)").execute(&state.pool).await?;
    if empty + deleted > 0 { tracing::info!(empty, deleted, "expired data cleaned"); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, db};
    use uuid::Uuid;

    #[tokio::test]
    async fn cleanup_only_removes_truly_empty_and_expired_deleted_vaults() {
        let path = std::path::PathBuf::from(format!(
            "/tmp/crossprompt-cleanup-test-{}.db",
            Uuid::new_v4()
        ));
        let config = Config {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            database_url: format!("sqlite://{}", path.display()),
            database_path: path.clone(),
            frontend_dir: "/tmp/empty".into(),
            public_base_url: "http://test".into(),
            app_env: "test".into(),
            admin_username: "admin".into(),
            admin_password_hash: "$argon2id$v=19$m=19456,t=2,p=1$Y3Jvc3Nwcm9tcC10ZXN0$AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8".into(),
            session_secret: "test-session-secret-that-is-long-enough".into(),
            master_key: [3; 32],
            ip_hash_salt: "test-ip-hash-salt-long-enough".into(),
            turnstile_secret_key: None,
            turnstile_site_key: None,
            cookie_secure: false,
            trust_proxy: false,
            smtp: None,
        };
        let pool = db::connect(&config).await.unwrap();
        let state = AppState::new(config, pool.clone()).unwrap();
        let old = (Utc::now() - Duration::days(31)).to_rfc3339();
        let deleted_old = (Utc::now() - Duration::days(8)).to_rfc3339();
        let deleted_recent = (Utc::now() - Duration::days(6)).to_rfc3339();

        insert_vault(&pool, "empty-old", false, "active", &old, None).await;
        insert_vault(&pool, "used-old", true, "active", &old, None).await;
        sqlx::query("INSERT INTO blocks (id, vault_id, title, content, position, created_at, updated_at) VALUES ('block-1', 'used-old', 'Prompt', 'keep me', 0, ?, ?)")
            .bind(&old).bind(&old).execute(&pool).await.unwrap();
        insert_vault(&pool, "configured-old", false, "active", &old, None).await;
        sqlx::query("INSERT INTO notification_targets (vault_id, kind, encrypted_config, created_at, updated_at) VALUES ('configured-old', 'ntfy', X'00', ?, ?)")
            .bind(&old).bind(&old).execute(&pool).await.unwrap();
        insert_vault(&pool, "deleted-expired", true, "deleted", &old, Some(&deleted_old)).await;
        insert_vault(&pool, "deleted-recent", true, "deleted", &old, Some(&deleted_recent)).await;

        run(&state).await.unwrap();

        let remaining = sqlx::query_scalar::<_, String>("SELECT id FROM vaults ORDER BY id")
            .fetch_all(&pool).await.unwrap();
        assert_eq!(remaining, vec!["configured-old", "deleted-recent", "used-old"]);

        pool.close().await;
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    async fn insert_vault(
        pool: &sqlx::SqlitePool,
        id: &str,
        ever_used: bool,
        status: &str,
        created_at: &str,
        deleted_at: Option<&str>,
    ) {
        sqlx::query("INSERT INTO vaults (id, secret_hash, name, status, ever_used, deleted_by, created_at, updated_at, deleted_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
            .bind(id)
            .bind(crate::security::digest(id))
            .bind(id)
            .bind(status)
            .bind(ever_used)
            .bind((status == "deleted").then_some("user"))
            .bind(created_at)
            .bind(created_at)
            .bind(deleted_at)
            .execute(pool)
            .await
            .unwrap();
    }
}
