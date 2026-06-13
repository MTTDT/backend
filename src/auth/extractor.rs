use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::future::Future;

use crate::{auth::jwt::validate_token, state::AppState};

// ── AuthUser ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: String,
    pub username: String,
    pub is_admin: bool
}

pub struct AuthError(StatusCode, String);

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({ "error": self.1 }))).into_response()
    }
}

#[derive(Debug, Clone)]
pub struct AdminUser {
    pub user_id: String,
    pub username: String,
}

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AuthError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|s| s.to_owned());

        let secret = state.jwt_secret.clone();

        async move {
            let token = token.ok_or_else(|| AuthError(
                StatusCode::UNAUTHORIZED,
                "Missing or malformed Authorization header".into(),
            ))?;

            let claims = validate_token(&token, &secret).map_err(|_| AuthError(
                StatusCode::UNAUTHORIZED,
                "Invalid or expired token".into(),
            ))?;

            if !claims.is_admin {
                return Err(AuthError(
                    StatusCode::FORBIDDEN,
                    "Admin access required".into(),
                ));
            }

            Ok(AdminUser {
                user_id: claims.sub,
                username: claims.username,
            })
        }
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AuthError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let token = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|s| s.to_owned());

        let secret = state.jwt_secret.clone();

        async move {
            let token = token.ok_or_else(|| AuthError(
                StatusCode::UNAUTHORIZED,
                "Missing or malformed Authorization header".into(),
            ))?;

            let claims = validate_token(&token, &secret).map_err(|_| AuthError(
                StatusCode::UNAUTHORIZED,
                "Invalid or expired token".into(),
            ))?;

            Ok(AuthUser { user_id: claims.sub, username: claims.username, is_admin: claims.is_admin })
        }
    }
}

// ── SessionContext ────────────────────────────────────────────────────────────
//
// Single extractor resolving auth-or-guest in one place.
// Axum 0.8 requires all extractor logic in one type — combining
// Option<AuthUser> + HeaderMap inline in a handler signature breaks Handler bounds.

#[derive(Debug, Clone)]
pub struct SessionContext {
    pub session_id: String,
    /// Some(user_id) if authenticated via JWT, None if guest
    pub user_id: Option<String>,
}

impl FromRequestParts<AppState> for SessionContext {
    type Rejection = AuthError;

    fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        // Extract both possible sources eagerly (before the async block)
        // so we don't hold a borrow on `parts` across an await.
        let bearer = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|s| s.to_owned());

        let guest = parts
            .headers
            .get("x-guest-session")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_owned());

        let secret = state.jwt_secret.clone();

        async move {
            if let Some(token) = bearer {
                if let Ok(claims) = validate_token(&token, &secret) {
                    return Ok(SessionContext {
                        session_id: claims.sub.clone(),
                        user_id: Some(claims.sub),
                    });
                }
            }

            if let Some(guest_id) = guest {
                return Ok(SessionContext {
                    session_id: guest_id,
                    user_id: None,
                });
            }

            Err(AuthError(
                StatusCode::UNAUTHORIZED,
                "Provide a Bearer token or X-Guest-Session header".into(),
            ))
        }
    }
}