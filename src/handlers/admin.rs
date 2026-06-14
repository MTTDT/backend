use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;

use crate::{
    auth::extractor::AdminUser,
    errors::AppError,
    models::{MessageResponse, PublicUser, UserRow},
    state::AppState,
};

// ── GET /api/admin/users ──────────────────────────────────────────────────────

pub async fn get_users(
    State(state): State<AppState>,
    _admin: AdminUser,
) -> Result<Json<Vec<PublicUser>>, AppError> {
    let rows: Vec<UserRow> = sqlx::query_as(
        "SELECT id, username, email, password_hash, created_at, updated_at, is_admin
         FROM users ORDER BY created_at DESC"
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(rows.into_iter().map(PublicUser::from).collect()))
}

// ── DELETE /api/admin/users/:id ───────────────────────────────────────────────

pub async fn delete_user(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(user_id): Path<String>,
) -> Result<Json<MessageResponse>, AppError> {
    if user_id == admin.user_id {
        return Err(AppError::BadRequest("Cannot delete your own account".into()));
    }

    let result = sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(&user_id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("User {} not found", user_id)));
    }

    // Drop their in-memory session if active
    state.remove_session(&user_id).await;

    Ok(Json(MessageResponse {
        message: format!("User {} deleted", user_id),
    }))
}

// ── PATCH /api/admin/users/:id/role ──────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ChangeRoleRequest {
    pub is_admin: bool,
}

pub async fn change_role(
    State(state): State<AppState>,
    admin: AdminUser,
    Path(user_id): Path<String>,
    Json(payload): Json<ChangeRoleRequest>,
) -> Result<Json<PublicUser>, AppError> {
    if user_id == admin.user_id && !payload.is_admin {
        return Err(AppError::BadRequest("Cannot remove your own admin rights".into()));
    }

    let result = sqlx::query("UPDATE users SET is_admin = ? WHERE id = ?")
        .bind(payload.is_admin)
        .bind(&user_id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("User {} not found", user_id)));
    }

    let updated: UserRow = sqlx::query_as(
        "SELECT id, username, email, password_hash, created_at, updated_at, is_admin
         FROM users WHERE id = ?"
    )
    .bind(&user_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(PublicUser::from(updated)))
}