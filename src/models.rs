use serde::{Deserialize, Serialize};

/// Row returned from the `users` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserRow {
    pub id: String,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub created_at: String,
    pub updated_at: String,
    pub is_admin: bool
}

/// Safe public-facing user representation (no hash).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicUser {
    pub id: String,
    pub username: String,
    pub email: String,
    pub created_at: String,
    pub is_admin: bool
}

impl From<UserRow> for PublicUser {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            username: row.username,
            email: row.email,
            created_at: row.created_at,
            is_admin: row.is_admin,
        }
    }
}

/// Row from the `user_tickers` table.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserTickerRow {
    pub id: i64,
    pub user_id: String,
    pub ticker: String,
    pub interval: String,
    pub range: String,
    pub added_at: String,
}

// ── Auth request/response shapes ────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
    /// If true, caller will also send a list of tickers to persist
    #[serde(default)]
    pub persist_guest_tickers: bool,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: PublicUser,
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: String,
}