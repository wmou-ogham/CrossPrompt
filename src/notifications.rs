use anyhow::{bail, Context, Result};
use axum::http::StatusCode;
use chrono::Utc;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::json;
use sqlx::FromRow;

use crate::{
    models::{CallbackPayload, NotificationConfig, NotificationTargetView},
    security::{decrypt_config, mask_url, validate_headers, validate_webhook_url},
    state::AppState,
};

#[derive(Debug, FromRow)]
pub struct StoredTarget {
    pub kind: String,
    pub encrypted_config: Vec<u8>,
    pub updated_at: String,
}

pub async fn load_target(state: &AppState, vault_id: &str) -> Result<Option<(StoredTarget, NotificationConfig)>> {
    let target = sqlx::query_as::<_, StoredTarget>(
        "SELECT kind, encrypted_config, updated_at FROM notification_targets WHERE vault_id = ?",
    )
    .bind(vault_id)
    .fetch_optional(&state.pool)
    .await?;
    target
        .map(|stored| {
            let config = decrypt_config(&state.config.master_key, &stored.encrypted_config)?;
            Ok((stored, config))
        })
        .transpose()
}

pub async fn validate_target(kind: &str, config: &NotificationConfig) -> Result<()> {
    if !matches!(kind, "pushcut" | "ntfy" | "generic_json") {
        bail!("unsupported notification target type");
    }
    if config.url.len() > 2048 {
        bail!("notification URL is too long");
    }
    validate_headers(&config.headers)?;
    validate_webhook_url(&config.url).await?;
    Ok(())
}

pub fn view(stored: &StoredTarget, config: &NotificationConfig) -> NotificationTargetView {
    NotificationTargetView {
        kind: stored.kind.clone(),
        masked_url: mask_url(&config.url),
        updated_at: stored.updated_at.clone(),
    }
}

pub async fn send(state: &AppState, vault_id: &str, target: &StoredTarget, config: &NotificationConfig, payload: &CallbackPayload) -> Result<StatusCode> {
    let (url, addresses) = validate_webhook_url(&config.url).await?;
    let host = url.host_str().context("webhook URL missing host")?.to_owned();
    let mut headers = HeaderMap::new();
    for (name, value) in &config.headers {
        headers.insert(HeaderName::try_from(name)?, HeaderValue::try_from(value)?);
    }

    // Pin the already-validated DNS answers for this request to prevent rebinding.
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("CrossPrompt/0.1")
        .resolve_to_addrs(&host, &addresses)
        .build()?;
    let request = client.post(url).headers(headers);
    let response = match target.kind.as_str() {
        "ntfy" => {
            let tags = match payload.status.as_str() {
                "completed" => "white_check_mark",
                "needs_input" => "question",
                "failed" => "warning",
                _ => "bell",
            };
            let mut request = request
                .header("Title", sanitize_header(&payload.title))
                .header("Tags", tags)
                .body(payload.message.clone());
            if let Some(url) = &payload.url {
                request = request.header("Click", sanitize_header(url));
            }
            request.send().await
        }
        "pushcut" => request
            .json(&json!({
                "title": payload.title,
                "text": payload.message,
                "input": payload.url,
                "isTimeSensitive": payload.status == "needs_input",
            }))
            .send()
            .await,
        "generic_json" => request.json(payload).send().await,
        _ => bail!("unsupported notification target type"),
    };
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            record_delivery(state, vault_id, &target.kind, None, false).await?;
            return Err(error.into());
        }
    };
    let status = StatusCode::from_u16(response.status().as_u16())?;
    record_delivery(state, vault_id, &target.kind, Some(i64::from(status.as_u16())), status.is_success()).await?;
    if !status.is_success() {
        bail!("notification service returned {status}");
    }
    Ok(status)
}

async fn record_delivery(
    state: &AppState,
    vault_id: &str,
    kind: &str,
    status_code: Option<i64>,
    success: bool,
) -> Result<()> {
    sqlx::query("INSERT INTO webhook_deliveries (vault_id, target_type, status_code, success, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(vault_id)
        .bind(kind)
        .bind(status_code)
        .bind(success)
        .bind(Utc::now().to_rfc3339())
        .execute(&state.pool)
        .await?;
    Ok(())
}

fn sanitize_header(value: &str) -> String {
    value.replace(['\r', '\n'], " ").chars().take(512).collect()
}
