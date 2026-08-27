use std::{net::SocketAddr, time::Duration};

use axum::{
    extract::{ConnectInfo, Query, State},
    http::{header, HeaderMap, HeaderValue},
    response::{IntoResponse, Redirect, Response},
    Json,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};
use subtle::ConstantTimeEq;
use url::Url;
use uuid::Uuid;

use crate::{
    auth::{ip_hash, user_vault},
    email_auth::{issue_email_session, normalize_email},
    error::{AppError, AppResult},
    security::{keyed_digest, new_secret},
    state::AppState,
};

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO_URL: &str = "https://openidconnect.googleapis.com/v1/userinfo";

#[derive(Debug, Deserialize)]
pub struct StartQuery {
    #[serde(default = "default_purpose")]
    pub purpose: String,
}

fn default_purpose() -> String {
    "login".into()
}

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OAuthIntent {
    nonce: String,
    purpose: String,
    vault_id: Option<String>,
}

/// Begin Google OAuth for Vault login (browser redirect).
pub async fn start_login(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(query): Query<StartQuery>,
) -> AppResult<Response> {
    if query.purpose != "login" {
        return Err(AppError::bad("use POST /auth/google/start for bind"));
    }
    rate_limit_start(&state, &headers, peer)?;
    begin_oauth(&state, "login", None).await
}

/// Begin Google OAuth for binding the current Vault email.
/// Returns a JSON redirect URL. OAuth CSRF state is carried in the Google `state` param (HMAC-signed).
pub async fn start_bind(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    rate_limit_start(&state, &headers, peer)?;
    let vault = user_vault(&state, &headers).await?;
    let google = state
        .config
        .google_oauth
        .as_ref()
        .ok_or_else(|| AppError::bad("Google SSO is not configured"))?;
    let intent = OAuthIntent {
        nonce: new_secret(),
        purpose: "bind".into(),
        vault_id: Some(vault.id),
    };
    let state_token = sign_intent(&state.config, &intent);
    let authorize_url = authorize_url(google, &state.config.google_redirect_uri(), &state_token)?;
    Ok(Json(json!({ "redirect_url": authorize_url })))
}

pub async fn callback(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(query): Query<CallbackQuery>,
) -> Response {
    match complete_callback(&state, peer, &headers, query).await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(error = %error, "google oauth callback failed");
            let destination = format!(
                "{}/#/?google_sso=error",
                state.config.public_base_url
            );
            Redirect::temporary(&destination).into_response()
        }
    }
}

async fn begin_oauth(
    state: &AppState,
    purpose: &str,
    vault_id: Option<String>,
) -> AppResult<Response> {
    let google = state
        .config
        .google_oauth
        .as_ref()
        .ok_or_else(|| AppError::bad("Google SSO is not configured"))?;
    let intent = OAuthIntent {
        nonce: new_secret(),
        purpose: purpose.to_owned(),
        vault_id,
    };
    let state_token = sign_intent(&state.config, &intent);
    let authorize_url = authorize_url(google, &state.config.google_redirect_uri(), &state_token)?;
    Ok(Redirect::temporary(&authorize_url).into_response())
}

async fn complete_callback(
    state: &AppState,
    peer: SocketAddr,
    headers: &HeaderMap,
    query: CallbackQuery,
) -> AppResult<Response> {
    rate_limit_callback(state, headers, peer)?;
    if let Some(error) = query.error.as_deref() {
        tracing::warn!(google_error = error, "google oauth returned error");
        return Err(AppError::Unauthorized);
    }
    let code = query
        .code
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            tracing::warn!("google oauth callback missing code");
            AppError::Unauthorized
        })?;
    let state_param = query
        .state
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            tracing::warn!("google oauth callback missing state");
            AppError::Unauthorized
        })?;
    let intent = verify_intent(&state.config, &state_param).map_err(|error| {
        tracing::warn!(error = %error, "google oauth state verification failed");
        error
    })?;

    let google = state
        .config
        .google_oauth
        .as_ref()
        .ok_or_else(|| AppError::bad("Google SSO is not configured"))?;
    let email = exchange_google_email(state, google, &code).await?;
    let email = normalize_email(&email)?;
    tracing::info!(purpose = %intent.purpose, "google oauth identity verified");

    let (vault_id, created_secret) = match intent.purpose.as_str() {
        "login" => login_or_create_vault(state, headers, peer, &email).await?,
        "bind" => {
            let vault_id = intent.vault_id.ok_or(AppError::Unauthorized)?;
            let status = sqlx::query_scalar::<_, String>("SELECT status FROM vaults WHERE id = ?")
                .bind(&vault_id)
                .fetch_optional(&state.pool)
                .await?
                .ok_or(AppError::Unauthorized)?;
            if status != "active" {
                return Err(AppError::Unauthorized);
            }
            let now = Utc::now().to_rfc3339();
            let mut tx = state.pool.begin().await?;
            let result = sqlx::query(
                "UPDATE vaults SET email = ?, email_verified_at = ?, ever_used = 1, updated_at = ? WHERE id = ? AND NOT EXISTS (SELECT 1 FROM vaults WHERE email = ? AND id != ?)",
            )
            .bind(&email)
            .bind(&now)
            .bind(&now)
            .bind(&vault_id)
            .bind(&email)
            .bind(&vault_id)
            .execute(&mut *tx)
            .await?;
            if result.rows_affected() == 0 {
                return Err(AppError::Conflict);
            }
            // Binding a new email invalidates prior email sessions for this vault.
            sqlx::query("DELETE FROM vault_email_sessions WHERE vault_id = ?")
                .bind(&vault_id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            (vault_id, None)
        }
        _ => return Err(AppError::bad("invalid oauth purpose")),
    };

    let (_token, set_cookie) = issue_email_session(state, &vault_id, &email).await?;
    // First-time Google sign-in returns the manage URL once so the secret link can be saved.
    let destination = if let Some(secret) = created_secret {
        format!("{}/#/v/{}", state.config.public_base_url, secret)
    } else {
        format!("{}/#/email-vault", state.config.public_base_url)
    };
    let mut response = Redirect::temporary(&destination).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&set_cookie).map_err(anyhow::Error::from)?,
    );
    Ok(response)
}

async fn login_or_create_vault(
    state: &AppState,
    headers: &HeaderMap,
    peer: SocketAddr,
    email: &str,
) -> AppResult<(String, Option<String>)> {
    let existing = sqlx::query_as::<_, (String, String)>(
        "SELECT id, status FROM vaults WHERE email = ? AND email_verified_at IS NOT NULL ORDER BY CASE status WHEN 'active' THEN 0 WHEN 'suspended' THEN 1 ELSE 2 END LIMIT 1",
    )
    .bind(email)
    .fetch_optional(&state.pool)
    .await?;

    match existing.as_ref().map(|(id, status)| (id.as_str(), status.as_str())) {
        Some((id, "active")) => Ok((id.to_owned(), None)),
        Some((_, "suspended")) => Err(AppError::Locked),
        Some((_, "deleted")) | None => {
            // Free the unique email index if a soft-deleted vault still holds it.
            if matches!(existing.as_ref().map(|(_, status)| status.as_str()), Some("deleted")) {
                sqlx::query(
                    "UPDATE vaults SET email = NULL, email_verified_at = NULL, updated_at = ? WHERE email = ? AND status = 'deleted'",
                )
                .bind(Utc::now().to_rfc3339())
                .bind(email)
                .execute(&state.pool)
                .await?;
            }
            create_vault_for_google(state, headers, peer, email).await
        }
        Some(_) => Err(AppError::Unauthorized),
    }
}

async fn create_vault_for_google(
    state: &AppState,
    headers: &HeaderMap,
    peer: SocketAddr,
    email: &str,
) -> AppResult<(String, Option<String>)> {
    use crate::security::{client_ip, digest, salted_digest};

    let request_ip = client_ip(headers, peer, state.config.trust_proxy);
    let ip_hash = salted_digest(&state.config.ip_hash_salt, &request_ip.to_string());
    let bucket = Utc::now().format("%Y-%m-%d").to_string();
    let mut tx = state.pool.begin().await?;
    let count: i64 =
        sqlx::query_scalar("SELECT count FROM creation_limits WHERE ip_hash = ? AND bucket = ?")
            .bind(&ip_hash)
            .bind(&bucket)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or(0);
    if count >= 100 {
        return Err(AppError::RateLimited);
    }
    sqlx::query(
        "INSERT INTO creation_limits (ip_hash, bucket, count) VALUES (?, ?, 1) ON CONFLICT(ip_hash, bucket) DO UPDATE SET count = count + 1",
    )
    .bind(&ip_hash)
    .bind(&bucket)
    .execute(&mut *tx)
    .await?;

    let secret = new_secret();
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let name = google_vault_name(email);
    let result = sqlx::query(
        "INSERT INTO vaults (id, secret_hash, name, email, email_verified_at, ever_used, created_at, updated_at) VALUES (?, ?, ?, ?, ?, 1, ?, ?)",
    )
    .bind(&id)
    .bind(digest(&secret))
    .bind(&name)
    .bind(email)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await;
    match result {
        Ok(_) => {
            tx.commit().await?;
            Ok((id, Some(secret)))
        }
        Err(error) if is_unique_constraint(&error) => {
            tx.rollback().await.ok();
            // Race: another request bound this email first.
            let vault_id = sqlx::query_scalar::<_, String>(
                "SELECT id FROM vaults WHERE email = ? AND email_verified_at IS NOT NULL AND status = 'active'",
            )
            .bind(email)
            .fetch_optional(&state.pool)
            .await?
            .ok_or(AppError::Conflict)?;
            Ok((vault_id, None))
        }
        Err(error) => Err(error.into()),
    }
}

fn google_vault_name(email: &str) -> String {
    let local = email.split('@').next().unwrap_or("google");
    let trimmed: String = local.chars().take(80).collect();
    if trimmed.is_empty() {
        "My CrossPrompt".into()
    } else {
        format!("{trimmed}'s CrossPrompt")
    }
}

fn is_unique_constraint(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db) => {
            let message = db.message().to_ascii_lowercase();
            message.contains("unique") || db.code().as_deref() == Some("2067")
        }
        _ => false,
    }
}

async fn exchange_google_email(
    state: &AppState,
    google: &crate::config::GoogleOAuthConfig,
    code: &str,
) -> AppResult<String> {
    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
        token_type: String,
    }
    #[derive(Deserialize)]
    struct UserInfo {
        email: Option<String>,
        email_verified: Option<bool>,
        verified_email: Option<bool>,
    }

    let redirect_uri = state.config.google_redirect_uri();
    let token_response = state
        .http
        .post(GOOGLE_TOKEN_URL)
        .form(&[
            ("code", code),
            ("client_id", google.client_id.as_str()),
            ("client_secret", google.client_secret.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "google token exchange failed");
            AppError::Internal(anyhow::anyhow!("google token exchange failed"))
        })?;
    if !token_response.status().is_success() {
        let status = token_response.status();
        let body = token_response.text().await.unwrap_or_default();
        tracing::warn!(%status, body = %body.chars().take(300).collect::<String>(), "google token endpoint rejected code");
        return Err(AppError::Unauthorized);
    }
    let token: TokenResponse = token_response.json().await.map_err(anyhow::Error::from)?;
    if !token.token_type.eq_ignore_ascii_case("Bearer") {
        tracing::warn!("google token response used unexpected token_type");
        return Err(AppError::Unauthorized);
    }

    let userinfo_response = state
        .http
        .get(GOOGLE_USERINFO_URL)
        .bearer_auth(&token.access_token)
        .send()
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "google userinfo request failed");
            AppError::Internal(anyhow::anyhow!("google userinfo request failed"))
        })?;
    if !userinfo_response.status().is_success() {
        tracing::warn!(status = %userinfo_response.status(), "google userinfo rejected access token");
        return Err(AppError::Unauthorized);
    }
    let profile: UserInfo = userinfo_response.json().await.map_err(anyhow::Error::from)?;
    let verified = profile.email_verified.unwrap_or(false) || profile.verified_email.unwrap_or(false);
    if !verified {
        tracing::warn!("google account email is not verified");
        return Err(AppError::Unauthorized);
    }
    profile
        .email
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            tracing::warn!("google userinfo missing email");
            AppError::Unauthorized
        })
}

fn authorize_url(
    google: &crate::config::GoogleOAuthConfig,
    redirect_uri: &str,
    state_token: &str,
) -> AppResult<String> {
    let mut url = Url::parse(GOOGLE_AUTH_URL).map_err(anyhow::Error::from)?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("client_id", &google.client_id);
        query.append_pair("redirect_uri", redirect_uri);
        query.append_pair("response_type", "code");
        query.append_pair("scope", "openid email profile");
        query.append_pair("state", state_token);
        query.append_pair("access_type", "online");
        query.append_pair("include_granted_scopes", "true");
        query.append_pair("prompt", "select_account");
    }
    Ok(url.into())
}

fn sign_intent(config: &crate::config::Config, intent: &OAuthIntent) -> String {
    let payload = encode_intent(intent);
    let signature = URL_SAFE_NO_PAD.encode(keyed_digest(
        &config.session_secret,
        &format!("google-oauth:{payload}"),
    ));
    format!("{payload}.{signature}")
}

fn verify_intent(config: &crate::config::Config, state_token: &str) -> AppResult<OAuthIntent> {
    let (payload, signature) = state_token
        .rsplit_once('.')
        .ok_or(AppError::Unauthorized)?;
    let expected = URL_SAFE_NO_PAD.encode(keyed_digest(
        &config.session_secret,
        &format!("google-oauth:{payload}"),
    ));
    if expected.as_bytes().ct_eq(signature.as_bytes()).unwrap_u8() != 1 {
        return Err(AppError::Unauthorized);
    }
    decode_intent(payload)
}

fn encode_intent(intent: &OAuthIntent) -> String {
    let vault = intent.vault_id.as_deref().unwrap_or("");
    URL_SAFE_NO_PAD.encode(format!(
        "{}|{}|{}",
        intent.nonce, intent.purpose, vault
    ))
}

fn decode_intent(payload: &str) -> AppResult<OAuthIntent> {
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| AppError::Unauthorized)?;
    let text = String::from_utf8(bytes).map_err(|_| AppError::Unauthorized)?;
    let mut parts = text.splitn(3, '|');
    let nonce = parts.next().ok_or(AppError::Unauthorized)?.to_owned();
    let purpose = parts.next().ok_or(AppError::Unauthorized)?.to_owned();
    let vault = parts.next().unwrap_or("");
    if nonce.is_empty() || (purpose != "login" && purpose != "bind") {
        return Err(AppError::Unauthorized);
    }
    if purpose == "bind" && vault.is_empty() {
        return Err(AppError::Unauthorized);
    }
    Ok(OAuthIntent {
        nonce,
        purpose,
        vault_id: if vault.is_empty() {
            None
        } else {
            Some(vault.to_owned())
        },
    })
}

fn rate_limit_start(state: &AppState, headers: &HeaderMap, peer: SocketAddr) -> AppResult<()> {
    let ip = ip_hash(&state.config, headers, peer);
    if !state
        .limits
        .check(format!("google-oauth-start:{ip}"), 20, Duration::from_secs(3600))
    {
        return Err(AppError::RateLimited);
    }
    Ok(())
}

fn rate_limit_callback(state: &AppState, headers: &HeaderMap, peer: SocketAddr) -> AppResult<()> {
    let ip = ip_hash(&state.config, headers, peer);
    if !state
        .limits
        .check(format!("google-oauth-callback:{ip}"), 40, Duration::from_secs(3600))
    {
        return Err(AppError::RateLimited);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, GoogleOAuthConfig};

    fn sample_config() -> Config {
        Config {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            database_url: "sqlite://tmp.db".into(),
            database_path: "/tmp/unused".into(),
            frontend_dir: "/tmp".into(),
            public_base_url: "https://crossprompt.example.com".into(),
            app_env: "test".into(),
            admin_username: "admin".into(),
            admin_password_hash: "$argon2id$v=19$m=19456,t=2,p=1$Y3Jvc3Nwcm9tcC1kZXY$G6oY1qkT8qHk4n9qZ6z1Yw".into(),
            session_secret: "test-session-secret-that-is-long-enough".into(),
            master_key: [9; 32],
            ip_hash_salt: "test-ip-hash-salt-long-enough".into(),
            turnstile_secret_key: None,
            turnstile_site_key: None,
            cookie_secure: true,
            trust_proxy: false,
            smtp: None,
            google_oauth: Some(GoogleOAuthConfig {
                client_id: "client.apps.googleusercontent.com".into(),
                client_secret: "secret".into(),
            }),
        }
    }

    #[test]
    fn oauth_intent_roundtrip_and_signature() {
        let config = sample_config();
        let intent = OAuthIntent {
            nonce: "nonce-value".into(),
            purpose: "bind".into(),
            vault_id: Some("vault-1".into()),
        };
        let token = sign_intent(&config, &intent);
        assert!(token.contains('.'));
        let parsed = verify_intent(&config, &token).unwrap();
        assert_eq!(parsed, intent);
        assert!(verify_intent(&config, "tampered.token").is_err());
    }

    #[test]
    fn authorize_url_contains_required_params() {
        let google = GoogleOAuthConfig {
            client_id: "abc.apps.googleusercontent.com".into(),
            client_secret: "secret".into(),
        };
        let url = authorize_url(
            &google,
            "https://crossprompt.example.com/api/v1/auth/google/callback",
            "signed-state-token",
        )
        .unwrap();
        assert!(url.contains("client_id=abc.apps.googleusercontent.com"));
        assert!(url.contains("state=signed-state-token"));
        assert!(url.contains("openid"));
        assert!(url.contains("email"));
    }

    #[test]
    fn redirect_uri_uses_public_base_url() {
        let config = sample_config();
        assert_eq!(
            config.google_redirect_uri(),
            "https://crossprompt.example.com/api/v1/auth/google/callback"
        );
    }

    #[test]
    fn google_vault_name_uses_local_part() {
        assert_eq!(google_vault_name("ada@example.com"), "ada's CrossPrompt");
        assert_eq!(google_vault_name("@example.com"), "My CrossPrompt");
    }
}
