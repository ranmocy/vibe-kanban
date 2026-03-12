use deployment::Deployment;
use axum::{
    Router,
    extract::{Json, Path, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::MutationResponse;
use crate::DeploymentImpl;

#[derive(Debug, Serialize, FromRow)]
struct IssueComment {
    id: String,
    issue_id: String,
    author_id: Option<String>,
    parent_id: Option<String>,
    message: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct CreateIssueCommentRequest {
    id: Option<String>,
    issue_id: String,
    author_id: Option<String>,
    parent_id: Option<String>,
    message: String,
}

#[derive(Debug, Deserialize)]
struct UpdateIssueCommentRequest {
    message: Option<String>,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/kanban/issue/{issue_id}/comments", get(list_issue_comments))
        .route("/kanban/issue_comments", post(create_issue_comment))
        .route("/kanban/issue_comments/{id}", get(get_issue_comment).patch(update_issue_comment).delete(delete_issue_comment))
}

async fn list_issue_comments(
    State(deployment): State<DeploymentImpl>,
    Path(issue_id): Path<String>,
) -> Result<ResponseJson<Vec<IssueComment>>, StatusCode> {
    let pool = &deployment.db().pool;
    let comments = sqlx::query_as::<_, IssueComment>(
        "SELECT id, issue_id, author_id, parent_id, message, created_at, updated_at \
         FROM kanban_issue_comments WHERE issue_id = ? ORDER BY created_at",
    )
    .bind(&issue_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list issue comments: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(ResponseJson(comments))
}

async fn get_issue_comment(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<ResponseJson<IssueComment>, StatusCode> {
    let pool = &deployment.db().pool;
    let comment = sqlx::query_as::<_, IssueComment>(
        "SELECT id, issue_id, author_id, parent_id, message, created_at, updated_at \
         FROM kanban_issue_comments WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get issue comment: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;
    Ok(ResponseJson(comment))
}

async fn create_issue_comment(
    State(deployment): State<DeploymentImpl>,
    Json(req): Json<CreateIssueCommentRequest>,
) -> Result<ResponseJson<MutationResponse<IssueComment>>, StatusCode> {
    let pool = &deployment.db().pool;
    let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    sqlx::query(
        "INSERT INTO kanban_issue_comments (id, issue_id, author_id, parent_id, message, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.issue_id)
    .bind(&req.author_id)
    .bind(&req.parent_id)
    .bind(&req.message)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create issue comment: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let comment = sqlx::query_as::<_, IssueComment>(
        "SELECT id, issue_id, author_id, parent_id, message, created_at, updated_at \
         FROM kanban_issue_comments WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch created comment: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(ResponseJson(MutationResponse { data: comment, txid: 1 }))
}

async fn update_issue_comment(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
    Json(req): Json<UpdateIssueCommentRequest>,
) -> Result<ResponseJson<MutationResponse<IssueComment>>, StatusCode> {
    let pool = &deployment.db().pool;
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    let existing = sqlx::query_as::<_, IssueComment>(
        "SELECT id, issue_id, author_id, parent_id, message, created_at, updated_at \
         FROM kanban_issue_comments WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get issue comment: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    let message = req.message.unwrap_or(existing.message);

    sqlx::query("UPDATE kanban_issue_comments SET message = ?, updated_at = ? WHERE id = ?")
        .bind(&message)
        .bind(&now)
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to update issue comment: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let comment = sqlx::query_as::<_, IssueComment>(
        "SELECT id, issue_id, author_id, parent_id, message, created_at, updated_at \
         FROM kanban_issue_comments WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch updated comment: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(ResponseJson(MutationResponse { data: comment, txid: 1 }))
}

async fn delete_issue_comment(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let pool = &deployment.db().pool;
    let result = sqlx::query("DELETE FROM kanban_issue_comments WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete issue comment: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}
