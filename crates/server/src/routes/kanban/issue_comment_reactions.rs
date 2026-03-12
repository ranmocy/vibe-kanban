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
struct IssueCommentReaction {
    id: String,
    comment_id: String,
    user_id: String,
    emoji: String,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct CreateIssueCommentReactionRequest {
    id: Option<String>,
    comment_id: String,
    user_id: String,
    emoji: String,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/kanban/issue/{issue_id}/reactions", get(list_issue_comment_reactions))
        .route("/kanban/issue_comment_reactions", post(create_issue_comment_reaction))
        .route("/kanban/issue_comment_reactions/{id}", get(get_issue_comment_reaction).delete(delete_issue_comment_reaction))
}

async fn list_issue_comment_reactions(
    State(deployment): State<DeploymentImpl>,
    Path(issue_id): Path<String>,
) -> Result<ResponseJson<Vec<IssueCommentReaction>>, StatusCode> {
    let pool = &deployment.db().pool;
    let reactions = sqlx::query_as::<_, IssueCommentReaction>(
        "SELECT r.id, r.comment_id, r.user_id, r.emoji, r.created_at \
         FROM kanban_issue_comment_reactions r \
         INNER JOIN kanban_issue_comments c ON c.id = r.comment_id \
         WHERE c.issue_id = ?",
    )
    .bind(&issue_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list issue comment reactions: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(ResponseJson(reactions))
}

async fn get_issue_comment_reaction(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<ResponseJson<IssueCommentReaction>, StatusCode> {
    let pool = &deployment.db().pool;
    let reaction = sqlx::query_as::<_, IssueCommentReaction>(
        "SELECT id, comment_id, user_id, emoji, created_at \
         FROM kanban_issue_comment_reactions WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get issue comment reaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;
    Ok(ResponseJson(reaction))
}

async fn create_issue_comment_reaction(
    State(deployment): State<DeploymentImpl>,
    Json(req): Json<CreateIssueCommentReactionRequest>,
) -> Result<ResponseJson<MutationResponse<IssueCommentReaction>>, StatusCode> {
    let pool = &deployment.db().pool;
    let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    sqlx::query(
        "INSERT INTO kanban_issue_comment_reactions (id, comment_id, user_id, emoji, created_at) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.comment_id)
    .bind(&req.user_id)
    .bind(&req.emoji)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create issue comment reaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let reaction = sqlx::query_as::<_, IssueCommentReaction>(
        "SELECT id, comment_id, user_id, emoji, created_at \
         FROM kanban_issue_comment_reactions WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch created reaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(ResponseJson(MutationResponse { data: reaction, txid: 1 }))
}

async fn delete_issue_comment_reaction(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let pool = &deployment.db().pool;
    let result = sqlx::query("DELETE FROM kanban_issue_comment_reactions WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete issue comment reaction: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}
