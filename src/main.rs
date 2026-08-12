mod admin;
mod api;
mod auth;
mod cleanup;
mod config;
mod db;
mod error;
mod models;
mod notifications;
mod openapi;
mod rate_limit;
mod security;
mod state;

use std::{net::SocketAddr, time::Duration};

use axum::{
    extract::State,
    http::{header, HeaderValue, StatusCode},
    routing::{delete, get, patch, post},
    Router,
};
use state::AppState;
use tower_http::{
    catch_panic::CatchPanicLayer,
    compression::CompressionLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cross_prompt=info,tower_http=info".into()),
        )
        .init();

    let config = config::Config::from_env()?;
    let pool = db::connect(&config).await?;
    let state = AppState::new(config, pool)?;
    cleanup::spawn(state.clone());

    let address = state.config.bind_addr;
    let environment = state.config.app_env.clone();
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(%address, %environment, "CrossPrompt listening");
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

fn router(state: AppState) -> Router {
    let public_api = Router::new()
        .route("/config", get(api::config_public))
        .route("/vaults", post(api::create_vault))
        .route(
            "/vault",
            get(api::get_vault)
                .patch(api::rename_vault)
                .delete(api::delete_vault),
        )
        .route("/vault/restore", post(api::restore_vault))
        .route("/vault/rotate-secret", post(api::rotate_secret))
        .route("/blocks", get(api::list_blocks).post(api::create_block))
        .route("/blocks/reorder", post(api::reorder_blocks))
        .route(
            "/blocks/{id}",
            patch(api::update_block).delete(api::delete_block),
        )
        .route("/bundles", get(api::list_bundles).post(api::create_bundle))
        .route(
            "/bundles/{id}",
            patch(api::update_bundle).delete(api::delete_bundle),
        )
        .route("/revisions", get(api::list_revisions))
        .route("/revisions/{id}/restore", post(api::restore_revision))
        .route(
            "/notification-target",
            get(api::get_notification_target)
                .put(api::put_notification_target)
                .delete(api::delete_notification_target),
        )
        .route(
            "/notification-target/test",
            post(api::test_notification_target),
        )
        .route("/callback/{secret}", post(api::callback))
        .route("/openapi.json", get(openapi::document));

    let admin_api = Router::new()
        .route(
            "/session",
            get(admin::session_info)
                .post(admin::login)
                .delete(admin::logout),
        )
        .route("/overview", get(admin::overview))
        .route("/vaults", get(admin::list_vaults))
        .route("/vaults/{id}", get(admin::get_vault))
        .route("/vaults/{id}/suspend", post(admin::suspend))
        .route("/vaults/{id}/resume", post(admin::resume))
        .route("/vaults/{id}/delete", post(admin::soft_delete))
        .route("/vaults/{id}/restore", post(admin::restore))
        .route("/vaults/{id}/permanent", delete(admin::permanent_delete))
        .route("/audit-log", get(admin::audit_log));

    let index = state.config.frontend_dir.join("index.html");
    let static_files = ServeDir::new(&state.config.frontend_dir)
        .not_found_service(ServeFile::new(index));

    Router::new()
        .route("/healthz", get(health))
        .route("/readyz", get(ready))
        .nest("/api/v1/admin", admin_api)
        .nest("/api/v1", public_api)
        .fallback_service(static_files)
        .layer(SetResponseHeaderLayer::if_not_present(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(
                "default-src 'self'; script-src 'self' https://challenges.cloudflare.com; frame-src https://challenges.cloudflare.com; connect-src 'self' https://challenges.cloudflare.com; style-src 'self'; img-src 'self' data:; base-uri 'none'; object-src 'none'; frame-ancestors 'none'",
            ),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(CompressionLayer::new())
        .layer(CatchPanicLayer::new())
        // The span intentionally excludes the request URI because callbacks contain a secret.
        .layer(TraceLayer::new_for_http().make_span_with(|request: &axum::http::Request<_>| {
            tracing::info_span!("http_request", method = %request.method())
        }))
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .with_state(state)
}

async fn health() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn ready(State(state): State<AppState>) -> StatusCode {
    match sqlx::query_scalar::<_, i64>("SELECT 1").fetch_one(&state.pool).await {
        Ok(1) => StatusCode::NO_CONTENT,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install Ctrl+C handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
    tokio::time::sleep(Duration::from_millis(100)).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        extract::connect_info::ConnectInfo,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::{json, Value};
    use tower::ServiceExt;
    use uuid::Uuid;

    #[tokio::test]
    async fn vault_lifecycle_and_optimistic_concurrency() {
        let database_path = std::path::PathBuf::from(format!(
            "/tmp/crossprompt-api-test-{}.db",
            Uuid::new_v4()
        ));
        let config = config::Config {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            database_url: format!("sqlite://{}", database_path.display()),
            database_path: database_path.clone(),
            frontend_dir: "/tmp/crossprompt-empty-static".into(),
            public_base_url: "http://crossprompt.test".into(),
            app_env: "test".into(),
            admin_username: "admin".into(),
            admin_password_hash: "$argon2id$v=19$m=19456,t=2,p=1$Y3Jvc3Nwcm9tcHQtZGV2$xLQdyhm+6q3K0zRrcRQgcYLDbhq73JC6EHmvoOrqEaM".into(),
            session_secret: "test-session-secret-that-is-long-enough".into(),
            master_key: [7; 32],
            ip_hash_salt: "test-ip-hash-salt-long-enough".into(),
            turnstile_secret_key: None,
            turnstile_site_key: None,
            cookie_secure: false,
            trust_proxy: false,
        };
        let pool = db::connect(&config).await.unwrap();
        let app = router(AppState::new(config, pool.clone()).unwrap());
        let peer = ConnectInfo("203.0.113.20:43100".parse::<SocketAddr>().unwrap());

        let response = app
            .clone()
            .oneshot(json_request("POST", "/api/v1/vaults", json!({"name":"Portable prompts"}), None, peer))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let created = response_json(response).await;
        let secret = created["secret"].as_str().unwrap().to_owned();

        let response = app
            .clone()
            .oneshot(json_request(
                "POST",
                "/api/v1/blocks?source=integration-test",
                json!({"title":"System prompt","content":"Be concise."}),
                Some(&secret),
                peer,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let block = response_json(response).await;
        let block_id = block["id"].as_str().unwrap();

        let response = app
            .clone()
            .oneshot(json_request(
                "PATCH",
                &format!("/api/v1/blocks/{block_id}"),
                json!({"title":"System prompt","content":"Changed","version":999}),
                Some(&secret),
                peer,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);

        let response = app
            .clone()
            .oneshot(json_request("POST", "/api/v1/vault/rotate-secret", json!({}), Some(&secret), peer))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let rotated = response_json(response).await;
        let new_secret = rotated["secret"].as_str().unwrap();

        let old_response = app
            .clone()
            .oneshot(json_request("GET", "/api/v1/vault", json!({}), Some(&secret), peer))
            .await
            .unwrap();
        assert_eq!(old_response.status(), StatusCode::UNAUTHORIZED);
        let new_response = app
            .oneshot(json_request("GET", "/api/v1/vault", json!({}), Some(new_secret), peer))
            .await
            .unwrap();
        assert_eq!(new_response.status(), StatusCode::OK);

        pool.close().await;
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", database_path.display()));
        }
    }

    fn json_request(
        method: &str,
        uri: &str,
        body: Value,
        secret: Option<&str>,
        peer: ConnectInfo<SocketAddr>,
    ) -> Request<Body> {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json");
        if let Some(secret) = secret {
            builder = builder.header("authorization", format!("Bearer {secret}"));
        }
        let mut request = builder.body(Body::from(body.to_string())).unwrap();
        request.extensions_mut().insert(peer);
        request
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }
}
