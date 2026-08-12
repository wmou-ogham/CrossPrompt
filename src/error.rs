use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("authentication required")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("resource not found")]
    NotFound,
    #[error("vault is suspended")]
    Locked,
    #[error("vault has been deleted")]
    Gone,
    #[error("resource was changed by another client")]
    Conflict,
    #[error("rate limit exceeded")]
    RateLimited,
    #[error(transparent)]
    Sql(#[from] sqlx::Error),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    pub fn bad(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, "bad_request", message.clone()),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", self.to_string()),
            Self::Forbidden => (StatusCode::FORBIDDEN, "forbidden", self.to_string()),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found", self.to_string()),
            Self::Locked => (StatusCode::LOCKED, "vault_suspended", self.to_string()),
            Self::Gone => (StatusCode::GONE, "vault_deleted", self.to_string()),
            Self::Conflict => (StatusCode::CONFLICT, "version_conflict", self.to_string()),
            Self::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate_limited", self.to_string()),
            Self::Sql(error) => {
                tracing::error!(error = %error, "database error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "internal server error".into())
            }
            Self::Internal(error) => {
                tracing::error!(error = %error, "internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "internal server error".into())
            }
        };
        (status, Json(json!({ "error": { "code": code, "message": message } }))).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;

