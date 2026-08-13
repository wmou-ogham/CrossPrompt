use std::{net::SocketAddr, time::Duration};

use axum::{
    extract::{ConnectInfo, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{Duration as ChronoDuration, Utc};
use lettre::{
    message::Mailbox,
    transport::smtp::{authentication::Credentials, client::Tls},
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use rand::{distributions::Uniform, Rng};
use serde::Deserialize;
use serde_json::{json, Value};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::{
    auth::{ip_hash, user_vault},
    error::{AppError, AppResult},
    security::{keyed_digest, new_secret},
    state::AppState,
};

const OTP_TTL_MINUTES: i64 = 10;
const EMAIL_SESSION_DAYS: i64 = 30;

#[derive(Debug, Deserialize)]
pub struct RequestLoginCodeInput {
    pub email: String,
}

pub async fn request_login_code(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(input): Json<RequestLoginCodeInput>,
) -> AppResult<Json<Value>> {
    if state.config.smtp.is_none() {
        return Err(AppError::bad("Email login is not configured"));
    }
    let email = normalize_email(&input.email)?;
    limit_code_request(&state, &headers, peer, &email, "login")?;
    let vault_id = sqlx::query_scalar::<_, String>(
        "SELECT id FROM vaults WHERE email = ? AND email_verified_at IS NOT NULL AND status = 'active'",
    )
    .bind(&email)
    .fetch_optional(&state.pool)
    .await?;

    if let Some(vault_id) = vault_id {
        create_and_send_code(&state, &email, Some(&vault_id), "login").await?;
    }

    Ok(Json(json!({
        "accepted": true,
        "message": "如果此 Email 已綁定可用的 Vault，驗證碼已寄出。"
    })))
}

#[derive(Debug, Deserialize)]
pub struct VerifyLoginCodeInput {
    pub email: String,
    pub code: String,
}

pub async fn verify_login_code(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(input): Json<VerifyLoginCodeInput>,
) -> AppResult<impl IntoResponse> {
    let email = normalize_email(&input.email)?;
    if !state.limits.check(
        format!("email-verify-ip:{}", ip_hash(&state.config, &headers, peer)),
        20,
        Duration::from_secs(900),
    ) {
        return Err(AppError::RateLimited);
    }
    let vault_id = consume_code(&state, &email, &input.code, "login", None).await?;
    let vault_id = vault_id.ok_or(AppError::Unauthorized)?;
    let token = new_secret();
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO vault_email_sessions (token_digest, vault_id, email, created_at, expires_at) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(keyed_digest(&state.config.session_secret, &token))
    .bind(&vault_id)
    .bind(&email)
    .bind(now.to_rfc3339())
    .bind((now + ChronoDuration::days(EMAIL_SESSION_DAYS)).to_rfc3339())
    .execute(&state.pool)
    .await?;

    let mut response = Json(json!({ "authenticated": true, "vault_id": vault_id })).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&email_cookie(&state, &token)).map_err(anyhow::Error::from)?,
    );
    Ok(response)
}

#[derive(Debug, Deserialize)]
pub struct BindEmailInput {
    pub email: String,
}

pub async fn request_bind_code(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(input): Json<BindEmailInput>,
) -> AppResult<Json<Value>> {
    let vault = user_vault(&state, &headers).await?;
    let email = normalize_email(&input.email)?;
    limit_code_request(&state, &headers, peer, &email, "bind")?;
    let used_by_other: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM vaults WHERE email = ? AND id != ?")
            .bind(&email)
            .bind(&vault.id)
            .fetch_one(&state.pool)
            .await?;
    if used_by_other > 0 {
        return Err(AppError::Conflict);
    }
    create_and_send_code(&state, &email, Some(&vault.id), "bind").await?;
    Ok(Json(
        json!({ "accepted": true, "email": mask_email(&email) }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct VerifyBindCodeInput {
    pub email: String,
    pub code: String,
}

pub async fn verify_bind_code(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<VerifyBindCodeInput>,
) -> AppResult<Json<Value>> {
    let vault = user_vault(&state, &headers).await?;
    let email = normalize_email(&input.email)?;
    consume_code(&state, &email, &input.code, "bind", Some(&vault.id)).await?;
    let now = Utc::now().to_rfc3339();
    let mut tx = state.pool.begin().await?;
    let result = sqlx::query(
        "UPDATE vaults SET email = ?, email_verified_at = ?, ever_used = 1, updated_at = ? WHERE id = ? AND NOT EXISTS (SELECT 1 FROM vaults WHERE email = ? AND id != ?)",
    )
    .bind(&email)
    .bind(&now)
    .bind(&now)
    .bind(&vault.id)
    .bind(&email)
    .bind(&vault.id)
    .execute(&mut *tx)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::Conflict);
    }
    tx.commit().await?;
    Ok(Json(json!({ "bound": true, "email": mask_email(&email) })))
}

pub async fn unbind_email(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<StatusCode> {
    let vault = user_vault(&state, &headers).await?;
    if !crate::auth::has_bearer(&headers) {
        return Err(AppError::Forbidden);
    }
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "UPDATE vaults SET email = NULL, email_verified_at = NULL, updated_at = ? WHERE id = ?",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(&vault.id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM vault_email_sessions WHERE vault_id = ?")
        .bind(&vault.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn logout_email(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<impl IntoResponse> {
    if let Some(token) = crate::auth::cookie(&headers, "crossprompt_vault") {
        sqlx::query("DELETE FROM vault_email_sessions WHERE token_digest = ?")
            .bind(keyed_digest(&state.config.session_secret, &token))
            .execute(&state.pool)
            .await?;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&clear_email_cookie(&state)).map_err(anyhow::Error::from)?,
    );
    Ok(response)
}

fn limit_code_request(
    state: &AppState,
    headers: &HeaderMap,
    peer: SocketAddr,
    email: &str,
    purpose: &str,
) -> AppResult<()> {
    let ip = ip_hash(&state.config, headers, peer);
    let email_key = hex_digest(&keyed_digest(&state.config.session_secret, email));
    if !state.limits.check(
        format!("otp-ip:{purpose}:{ip}"),
        10,
        Duration::from_secs(3600),
    ) || !state.limits.check(
        format!("otp-email:{purpose}:{email_key}"),
        5,
        Duration::from_secs(3600),
    ) {
        return Err(AppError::RateLimited);
    }
    Ok(())
}

async fn create_and_send_code(
    state: &AppState,
    email: &str,
    vault_id: Option<&str>,
    purpose: &str,
) -> AppResult<()> {
    let code = generate_code();
    let now = Utc::now();
    let id = Uuid::new_v4().to_string();
    sqlx::query("UPDATE email_otp_challenges SET consumed_at = ? WHERE email = ? AND purpose = ? AND consumed_at IS NULL")
        .bind(now.to_rfc3339()).bind(email).bind(purpose).execute(&state.pool).await?;
    sqlx::query("INSERT INTO email_otp_challenges (id, email, vault_id, purpose, code_digest, created_at, expires_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(&id).bind(email).bind(vault_id).bind(purpose)
        .bind(keyed_digest(&state.config.session_secret, &format!("{id}:{code}")))
        .bind(now.to_rfc3339()).bind((now + ChronoDuration::minutes(OTP_TTL_MINUTES)).to_rfc3339())
        .execute(&state.pool).await?;

    if let Err(error) = send_code(state, email, &code, purpose).await {
        sqlx::query("DELETE FROM email_otp_challenges WHERE id = ?")
            .bind(&id)
            .execute(&state.pool)
            .await?;
        return Err(error);
    }
    Ok(())
}

async fn consume_code(
    state: &AppState,
    email: &str,
    code: &str,
    purpose: &str,
    expected_vault: Option<&str>,
) -> AppResult<Option<String>> {
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(AppError::Unauthorized);
    }
    let now = Utc::now().to_rfc3339();
    let mut tx = state.pool.begin().await?;
    let row = sqlx::query_as::<_, (String, Option<String>, Vec<u8>, i64)>(
        "SELECT id, vault_id, code_digest, attempts_remaining FROM email_otp_challenges WHERE email = ? AND purpose = ? AND consumed_at IS NULL AND expires_at > ? ORDER BY created_at DESC LIMIT 1",
    )
    .bind(email).bind(purpose).bind(&now).fetch_optional(&mut *tx).await?
    .ok_or(AppError::Unauthorized)?;
    if expected_vault.is_some() && row.1.as_deref() != expected_vault {
        return Err(AppError::Unauthorized);
    }
    let candidate = keyed_digest(&state.config.session_secret, &format!("{}:{code}", row.0));
    if candidate.ct_eq(&row.2).unwrap_u8() != 1 {
        let remaining = row.3 - 1;
        sqlx::query("UPDATE email_otp_challenges SET attempts_remaining = ?, consumed_at = CASE WHEN ? <= 0 THEN ? ELSE consumed_at END WHERE id = ?")
            .bind(remaining).bind(remaining).bind(&now).bind(&row.0).execute(&mut *tx).await?;
        tx.commit().await?;
        return Err(AppError::Unauthorized);
    }
    sqlx::query("UPDATE email_otp_challenges SET consumed_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&row.0)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(row.1)
}

async fn send_code(state: &AppState, email: &str, code: &str, purpose: &str) -> AppResult<()> {
    let smtp = state
        .config
        .smtp
        .as_ref()
        .ok_or_else(|| AppError::bad("Email login is not configured"))?;
    let action = if purpose == "bind" {
        "綁定 Vault Email"
    } else {
        "登入 CrossPrompt Vault"
    };
    let message = Message::builder()
        .from(smtp.from.parse::<Mailbox>().map_err(anyhow::Error::from)?)
        .to(email.parse::<Mailbox>().map_err(anyhow::Error::from)?)
        .subject(format!("CrossPrompt 驗證碼：{code}"))
        .body(format!("你的 CrossPrompt 驗證碼是：{code}\n\n用途：{action}\n有效時間：{OTP_TTL_MINUTES} 分鐘\n\n如果不是你操作，請忽略這封信。CrossPrompt 不會要求你回覆密碼或 Vault secret。"))
        .map_err(anyhow::Error::from)?;
    let mut builder = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&smtp.host)
        .port(smtp.port)
        .tls(Tls::Required(
            lettre::transport::smtp::client::TlsParameters::new(smtp.host.clone())
                .map_err(anyhow::Error::from)?,
        ));
    if let (Some(username), Some(password)) = (&smtp.username, &smtp.password) {
        builder = builder.credentials(Credentials::new(username.clone(), password.clone()));
    }
    builder.build().send(message).await.map_err(|error| {
        tracing::error!(error = %error, "verification email delivery failed");
        AppError::Upstream
    })?;
    Ok(())
}

fn normalize_email(raw: &str) -> AppResult<String> {
    let email = raw.trim().to_lowercase();
    if email.len() > 254 || email.starts_with('.') || email.ends_with('.') {
        return Err(AppError::bad("invalid email address"));
    }
    let (local, domain) = email
        .split_once('@')
        .ok_or_else(|| AppError::bad("invalid email address"))?;
    if local.is_empty()
        || domain.is_empty()
        || !domain.contains('.')
        || email.contains(char::is_whitespace)
        || email.parse::<lettre::Address>().is_err()
    {
        return Err(AppError::bad("invalid email address"));
    }
    Ok(email)
}

fn generate_code() -> String {
    let range = Uniform::new(0u8, 10u8);
    rand::thread_rng()
        .sample_iter(range)
        .take(6)
        .map(|digit| char::from(b'0' + digit))
        .collect()
}

fn mask_email(email: &str) -> String {
    let (local, domain) = email.split_once('@').unwrap_or((email, ""));
    let first = local.chars().next().unwrap_or('•');
    format!("{first}•••@{domain}")
}

fn email_cookie(state: &AppState, token: &str) -> String {
    format!(
        "crossprompt_vault={token}; Path=/; Max-Age={}; HttpOnly; SameSite=Strict{}",
        ChronoDuration::days(EMAIL_SESSION_DAYS).num_seconds(),
        if state.config.cookie_secure {
            "; Secure"
        } else {
            ""
        }
    )
}

fn clear_email_cookie(state: &AppState) -> String {
    format!(
        "crossprompt_vault=; Path=/; Max-Age=0; HttpOnly; SameSite=Strict{}",
        if state.config.cookie_secure {
            "; Secure"
        } else {
            ""
        }
    )
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn email_normalization_and_masking() {
        assert_eq!(
            normalize_email(" User@Example.COM ").unwrap(),
            "user@example.com"
        );
        assert_eq!(mask_email("user@example.com"), "u•••@example.com");
        assert!(normalize_email("not-an-email").is_err());
    }

    #[test]
    fn codes_are_six_digits() {
        for _ in 0..20 {
            let code = generate_code();
            assert_eq!(code.len(), 6);
            assert!(code.bytes().all(|byte| byte.is_ascii_digit()));
        }
    }
}
