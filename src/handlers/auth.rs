use axum::{extract::State, Json};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::{
        extractor::AuthUser,
        jwt::create_token,
        password::{hash_password, verify_password},
    },
    errors::AppError,
    models::{AuthResponse, LoginRequest, MessageResponse, PublicUser, UserRow, UserTickerRow},
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct RegisterPayload {
    pub username: String,
    pub email: String,
    pub password: String,
    #[serde(default)]
    pub guest_tickers: Vec<GuestTicker>,
    pub guest_session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GuestTicker {
    pub ticker: String,
    #[serde(default = "default_interval")]
    pub interval: String,
    #[serde(default = "default_range")]
    pub range: String,
}

fn default_interval() -> String { "1d".into() }
fn default_range() -> String { "3mo".into() }

// ── /api/auth/register ───────────────────────────────────────────────────────

pub async fn register(
    State(state): State<AppState>,
    Json(payload): Json<RegisterPayload>,
) -> Result<Json<AuthResponse>, AppError> {
    if payload.username.trim().is_empty() || payload.email.trim().is_empty() {
        return Err(AppError::BadRequest("Username and email are required".into()));
    }
    if payload.password.len() < 8 {
        return Err(AppError::BadRequest("Password must be at least 8 characters".into()));
    }

    let user_id = Uuid::new_v4().to_string();
    let hash = hash_password(&payload.password)?;

    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash) VALUES (?, ?, ?, ?)"
    )
    .bind(&user_id)
    .bind(&payload.username)
    .bind(&payload.email)
    .bind(&hash)
    .execute(&state.db)
    .await?;

    for gt in &payload.guest_tickers {
        sqlx::query(
            "INSERT OR IGNORE INTO user_tickers (user_id, ticker, interval, range) VALUES (?, ?, ?, ?)"
        )
        .bind(&user_id)
        .bind(&gt.ticker)
        .bind(&gt.interval)
        .bind(&gt.range)
        .execute(&state.db)
        .await?;
    }

    if let Some(guest_id) = &payload.guest_session_id {
        let mut sessions = state.sessions.lock().await;
        if let Some(register) = sessions.remove(guest_id.as_str()) {
            sessions.insert(user_id.clone(), register);
        }
    }


    let user: UserRow = sqlx::query_as(
        "SELECT id, username, email, password_hash, created_at, updated_at, is_admin FROM users WHERE id = ?"
    )
    .bind(&user_id)
    .fetch_one(&state.db)
    .await?;

    let token = create_token(&user_id, &payload.username,  user.is_admin, &state.jwt_secret)?;


    Ok(Json(AuthResponse { token, user: PublicUser::from(user) }))
}

// ── /api/auth/login ──────────────────────────────────────────────────────────

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let row: Option<UserRow> = sqlx::query_as(
        "SELECT id, username, email, password_hash, created_at, updated_at, is_admin
         FROM users WHERE username = ?"
    )
    .bind(&payload.username)
    .fetch_optional(&state.db)
    .await?;

    let row = row.ok_or_else(|| AppError::Auth("Invalid username or password".into()))?;

    if !verify_password(&payload.password, &row.password_hash)? {
        return Err(AppError::Auth("Invalid username or password".into()));
    }

    // Restore persisted tickers into memory in background
    let saved_tickers: Vec<UserTickerRow> = sqlx::query_as(
        "SELECT id, user_id, ticker, interval, range, added_at
         FROM user_tickers WHERE user_id = ?"
    )
    .bind(&row.id)
    .fetch_all(&state.db)
    .await?;

    if !saved_tickers.is_empty() {
        let session = state.get_or_create_session(&row.id).await;
        let tickers_to_load = saved_tickers;
        tokio::spawn(async move {
            let mut reg = session.lock().await;
            for t in tickers_to_load {
                if reg.get(&t.ticker).is_none() {
                    if let Err(e) = reg.fetch(&t.ticker, &t.interval, &t.range).await {
                        tracing::warn!("Failed to restore {}: {}", t.ticker, e);
                    }
                }
            }
        });
    }

    let token = create_token(&row.id, &row.username, row.is_admin, &state.jwt_secret)?;
    Ok(Json(AuthResponse { token, user: PublicUser::from(row) }))
}

// ── /api/auth/logout ─────────────────────────────────────────────────────────

pub async fn logout(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<MessageResponse>, AppError> {
    state.remove_session(&auth.user_id).await;
    Ok(Json(MessageResponse { message: "Logged out successfully".into() }))
}

// ── /api/auth/me ─────────────────────────────────────────────────────────────

pub async fn me(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<PublicUser>, AppError> {
    let row: Option<UserRow> = sqlx::query_as(
        "SELECT id, username, email, password_hash, created_at, updated_at, is_admin
         FROM users WHERE id = ?"
    )
    .bind(&auth.user_id)
    .fetch_optional(&state.db)
    .await?;

    let row = row.ok_or_else(|| AppError::NotFound("User not found".into()))?;
    Ok(Json(PublicUser::from(row)))
}