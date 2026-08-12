use std::{net::SocketAddr, time::Duration};

use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::http::{header, HeaderMap};
use chrono::{Duration as ChronoDuration, Utc};
use sqlx::SqlitePool;
use subtle::ConstantTimeEq;

use crate::{
    config::Config,
    error::{AppError, AppResult},
    models::Vault,
    rate_limit::RateLimits,
    security::{client_ip, digest, keyed_digest, salted_digest},
};

pub async fn user_vault(pool: &SqlitePool, headers: &HeaderMap, limits: &RateLimits) -> AppResult<Vault> {
    let token = bearer(headers)?;
    let hash = digest(token);
    let vault = sqlx::query_as::<_, Vault>("SELECT * FROM vaults WHERE secret_hash = ?")
        .bind(hash)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if !limits.check(format!("vault:{}", vault.id), 120, Duration::from_secs(60)) {
        return Err(AppError::RateLimited);
    }
    match vault.status.as_str() {
        "suspended" => Err(AppError::Locked),
        "deleted" => Err(AppError::Gone),
        _ => Ok(vault),
    }
}

pub async fn user_vault_allow_deleted(pool: &SqlitePool, headers: &HeaderMap) -> AppResult<Vault> {
    let hash = digest(bearer(headers)?);
    sqlx::query_as::<_, Vault>("SELECT * FROM vaults WHERE secret_hash = ?")
        .bind(hash)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::Unauthorized)
}

pub fn verify_admin_password(config: &Config, username: &str, password: &str) -> bool {
    if username != config.admin_username {
        return false;
    }
    let Ok(hash) = PasswordHash::new(&config.admin_password_hash) else { return false; };
    Argon2::default().verify_password(password.as_bytes(), &hash).is_ok()
}

#[derive(Clone)]
pub struct AdminAuth {
    pub csrf_digest: Vec<u8>,
}

pub async fn admin_session(config: &Config, pool: &SqlitePool, headers: &HeaderMap) -> AppResult<AdminAuth> {
    let token = cookie(headers, "crossprompt_admin").ok_or(AppError::Unauthorized)?;
    let now = Utc::now().to_rfc3339();
    let row = sqlx::query_as::<_, (Vec<u8>, String)>(
        "SELECT csrf_digest, expires_at FROM admin_sessions WHERE token_digest = ? AND expires_at > ?",
    )
    .bind(keyed_digest(&config.session_secret, &token))
    .bind(now)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::Unauthorized)?;
    Ok(AdminAuth { csrf_digest: row.0 })
}

pub async fn admin_mutation(config: &Config, pool: &SqlitePool, headers: &HeaderMap) -> AppResult<AdminAuth> {
    let auth = admin_session(config, pool, headers).await?;
    let csrf = headers.get("x-csrf-token").and_then(|v| v.to_str().ok()).ok_or(AppError::Forbidden)?;
    if digest(csrf).ct_eq(&auth.csrf_digest).unwrap_u8() != 1 {
        return Err(AppError::Forbidden);
    }
    Ok(auth)
}

pub fn admin_cookie(config: &Config, token: &str) -> String {
    format!(
        "crossprompt_admin={token}; Path=/; Max-Age={}; HttpOnly; SameSite=Strict{}",
        ChronoDuration::hours(12).num_seconds(),
        if config.cookie_secure { "; Secure" } else { "" }
    )
}

pub fn clear_admin_cookie(config: &Config) -> String {
    format!(
        "crossprompt_admin=; Path=/; Max-Age=0; HttpOnly; SameSite=Strict{}",
        if config.cookie_secure { "; Secure" } else { "" }
    )
}

pub fn ip_hash(config: &Config, headers: &HeaderMap, peer: SocketAddr) -> String {
    salted_digest(&config.ip_hash_salt, &client_ip(headers, peer, config.trust_proxy).to_string())
}

fn bearer(headers: &HeaderMap) -> AppResult<&str> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
        .ok_or(AppError::Unauthorized)
}

fn cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get(header::COOKIE)?.to_str().ok()?.split(';').find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        (key == name).then(|| value.to_owned())
    })
}
