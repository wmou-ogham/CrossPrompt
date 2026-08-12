use std::{net::SocketAddr, time::Duration};

use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use chrono::{Duration as ChronoDuration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    artifact_types,
    auth::{user_vault, user_vault_allow_deleted},
    db,
    error::{AppError, AppResult},
    models::{Block, Bundle, BundleRow, CallbackPayload, NotificationConfig, Revision, Vault},
    notifications,
    security::{client_ip, digest, encrypt_config, mask_url, new_secret, salted_digest},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct CreateVaultInput {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub turnstile_token: Option<String>,
}

pub async fn create_vault(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(input): Json<CreateVaultInput>,
) -> AppResult<(StatusCode, Json<Value>)> {
    let request_ip = client_ip(&headers, peer, state.config.trust_proxy);
    verify_turnstile(&state, input.turnstile_token.as_deref(), request_ip.to_string()).await?;
    let ip_hash = salted_digest(&state.config.ip_hash_salt, &request_ip.to_string());
    let bucket = Utc::now().format("%Y-%m-%d").to_string();
    let mut tx = state.pool.begin().await?;
    let count: i64 = sqlx::query_scalar("SELECT count FROM creation_limits WHERE ip_hash = ? AND bucket = ?")
        .bind(&ip_hash)
        .bind(&bucket)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);
    if count >= 5 {
        return Err(AppError::RateLimited);
    }
    sqlx::query("INSERT INTO creation_limits (ip_hash, bucket, count) VALUES (?, ?, 1) ON CONFLICT(ip_hash, bucket) DO UPDATE SET count = count + 1")
        .bind(&ip_hash)
        .bind(&bucket)
        .execute(&mut *tx)
        .await?;
    let secret = new_secret();
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let name = clean_name(input.name.as_deref().unwrap_or("My CrossPrompt"))?;
    sqlx::query("INSERT INTO vaults (id, secret_hash, name, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(digest(&secret))
        .bind(&name)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(json!({
        "secret": secret,
        "manage_url": format!("{}/#/v/{}", state.config.public_base_url, secret),
        "vault": { "id": id, "name": name, "status": "active", "created_at": now }
    }))))
}

pub async fn get_vault(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    Ok(Json(serde_json::to_value(db::snapshot(&state.pool, vault).await?).unwrap()))
}

#[derive(Debug, Deserialize)]
pub struct RenameVaultInput { pub name: String }

pub async fn rename_vault(State(state): State<AppState>, headers: HeaderMap, Json(input): Json<RenameVaultInput>) -> AppResult<Json<Vault>> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    let name = clean_name(&input.name)?;
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE vaults SET name = ?, updated_at = ? WHERE id = ?")
        .bind(&name).bind(&now).bind(&vault.id).execute(&state.pool).await?;
    let after = sqlx::query_as::<_, Vault>("SELECT * FROM vaults WHERE id = ?").bind(&vault.id).fetch_one(&state.pool).await?;
    db::revision(&state.pool, &vault.id, "vault", Some(&vault.id), "update", Some(&vault), Some(&after), "web").await?;
    Ok(Json(after))
}

pub async fn delete_vault(State(state): State<AppState>, headers: HeaderMap) -> AppResult<StatusCode> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    let now = Utc::now().to_rfc3339();
    sqlx::query("UPDATE vaults SET status = 'deleted', deleted_by = 'user', deleted_at = ?, updated_at = ? WHERE id = ?")
        .bind(&now).bind(&now).bind(&vault.id).execute(&state.pool).await?;
    db::revision(&state.pool, &vault.id, "vault", Some(&vault.id), "delete", Some(&vault), Option::<&Vault>::None, "web").await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn restore_vault(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Vault>> {
    let vault = user_vault_allow_deleted(&state.pool, &headers).await?;
    if vault.status != "deleted" || vault.deleted_by.as_deref() != Some("user") {
        return Err(AppError::Forbidden);
    }
    let deleted_at = vault.deleted_at.as_deref().and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok()).ok_or(AppError::Gone)?;
    if deleted_at < Utc::now() - ChronoDuration::days(7) {
        return Err(AppError::Gone);
    }
    sqlx::query("UPDATE vaults SET status = 'active', deleted_by = NULL, deleted_at = NULL, updated_at = ? WHERE id = ?")
        .bind(Utc::now().to_rfc3339()).bind(&vault.id).execute(&state.pool).await?;
    Ok(Json(sqlx::query_as::<_, Vault>("SELECT * FROM vaults WHERE id = ?").bind(&vault.id).fetch_one(&state.pool).await?))
}

pub async fn rotate_secret(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    let secret = new_secret();
    sqlx::query("UPDATE vaults SET secret_hash = ?, updated_at = ? WHERE id = ?")
        .bind(digest(&secret)).bind(Utc::now().to_rfc3339()).bind(&vault.id).execute(&state.pool).await?;
    Ok(Json(json!({ "secret": secret, "manage_url": format!("{}/#/v/{}", state.config.public_base_url, secret) })))
}

#[derive(Debug, Deserialize)]
pub struct SourceQuery { #[serde(default = "default_source")] pub source: String }
fn default_source() -> String { "api".into() }

fn clean_source(source: &str) -> String {
    source.trim().chars().take(80).collect()
}

pub async fn list_blocks(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Vec<Block>>> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    Ok(Json(sqlx::query_as::<_, Block>("SELECT * FROM blocks WHERE vault_id = ? ORDER BY position, created_at")
        .bind(vault.id).fetch_all(&state.pool).await?))
}

#[derive(Debug, Deserialize)]
pub struct PortableTextInput { pub block_ids: Vec<String> }

pub async fn portable_text(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<PortableTextInput>,
) -> AppResult<Json<Value>> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    if input.block_ids.is_empty() {
        return Err(AppError::bad("block_ids must contain at least one block"));
    }
    let mut unique = input.block_ids.clone();
    unique.sort();
    unique.dedup();
    if unique.len() != input.block_ids.len() {
        return Err(AppError::bad("block_ids cannot contain duplicates"));
    }

    let available = sqlx::query_as::<_, Block>(
        "SELECT * FROM blocks WHERE vault_id = ? ORDER BY position, created_at",
    )
    .bind(&vault.id)
    .fetch_all(&state.pool)
    .await?;
    let ordered = input
        .block_ids
        .iter()
        .map(|id| available.iter().find(|block| &block.id == id).cloned())
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| AppError::bad("block_ids includes an unknown block"))?;

    Ok(Json(json!({
        "text": artifact_types::render_portable_pack(&ordered),
        "block_count": ordered.len(),
    })))
}

#[derive(Debug, Deserialize)]
pub struct CreateBlockInput {
    #[serde(default = "default_block_type")]
    pub block_type: String,
    pub title: String,
    pub content: String,
    pub position: Option<i64>,
}

fn default_block_type() -> String { "prompt".into() }

pub async fn create_block(State(state): State<AppState>, headers: HeaderMap, Query(source): Query<SourceQuery>, Json(input): Json<CreateBlockInput>) -> AppResult<(StatusCode, Json<Block>)> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    validate_block(&input.title, &input.content)?;
    validate_block_type(&input.block_type)?;
    ensure_block_capacity(&state, &vault.id, input.content.len() as i64, true).await?;
    let position = input.position.unwrap_or(sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(position), -1) + 1 FROM blocks WHERE vault_id = ?").bind(&vault.id).fetch_one(&state.pool).await?);
    let block = Block { id: Uuid::new_v4().to_string(), vault_id: vault.id.clone(), block_type: input.block_type, title: input.title.trim().into(), content: input.content, position, version: 1, created_at: Utc::now().to_rfc3339(), updated_at: Utc::now().to_rfc3339() };
    sqlx::query("INSERT INTO blocks (id, vault_id, block_type, title, content, position, version, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)")
        .bind(&block.id).bind(&block.vault_id).bind(&block.block_type).bind(&block.title).bind(&block.content).bind(block.position).bind(&block.created_at).bind(&block.updated_at).execute(&state.pool).await?;
    db::revision(&state.pool, &vault.id, "block", Some(&block.id), "create", Option::<&Block>::None, Some(&block), &clean_source(&source.source)).await?;
    after_content_change(&state, &vault.id).await?;
    Ok((StatusCode::CREATED, Json(block)))
}

#[derive(Debug, Deserialize)]
pub struct UpdateBlockInput {
    pub block_type: Option<String>,
    pub title: String,
    pub content: String,
    pub position: Option<i64>,
    pub version: i64,
}

pub async fn update_block(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Query(source): Query<SourceQuery>, Json(input): Json<UpdateBlockInput>) -> AppResult<Json<Block>> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    validate_block(&input.title, &input.content)?;
    let before = block_for(&state, &vault.id, &id).await?;
    let block_type = input.block_type.as_deref().unwrap_or(&before.block_type);
    validate_block_type(block_type)?;
    ensure_block_capacity(&state, &vault.id, input.content.len() as i64 - before.content.len() as i64, false).await?;
    let result = sqlx::query("UPDATE blocks SET block_type = ?, title = ?, content = ?, position = COALESCE(?, position), version = version + 1, updated_at = ? WHERE id = ? AND vault_id = ? AND version = ?")
        .bind(block_type).bind(input.title.trim()).bind(&input.content).bind(input.position).bind(Utc::now().to_rfc3339()).bind(&id).bind(&vault.id).bind(input.version).execute(&state.pool).await?;
    if result.rows_affected() == 0 { return Err(AppError::Conflict); }
    let after = block_for(&state, &vault.id, &id).await?;
    db::revision(&state.pool, &vault.id, "block", Some(&id), "update", Some(&before), Some(&after), &clean_source(&source.source)).await?;
    after_content_change(&state, &vault.id).await?;
    Ok(Json(after))
}

#[derive(Debug, Deserialize)]
pub struct VersionInput { pub version: i64 }

pub async fn delete_block(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Query(source): Query<SourceQuery>, Json(input): Json<VersionInput>) -> AppResult<StatusCode> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    let before = block_for(&state, &vault.id, &id).await?;
    let result = sqlx::query("DELETE FROM blocks WHERE id = ? AND vault_id = ? AND version = ?")
        .bind(&id).bind(&vault.id).bind(input.version).execute(&state.pool).await?;
    if result.rows_affected() == 0 { return Err(AppError::Conflict); }
    // Drop deleted block references from all bundles.
    let rows = sqlx::query_as::<_, BundleRow>("SELECT * FROM bundles WHERE vault_id = ?").bind(&vault.id).fetch_all(&state.pool).await?;
    for row in rows {
        let mut bundle = Bundle::try_from(row).map_err(anyhow::Error::from)?;
        let old_len = bundle.block_ids.len();
        bundle.block_ids.retain(|block_id| block_id != &id);
        if bundle.block_ids.len() != old_len {
            sqlx::query("UPDATE bundles SET block_ids = ?, version = version + 1, updated_at = ? WHERE id = ?")
                .bind(serde_json::to_string(&bundle.block_ids).unwrap()).bind(Utc::now().to_rfc3339()).bind(&bundle.id).execute(&state.pool).await?;
        }
    }
    db::revision(&state.pool, &vault.id, "block", Some(&id), "delete", Some(&before), Option::<&Block>::None, &clean_source(&source.source)).await?;
    after_content_change(&state, &vault.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct ReorderBlocksInput { pub block_ids: Vec<String> }

pub async fn reorder_blocks(State(state): State<AppState>, headers: HeaderMap, Json(input): Json<ReorderBlocksInput>) -> AppResult<StatusCode> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    let existing = sqlx::query_scalar::<_, String>("SELECT id FROM blocks WHERE vault_id = ? ORDER BY position")
        .bind(&vault.id).fetch_all(&state.pool).await?;
    let mut a = existing.clone(); let mut b = input.block_ids.clone(); a.sort(); b.sort();
    if a != b { return Err(AppError::bad("block_ids must contain every block exactly once")); }
    let mut tx = state.pool.begin().await?;
    for (position, id) in input.block_ids.iter().enumerate() {
        sqlx::query("UPDATE blocks SET position = ?, version = version + 1, updated_at = ? WHERE id = ? AND vault_id = ?")
            .bind(position as i64).bind(Utc::now().to_rfc3339()).bind(id).bind(&vault.id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    db::mark_used(&state.pool, &vault.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_bundles(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Vec<Bundle>>> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    let rows = sqlx::query_as::<_, BundleRow>("SELECT * FROM bundles WHERE vault_id = ? ORDER BY created_at").bind(vault.id).fetch_all(&state.pool).await?;
    Ok(Json(rows.into_iter().map(Bundle::try_from).collect::<Result<_, _>>().map_err(anyhow::Error::from)?))
}

#[derive(Debug, Deserialize)]
pub struct CreateBundleInput { pub name: String, pub block_ids: Vec<String> }

pub async fn create_bundle(State(state): State<AppState>, headers: HeaderMap, Query(source): Query<SourceQuery>, Json(input): Json<CreateBundleInput>) -> AppResult<(StatusCode, Json<Bundle>)> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM bundles WHERE vault_id = ?").bind(&vault.id).fetch_one(&state.pool).await?;
    if count >= 20 { return Err(AppError::bad("vault has reached the 20 bundle limit")); }
    validate_bundle(&state, &vault.id, &input.name, &input.block_ids).await?;
    let now = Utc::now().to_rfc3339();
    let bundle = Bundle { id: Uuid::new_v4().to_string(), vault_id: vault.id.clone(), name: input.name.trim().into(), block_ids: input.block_ids, version: 1, created_at: now.clone(), updated_at: now };
    sqlx::query("INSERT INTO bundles (id, vault_id, name, block_ids, version, created_at, updated_at) VALUES (?, ?, ?, ?, 1, ?, ?)")
        .bind(&bundle.id).bind(&bundle.vault_id).bind(&bundle.name).bind(serde_json::to_string(&bundle.block_ids).unwrap()).bind(&bundle.created_at).bind(&bundle.updated_at).execute(&state.pool).await?;
    db::revision(&state.pool, &vault.id, "bundle", Some(&bundle.id), "create", Option::<&Bundle>::None, Some(&bundle), &clean_source(&source.source)).await?;
    after_content_change(&state, &vault.id).await?;
    Ok((StatusCode::CREATED, Json(bundle)))
}

#[derive(Debug, Deserialize)]
pub struct UpdateBundleInput { pub name: String, pub block_ids: Vec<String>, pub version: i64 }

pub async fn update_bundle(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Query(source): Query<SourceQuery>, Json(input): Json<UpdateBundleInput>) -> AppResult<Json<Bundle>> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    validate_bundle(&state, &vault.id, &input.name, &input.block_ids).await?;
    let before = bundle_for(&state, &vault.id, &id).await?;
    let result = sqlx::query("UPDATE bundles SET name = ?, block_ids = ?, version = version + 1, updated_at = ? WHERE id = ? AND vault_id = ? AND version = ?")
        .bind(input.name.trim()).bind(serde_json::to_string(&input.block_ids).unwrap()).bind(Utc::now().to_rfc3339()).bind(&id).bind(&vault.id).bind(input.version).execute(&state.pool).await?;
    if result.rows_affected() == 0 { return Err(AppError::Conflict); }
    let after = bundle_for(&state, &vault.id, &id).await?;
    db::revision(&state.pool, &vault.id, "bundle", Some(&id), "update", Some(&before), Some(&after), &clean_source(&source.source)).await?;
    after_content_change(&state, &vault.id).await?;
    Ok(Json(after))
}

pub async fn delete_bundle(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<String>, Query(source): Query<SourceQuery>, Json(input): Json<VersionInput>) -> AppResult<StatusCode> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    let before = bundle_for(&state, &vault.id, &id).await?;
    let result = sqlx::query("DELETE FROM bundles WHERE id = ? AND vault_id = ? AND version = ?")
        .bind(&id).bind(&vault.id).bind(input.version).execute(&state.pool).await?;
    if result.rows_affected() == 0 { return Err(AppError::Conflict); }
    db::revision(&state.pool, &vault.id, "bundle", Some(&id), "delete", Some(&before), Option::<&Bundle>::None, &clean_source(&source.source)).await?;
    after_content_change(&state, &vault.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct RevisionQuery { pub limit: Option<i64> }

pub async fn list_revisions(State(state): State<AppState>, headers: HeaderMap, Query(query): Query<RevisionQuery>) -> AppResult<Json<Vec<Revision>>> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    Ok(Json(sqlx::query_as::<_, Revision>("SELECT * FROM revisions WHERE vault_id = ? ORDER BY id DESC LIMIT ?")
        .bind(vault.id).bind(limit).fetch_all(&state.pool).await?))
}

pub async fn restore_revision(State(state): State<AppState>, headers: HeaderMap, Path(id): Path<i64>) -> AppResult<Json<Value>> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    let revision = sqlx::query_as::<_, Revision>("SELECT * FROM revisions WHERE id = ? AND vault_id = ?")
        .bind(id).bind(&vault.id).fetch_optional(&state.pool).await?.ok_or(AppError::NotFound)?;
    match revision.resource_type.as_str() {
        "block" => restore_block_revision(&state, &vault.id, &revision).await?,
        "bundle" => restore_bundle_revision(&state, &vault.id, &revision).await?,
        _ => return Err(AppError::bad("only block and bundle revisions can be restored")),
    }
    after_content_change(&state, &vault.id).await?;
    Ok(Json(json!({ "restored": true, "revision_id": id })))
}

#[derive(Debug, Deserialize)]
pub struct NotificationTargetInput { pub kind: String, pub url: String, #[serde(default)] pub headers: std::collections::BTreeMap<String, String> }

pub async fn get_notification_target(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    let target = notifications::load_target(&state, &vault.id).await.map_err(anyhow::Error::from)?;
    Ok(Json(match target { Some((stored, config)) => serde_json::to_value(notifications::view(&stored, &config)).unwrap(), None => Value::Null }))
}

pub async fn put_notification_target(State(state): State<AppState>, headers: HeaderMap, Json(input): Json<NotificationTargetInput>) -> AppResult<Json<Value>> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    let config = NotificationConfig { url: input.url, headers: input.headers };
    notifications::validate_target(&input.kind, &config).await.map_err(|error| AppError::bad(error.to_string()))?;
    let encrypted = encrypt_config(&state.config.master_key, &config).map_err(AppError::Internal)?;
    let now = Utc::now().to_rfc3339();
    sqlx::query("INSERT INTO notification_targets (vault_id, kind, encrypted_config, created_at, updated_at) VALUES (?, ?, ?, ?, ?) ON CONFLICT(vault_id) DO UPDATE SET kind = excluded.kind, encrypted_config = excluded.encrypted_config, updated_at = excluded.updated_at")
        .bind(&vault.id).bind(&input.kind).bind(encrypted).bind(&now).bind(&now).execute(&state.pool).await?;
    db::mark_used(&state.pool, &vault.id).await?;
    Ok(Json(json!({ "kind": input.kind, "masked_url": mask_url(&config.url), "updated_at": now })))
}

pub async fn delete_notification_target(State(state): State<AppState>, headers: HeaderMap) -> AppResult<StatusCode> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    sqlx::query("DELETE FROM notification_targets WHERE vault_id = ?").bind(vault.id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn test_notification_target(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    let vault = user_vault(&state.pool, &headers, &state.limits).await?;
    let (stored, config) = notifications::load_target(&state, &vault.id).await.map_err(AppError::Internal)?.ok_or_else(|| AppError::bad("notification target is not configured"))?;
    let payload = CallbackPayload { status: "completed".into(), title: "CrossPrompt test".into(), message: "Your notification target is connected.".into(), source: Some("CrossPrompt".into()), url: Some(format!("{}/#/v", state.config.public_base_url)) };
    let status = notifications::send(&state, &vault.id, &stored, &config, &payload).await.map_err(|_| AppError::Upstream)?;
    Ok(Json(json!({ "delivered": true, "status_code": status.as_u16() })))
}

pub async fn callback(State(state): State<AppState>, Path(secret): Path<String>, Json(payload): Json<CallbackPayload>) -> AppResult<Json<Value>> {
    validate_callback(&payload)?;
    let vault = sqlx::query_as::<_, Vault>("SELECT * FROM vaults WHERE secret_hash = ?")
        .bind(digest(&secret)).fetch_optional(&state.pool).await?.ok_or(AppError::NotFound)?;
    if vault.status == "suspended" { return Err(AppError::Locked); }
    if vault.status == "deleted" { return Err(AppError::Gone); }
    if !state.limits.check(format!("callback-minute:{}", vault.id), 10, Duration::from_secs(60))
        || !state.limits.check(format!("callback-day:{}", vault.id), 100, Duration::from_secs(86_400)) { return Err(AppError::RateLimited); }
    let (stored, config) = notifications::load_target(&state, &vault.id).await.map_err(AppError::Internal)?.ok_or_else(|| AppError::bad("notification target is not configured"))?;
    let status = notifications::send(&state, &vault.id, &stored, &config, &payload).await.map_err(|_| AppError::Upstream)?;
    Ok(Json(json!({ "delivered": true, "status_code": status.as_u16() })))
}

pub async fn config_public(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "turnstile_site_key": state.config.turnstile_site_key,
        "turnstile_required": state.config.turnstile_secret_key.is_some(),
        "public_base_url": state.config.public_base_url,
    }))
}

pub async fn artifact_types() -> Json<&'static [artifact_types::ArtifactType]> {
    Json(artifact_types::TYPES)
}

async fn verify_turnstile(state: &AppState, token: Option<&str>, remote_ip: String) -> AppResult<()> {
    let Some(secret) = state.config.turnstile_secret_key.as_deref() else { return Ok(()); };
    let token = token.filter(|v| !v.is_empty()).ok_or_else(|| AppError::bad("Turnstile verification is required"))?;
    #[derive(Deserialize)] struct ResultBody { success: bool }
    let response: ResultBody = state.http.post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .form(&[("secret", secret), ("response", token), ("remoteip", remote_ip.as_str())])
        .send().await.map_err(anyhow::Error::from)?.json().await.map_err(anyhow::Error::from)?;
    if !response.success { return Err(AppError::bad("Turnstile verification failed")); }
    Ok(())
}

fn clean_name(value: &str) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > 100 { return Err(AppError::bad("name must be between 1 and 100 characters")); }
    Ok(value.into())
}

fn validate_block(title: &str, content: &str) -> AppResult<()> {
    clean_name(title)?;
    if content.len() > 65_536 { return Err(AppError::bad("block content exceeds 64 KiB")); }
    Ok(())
}

fn validate_block_type(block_type: &str) -> AppResult<()> {
    artifact_types::find(block_type)
        .map(|_| ())
        .ok_or_else(|| AppError::bad("unknown block_type; use GET /api/v1/artifact-types"))
}

async fn ensure_block_capacity(state: &AppState, vault_id: &str, delta: i64, is_new: bool) -> AppResult<()> {
    let (count, bytes): (i64, i64) = sqlx::query_as("SELECT COUNT(*), COALESCE(SUM(LENGTH(CAST(content AS BLOB))), 0) FROM blocks WHERE vault_id = ?")
        .bind(vault_id).fetch_one(&state.pool).await?;
    if is_new && count >= 100 { return Err(AppError::bad("vault has reached the 100 block limit")); }
    if bytes + delta > 1_048_576 { return Err(AppError::bad("vault content exceeds 1 MiB")); }
    Ok(())
}

async fn block_for(state: &AppState, vault_id: &str, id: &str) -> AppResult<Block> {
    sqlx::query_as::<_, Block>("SELECT * FROM blocks WHERE id = ? AND vault_id = ?").bind(id).bind(vault_id).fetch_optional(&state.pool).await?.ok_or(AppError::NotFound)
}

async fn bundle_for(state: &AppState, vault_id: &str, id: &str) -> AppResult<Bundle> {
    let row = sqlx::query_as::<_, BundleRow>("SELECT * FROM bundles WHERE id = ? AND vault_id = ?").bind(id).bind(vault_id).fetch_optional(&state.pool).await?.ok_or(AppError::NotFound)?;
    Bundle::try_from(row).map_err(anyhow::Error::from).map_err(AppError::Internal)
}

async fn validate_bundle(state: &AppState, vault_id: &str, name: &str, block_ids: &[String]) -> AppResult<()> {
    clean_name(name)?;
    let mut unique = block_ids.to_vec(); unique.sort(); unique.dedup();
    if unique.len() != block_ids.len() { return Err(AppError::bad("bundle cannot contain duplicate block IDs")); }
    if !block_ids.is_empty() {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM blocks WHERE vault_id = ? AND id IN (SELECT value FROM json_each(?))")
            .bind(vault_id).bind(serde_json::to_string(block_ids).unwrap()).fetch_one(&state.pool).await?;
        if count as usize != block_ids.len() { return Err(AppError::bad("bundle includes an unknown block")); }
    }
    Ok(())
}

async fn after_content_change(state: &AppState, vault_id: &str) -> AppResult<()> {
    db::mark_used(&state.pool, vault_id).await?;
    db::prune_revisions(&state.pool, vault_id).await
}

fn validate_callback(payload: &CallbackPayload) -> AppResult<()> {
    if !matches!(payload.status.as_str(), "completed" | "needs_input" | "failed") { return Err(AppError::bad("invalid callback status")); }
    if payload.title.trim().is_empty() || payload.title.chars().count() > 250 || payload.message.trim().is_empty() || payload.message.chars().count() > 1024 { return Err(AppError::bad("invalid callback title or message length")); }
    if let Some(url) = &payload.url { let parsed = url::Url::parse(url).map_err(|_| AppError::bad("invalid result URL"))?; if !matches!(parsed.scheme(), "http" | "https") { return Err(AppError::bad("result URL must use http or https")); } }
    Ok(())
}

async fn restore_block_revision(state: &AppState, vault_id: &str, revision: &Revision) -> AppResult<()> {
    let before = revision.before_json.as_deref().map(serde_json::from_str::<Block>).transpose().map_err(anyhow::Error::from)?;
    let current = match revision.resource_id.as_deref() { Some(id) => block_for(state, vault_id, id).await.ok(), None => None };
    match before {
        Some(mut block) => {
            block.vault_id = vault_id.into(); block.version = current.as_ref().map(|b| b.version + 1).unwrap_or(block.version + 1); block.updated_at = Utc::now().to_rfc3339();
            validate_block_type(&block.block_type)?;
            sqlx::query("INSERT INTO blocks (id, vault_id, block_type, title, content, position, version, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET block_type = excluded.block_type, title = excluded.title, content = excluded.content, position = excluded.position, version = excluded.version, updated_at = excluded.updated_at")
                .bind(&block.id).bind(vault_id).bind(&block.block_type).bind(&block.title).bind(&block.content).bind(block.position).bind(block.version).bind(&block.created_at).bind(&block.updated_at).execute(&state.pool).await?;
            db::revision(&state.pool, vault_id, "block", Some(&block.id), "restore", current.as_ref(), Some(&block), "restore").await?;
        }
        None => if let Some(current) = current { sqlx::query("DELETE FROM blocks WHERE id = ? AND vault_id = ?").bind(&current.id).bind(vault_id).execute(&state.pool).await?; db::revision(&state.pool, vault_id, "block", Some(&current.id), "restore", Some(&current), Option::<&Block>::None, "restore").await?; },
    }
    Ok(())
}

async fn restore_bundle_revision(state: &AppState, vault_id: &str, revision: &Revision) -> AppResult<()> {
    let before = revision.before_json.as_deref().map(serde_json::from_str::<Bundle>).transpose().map_err(anyhow::Error::from)?;
    let current = match revision.resource_id.as_deref() { Some(id) => bundle_for(state, vault_id, id).await.ok(), None => None };
    match before {
        Some(mut bundle) => {
            bundle.vault_id = vault_id.into(); bundle.version = current.as_ref().map(|b| b.version + 1).unwrap_or(bundle.version + 1); bundle.updated_at = Utc::now().to_rfc3339();
            validate_bundle(state, vault_id, &bundle.name, &bundle.block_ids).await?;
            sqlx::query("INSERT INTO bundles (id, vault_id, name, block_ids, version, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET name = excluded.name, block_ids = excluded.block_ids, version = excluded.version, updated_at = excluded.updated_at")
                .bind(&bundle.id).bind(vault_id).bind(&bundle.name).bind(serde_json::to_string(&bundle.block_ids).unwrap()).bind(bundle.version).bind(&bundle.created_at).bind(&bundle.updated_at).execute(&state.pool).await?;
            db::revision(&state.pool, vault_id, "bundle", Some(&bundle.id), "restore", current.as_ref(), Some(&bundle), "restore").await?;
        }
        None => if let Some(current) = current { sqlx::query("DELETE FROM bundles WHERE id = ? AND vault_id = ?").bind(&current.id).bind(vault_id).execute(&state.pool).await?; db::revision(&state.pool, vault_id, "bundle", Some(&current.id), "restore", Some(&current), Option::<&Bundle>::None, "restore").await?; },
    }
    Ok(())
}
