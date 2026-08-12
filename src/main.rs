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
        http::{header, Request, StatusCode},
    };
    use argon2::{password_hash::{PasswordHasher, SaltString}, Argon2};
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
        let config = test_config(database_path.clone(), test_password_hash("unused-password"));
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

    #[tokio::test]
    async fn administrator_moderation_is_csrf_protected_and_audited() {
        let database_path = std::path::PathBuf::from(format!(
            "/tmp/crossprompt-admin-test-{}.db",
            Uuid::new_v4()
        ));
        let password = "correct horse battery staple";
        let config = test_config(database_path.clone(), test_password_hash(password));
        let pool = db::connect(&config).await.unwrap();
        let app = router(AppState::new(config, pool.clone()).unwrap());
        let user_peer = ConnectInfo("203.0.113.20:43100".parse::<SocketAddr>().unwrap());
        let admin_peer = ConnectInfo("198.51.100.14:52100".parse::<SocketAddr>().unwrap());

        let response = app.clone().oneshot(json_request(
            "POST", "/api/v1/vaults", json!({"name":"Moderated Vault"}), None, user_peer,
        )).await.unwrap();
        let created = response_json(response).await;
        let secret = created["secret"].as_str().unwrap().to_owned();
        let vault_id = created["vault"]["id"].as_str().unwrap().to_owned();

        let login_response = app.clone().oneshot(session_request(
            "POST", "/api/v1/admin/session",
            json!({"username":"admin","password":password}), None, None, admin_peer,
        )).await.unwrap();
        assert_eq!(login_response.status(), StatusCode::OK);
        let cookie = login_response.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap()
            .split(';').next().unwrap().to_owned();
        let login_data = response_json(login_response).await;
        let csrf = login_data["csrf_token"].as_str().unwrap().to_owned();

        let no_csrf = app.clone().oneshot(session_request(
            "POST", &format!("/api/v1/admin/vaults/{vault_id}/suspend"),
            json!({"reason":"abuse review"}), Some(&cookie), None, admin_peer,
        )).await.unwrap();
        assert_eq!(no_csrf.status(), StatusCode::FORBIDDEN);

        let detail = app.clone().oneshot(session_request(
            "GET", &format!("/api/v1/admin/vaults/{vault_id}"), json!({}),
            Some(&cookie), None, admin_peer,
        )).await.unwrap();
        assert_eq!(detail.status(), StatusCode::OK);

        let suspended = app.clone().oneshot(session_request(
            "POST", &format!("/api/v1/admin/vaults/{vault_id}/suspend"),
            json!({"reason":"abuse review"}), Some(&cookie), Some(&csrf), admin_peer,
        )).await.unwrap();
        assert_eq!(suspended.status(), StatusCode::NO_CONTENT);
        let locked = app.clone().oneshot(json_request(
            "GET", "/api/v1/vault", json!({}), Some(&secret), user_peer,
        )).await.unwrap();
        assert_eq!(locked.status(), StatusCode::LOCKED);

        let resumed = app.clone().oneshot(session_request(
            "POST", &format!("/api/v1/admin/vaults/{vault_id}/resume"),
            json!({"reason":"review complete"}), Some(&cookie), Some(&csrf), admin_peer,
        )).await.unwrap();
        assert_eq!(resumed.status(), StatusCode::NO_CONTENT);
        let active = app.clone().oneshot(json_request(
            "GET", "/api/v1/vault", json!({}), Some(&secret), user_peer,
        )).await.unwrap();
        assert_eq!(active.status(), StatusCode::OK);

        let deleted = app.clone().oneshot(session_request(
            "POST", &format!("/api/v1/admin/vaults/{vault_id}/delete"),
            json!({"reason":"policy"}), Some(&cookie), Some(&csrf), admin_peer,
        )).await.unwrap();
        assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
        let user_restore = app.clone().oneshot(json_request(
            "POST", "/api/v1/vault/restore", json!({}), Some(&secret), user_peer,
        )).await.unwrap();
        assert_eq!(user_restore.status(), StatusCode::FORBIDDEN);

        let wrong_confirmation = app.clone().oneshot(session_request(
            "DELETE", &format!("/api/v1/admin/vaults/{vault_id}/permanent"),
            json!({"confirmation":"wrong-id","reason":"test"}), Some(&cookie), Some(&csrf), admin_peer,
        )).await.unwrap();
        assert_eq!(wrong_confirmation.status(), StatusCode::BAD_REQUEST);

        let restored = app.clone().oneshot(session_request(
            "POST", &format!("/api/v1/admin/vaults/{vault_id}/restore"),
            json!({"reason":"appeal"}), Some(&cookie), Some(&csrf), admin_peer,
        )).await.unwrap();
        assert_eq!(restored.status(), StatusCode::NO_CONTENT);

        let audit_response = app.oneshot(session_request(
            "GET", "/api/v1/admin/audit-log", json!({}), Some(&cookie), None, admin_peer,
        )).await.unwrap();
        let audit = response_json(audit_response).await;
        let actions = audit["items"].as_array().unwrap().iter()
            .filter_map(|item| item["action"].as_str()).collect::<Vec<_>>();
        assert!(actions.contains(&"view_content"));
        assert!(actions.contains(&"suspend"));
        assert!(actions.contains(&"resume"));
        assert!(actions.contains(&"soft_delete"));
        assert!(actions.contains(&"restore"));

        pool.close().await;
        remove_test_database(&database_path);
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

    fn session_request(
        method: &str,
        uri: &str,
        body: Value,
        cookie: Option<&str>,
        csrf: Option<&str>,
        peer: ConnectInfo<SocketAddr>,
    ) -> Request<Body> {
        let mut builder = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json");
        if let Some(cookie) = cookie { builder = builder.header(header::COOKIE, cookie); }
        if let Some(csrf) = csrf { builder = builder.header("x-csrf-token", csrf); }
        let mut request = builder.body(Body::from(body.to_string())).unwrap();
        request.extensions_mut().insert(peer);
        request
    }

    async fn response_json(response: axum::response::Response) -> Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn test_password_hash(password: &str) -> String {
        let salt = SaltString::encode_b64(b"crossprompt-test").unwrap();
        Argon2::default().hash_password(password.as_bytes(), &salt).unwrap().to_string()
    }

    fn test_config(database_path: std::path::PathBuf, admin_password_hash: String) -> config::Config {
        config::Config {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            database_url: format!("sqlite://{}", database_path.display()),
            database_path,
            frontend_dir: "/tmp/crossprompt-empty-static".into(),
            public_base_url: "http://crossprompt.test".into(),
            app_env: "test".into(),
            admin_username: "admin".into(),
            admin_password_hash,
            session_secret: "test-session-secret-that-is-long-enough".into(),
            master_key: [7; 32],
            ip_hash_salt: "test-ip-hash-salt-long-enough".into(),
            turnstile_secret_key: None,
            turnstile_site_key: None,
            cookie_secure: false,
            trust_proxy: false,
        }
    }

    fn remove_test_database(path: &std::path::Path) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }
}
