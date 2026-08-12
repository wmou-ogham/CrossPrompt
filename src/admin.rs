use std::{net::SocketAddr, time::Duration};

use axum::{
    extract::{ConnectInfo, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, QueryBuilder, Sqlite};

use crate::{
    auth::{admin_cookie, admin_mutation, admin_session, clear_admin_cookie, ip_hash, verify_admin_password},
    error::{AppError, AppResult},
    models::{Block, Bundle, BundleRow, Revision, Vault},
    notifications,
    security::{digest, keyed_digest, new_secret},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct LoginInput { pub username: String, pub password: String }

pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(input): Json<LoginInput>,
) -> AppResult<impl IntoResponse> {
    let ip = ip_hash(&state.config, &headers, peer);
    let key = format!("admin-login:{ip}");
    if !state.limits.check(key.clone(), 5, Duration::from_secs(900)) {
        return Err(AppError::RateLimited);
    }
    if !verify_admin_password(&state.config, &input.username, &input.password) {
        return Err(AppError::Unauthorized);
    }
    state.limits.clear(&key);
    let token = new_secret();
    let csrf = new_secret();
    let now = Utc::now();
    sqlx::query("INSERT INTO admin_sessions (token_digest, csrf_digest, created_at, expires_at) VALUES (?, ?, ?, ?)")
        .bind(keyed_digest(&state.config.session_secret, &token))
        .bind(digest(&csrf))
        .bind(now.to_rfc3339())
        .bind((now + ChronoDuration::hours(12)).to_rfc3339())
        .execute(&state.pool)
        .await?;
    let mut response = Json(json!({ "csrf_token": csrf, "expires_in": 43_200 })).into_response();
    response.headers_mut().insert(header::SET_COOKIE, HeaderValue::from_str(&admin_cookie(&state.config, &token)).map_err(anyhow::Error::from)?);
    Ok(response)
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> AppResult<impl IntoResponse> {
    admin_mutation(&state.config, &state.pool, &headers).await?;
    if let Some(token) = cookie(&headers, "crossprompt_admin") {
        sqlx::query("DELETE FROM admin_sessions WHERE token_digest = ?").bind(keyed_digest(&state.config.session_secret, &token)).execute(&state.pool).await?;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(header::SET_COOKIE, HeaderValue::from_str(&clear_admin_cookie(&state.config)).map_err(anyhow::Error::from)?);
    Ok(response)
}

pub async fn session_info(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    admin_session(&state.config, &state.pool, &headers).await?;
    Ok(Json(json!({ "authenticated": true })))
}

pub async fn overview(State(state): State<AppState>, headers: HeaderMap) -> AppResult<Json<Value>> {
    admin_session(&state.config, &state.pool, &headers).await?;
    let counts: (i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(status = 'active'), 0), COALESCE(SUM(status = 'suspended'), 0), COALESCE(SUM(status = 'deleted'), 0), COALESCE(SUM(ever_used = 0), 0), COALESCE(SUM(created_at >= ?), 0), COALESCE(SUM(created_at >= ?), 0) FROM vaults",
    ).bind((Utc::now() - ChronoDuration::days(1)).to_rfc3339()).bind((Utc::now() - ChronoDuration::days(30)).to_rfc3339()).fetch_one(&state.pool).await?;
    let object_counts: (i64, i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM blocks), (SELECT COUNT(*) FROM bundles), (SELECT COUNT(*) FROM revisions)",
    ).fetch_one(&state.pool).await?;
    let bytes: (i64, i64, i64) = sqlx::query_as(
        "SELECT COALESCE((SELECT SUM(LENGTH(CAST(content AS BLOB))) FROM blocks), 0), COALESCE((SELECT SUM(LENGTH(CAST(block_ids AS BLOB))) FROM bundles), 0), COALESCE((SELECT SUM(LENGTH(CAST(COALESCE(before_json, '') || COALESCE(after_json, '') AS BLOB))) FROM revisions), 0)",
    ).fetch_one(&state.pool).await?;
    let deliveries: (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(success = 1), 0), COALESCE(SUM(success = 0), 0) FROM webhook_deliveries WHERE created_at >= ?",
    ).bind((Utc::now() - ChronoDuration::days(30)).to_rfc3339()).fetch_one(&state.pool).await?;
    let largest = sqlx::query_as::<_, VaultSummary>(&format!("{SUMMARY_QUERY} ORDER BY content_bytes DESC LIMIT 10"))
        .fetch_all(&state.pool).await?
        .into_iter().collect::<Vec<_>>();
    let file_bytes = database_file_bytes(&state.config.database_path);
    Ok(Json(json!({
        "vaults": { "total": counts.0, "active": counts.1, "suspended": counts.2, "deleted": counts.3, "never_used": counts.4, "created_24h": counts.5, "created_30d": counts.6 },
        "objects": { "blocks": object_counts.0, "bundles": object_counts.1, "revisions": object_counts.2 },
        "storage": { "database_file_bytes": file_bytes, "block_bytes": bytes.0, "bundle_bytes": bytes.1, "revision_bytes": bytes.2 },
        "webhooks_30d": { "success": deliveries.0, "failed": deliveries.1 },
        "largest_vaults": largest,
    })))
}

const SUMMARY_QUERY: &str = r#"
SELECT v.id, v.name, v.status, v.ever_used, v.created_at, v.updated_at, v.deleted_at,
       (SELECT COUNT(*) FROM blocks b WHERE b.vault_id = v.id) AS block_count,
       (SELECT COUNT(*) FROM bundles bu WHERE bu.vault_id = v.id) AS bundle_count,
       (SELECT COUNT(*) FROM revisions r WHERE r.vault_id = v.id) AS revision_count,
       (SELECT COALESCE(SUM(LENGTH(CAST(content AS BLOB))), 0) FROM blocks b WHERE b.vault_id = v.id) AS content_bytes
FROM vaults v
"#;

#[derive(Debug, Serialize, FromRow)]
pub struct VaultSummary {
    pub id: String,
    pub name: String,
    pub status: String,
    pub ever_used: bool,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub block_count: i64,
    pub bundle_count: i64,
    pub revision_count: i64,
    pub content_bytes: i64,
}

#[derive(Debug, Deserialize)]
pub struct VaultListQuery {
    pub q: Option<String>,
    pub status: Option<String>,
    pub sort: Option<String>,
    pub page: Option<i64>,
}

pub async fn list_vaults(State(state): State<AppState>, headers: HeaderMap, Query(query): Query<VaultListQuery>) -> AppResult<Json<Value>> {
    admin_session(&state.config, &state.pool, &headers).await?;
    let page = query.page.unwrap_or(1).max(1);
    let mut builder: QueryBuilder<Sqlite> = QueryBuilder::new(SUMMARY_QUERY);
    let mut has_where = false;
    if let Some(status) = query.status.as_deref().filter(|s| matches!(*s, "active" | "suspended" | "deleted" | "empty")) {
        builder.push(" WHERE "); has_where = true;
        if status == "empty" { builder.push("v.ever_used = 0"); } else { builder.push("v.status = ").push_bind(status.to_owned()); }
    }
    if let Some(term) = query.q.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        builder.push(if has_where { " AND " } else { " WHERE " });
        builder.push("(v.id LIKE ").push_bind(format!("%{term}%")).push(" OR v.name LIKE ").push_bind(format!("%{term}%")).push(")");
    }
    builder.push(" ORDER BY ");
    match query.sort.as_deref() {
        Some("created_asc") => builder.push("v.created_at ASC"),
        Some("updated_asc") => builder.push("v.updated_at ASC"),
        Some("size_desc") => builder.push("content_bytes DESC"),
        _ => builder.push("v.updated_at DESC"),
    };
    builder.push(" LIMIT 50 OFFSET ").push_bind((page - 1) * 50);
    let items = builder.build_query_as::<VaultSummary>().fetch_all(&state.pool).await?;
    Ok(Json(json!({ "page": page, "page_size": 50, "items": items })))
}

pub async fn get_vault(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Path(id): Path<String>) -> AppResult<Json<Value>> {
    admin_session(&state.config, &state.pool, &headers).await?;
    let vault = sqlx::query_as::<_, Vault>("SELECT * FROM vaults WHERE id = ?").bind(&id).fetch_optional(&state.pool).await?.ok_or(AppError::NotFound)?;
    let blocks = sqlx::query_as::<_, Block>("SELECT * FROM blocks WHERE vault_id = ? ORDER BY position").bind(&id).fetch_all(&state.pool).await?;
    let rows = sqlx::query_as::<_, BundleRow>("SELECT * FROM bundles WHERE vault_id = ? ORDER BY created_at").bind(&id).fetch_all(&state.pool).await?;
    let bundles: Vec<Bundle> = rows.into_iter().map(Bundle::try_from).collect::<Result<_, _>>().map_err(anyhow::Error::from)?;
    let revisions = sqlx::query_as::<_, Revision>("SELECT * FROM revisions WHERE vault_id = ? ORDER BY id DESC LIMIT 100").bind(&id).fetch_all(&state.pool).await?;
    let target = notifications::load_target(&state, &id).await.map_err(anyhow::Error::from)?.map(|(stored, config)| notifications::view(&stored, &config));
    audit(&state, "view_content", Some(&id), None, &ip_hash(&state.config, &headers, peer)).await?;
    Ok(Json(json!({ "vault": vault, "blocks": blocks, "bundles": bundles, "revisions": revisions, "notification_target": target })))
}

#[derive(Debug, Deserialize)]
pub struct ReasonInput { pub reason: Option<String> }

pub async fn suspend(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Path(id): Path<String>, Json(input): Json<ReasonInput>) -> AppResult<StatusCode> {
    admin_mutation(&state.config, &state.pool, &headers).await?;
    let result = sqlx::query("UPDATE vaults SET status = 'suspended', suspended_reason = ?, updated_at = ? WHERE id = ? AND status != 'deleted'")
        .bind(input.reason.as_deref().map(str::trim)).bind(Utc::now().to_rfc3339()).bind(&id).execute(&state.pool).await?;
    if result.rows_affected() == 0 { return Err(AppError::NotFound); }
    audit(&state, "suspend", Some(&id), input.reason.as_deref(), &ip_hash(&state.config, &headers, peer)).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn resume(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Path(id): Path<String>, Json(input): Json<ReasonInput>) -> AppResult<StatusCode> {
    admin_mutation(&state.config, &state.pool, &headers).await?;
    let result = sqlx::query("UPDATE vaults SET status = 'active', suspended_reason = NULL, updated_at = ? WHERE id = ? AND status = 'suspended'")
        .bind(Utc::now().to_rfc3339()).bind(&id).execute(&state.pool).await?;
    if result.rows_affected() == 0 { return Err(AppError::NotFound); }
    audit(&state, "resume", Some(&id), input.reason.as_deref(), &ip_hash(&state.config, &headers, peer)).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn soft_delete(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Path(id): Path<String>, Json(input): Json<ReasonInput>) -> AppResult<StatusCode> {
    admin_mutation(&state.config, &state.pool, &headers).await?;
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query("UPDATE vaults SET status = 'deleted', deleted_by = 'admin', deleted_at = ?, updated_at = ? WHERE id = ?")
        .bind(&now).bind(&now).bind(&id).execute(&state.pool).await?;
    if result.rows_affected() == 0 { return Err(AppError::NotFound); }
    audit(&state, "soft_delete", Some(&id), input.reason.as_deref(), &ip_hash(&state.config, &headers, peer)).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn restore(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Path(id): Path<String>, Json(input): Json<ReasonInput>) -> AppResult<StatusCode> {
    admin_mutation(&state.config, &state.pool, &headers).await?;
    let result = sqlx::query("UPDATE vaults SET status = 'active', deleted_by = NULL, deleted_at = NULL, suspended_reason = NULL, updated_at = ? WHERE id = ? AND status = 'deleted' AND deleted_at >= ?")
        .bind(Utc::now().to_rfc3339()).bind(&id).bind((Utc::now() - ChronoDuration::days(7)).to_rfc3339()).execute(&state.pool).await?;
    if result.rows_affected() == 0 { return Err(AppError::NotFound); }
    audit(&state, "restore", Some(&id), input.reason.as_deref(), &ip_hash(&state.config, &headers, peer)).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct PermanentDeleteInput { pub confirmation: String, pub reason: Option<String> }

pub async fn permanent_delete(State(state): State<AppState>, ConnectInfo(peer): ConnectInfo<SocketAddr>, headers: HeaderMap, Path(id): Path<String>, Json(input): Json<PermanentDeleteInput>) -> AppResult<StatusCode> {
    admin_mutation(&state.config, &state.pool, &headers).await?;
    if input.confirmation != id { return Err(AppError::bad("confirmation must exactly match the Vault ID")); }
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM vaults WHERE id = ?").bind(&id).fetch_one(&state.pool).await?;
    if exists == 0 { return Err(AppError::NotFound); }
    audit(&state, "permanent_delete", Some(&id), input.reason.as_deref(), &ip_hash(&state.config, &headers, peer)).await?;
    sqlx::query("DELETE FROM vaults WHERE id = ?").bind(&id).execute(&state.pool).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct AuditQuery { pub page: Option<i64> }

#[derive(Debug, Serialize, FromRow)]
pub struct AuditLog { pub id: i64, pub action: String, pub vault_id: Option<String>, pub reason: Option<String>, pub ip_hash: String, pub created_at: String }

pub async fn audit_log(State(state): State<AppState>, headers: HeaderMap, Query(query): Query<AuditQuery>) -> AppResult<Json<Value>> {
    admin_session(&state.config, &state.pool, &headers).await?;
    let page = query.page.unwrap_or(1).max(1);
    let items = sqlx::query_as::<_, AuditLog>("SELECT * FROM admin_audit_logs ORDER BY id DESC LIMIT 100 OFFSET ?")
        .bind((page - 1) * 100).fetch_all(&state.pool).await?;
    Ok(Json(json!({ "page": page, "items": items })))
}

async fn audit(state: &AppState, action: &str, vault_id: Option<&str>, reason: Option<&str>, ip_hash: &str) -> AppResult<()> {
    sqlx::query("INSERT INTO admin_audit_logs (action, vault_id, reason, ip_hash, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(action).bind(vault_id).bind(reason.map(|v| v.trim().chars().take(500).collect::<String>())).bind(ip_hash).bind(Utc::now().to_rfc3339()).execute(&state.pool).await?;
    Ok(())
}

fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get(header::COOKIE)?.to_str().ok()?.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        (key == name).then(|| value.to_owned())
    })
}

fn database_file_bytes(path: &std::path::Path) -> u64 {
    ["", "-wal", "-shm"]
        .iter()
        .map(|suffix| std::fs::metadata(format!("{}{suffix}", path.display())).map(|m| m.len()).unwrap_or(0))
        .sum()
}
