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
struct IssueFollower {
    id: String,
    issue_id: String,
    user_id: String,
}

#[derive(Debug, Deserialize)]
struct CreateIssueFollowerRequest {
    id: Option<String>,
    issue_id: String,
    user_id: String,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/kanban/project/{project_id}/issue_followers", get(list_issue_followers))
        .route("/kanban/issue_followers", post(create_issue_follower))
        .route("/kanban/issue_followers/{id}", get(get_issue_follower).delete(delete_issue_follower))
}

async fn list_issue_followers(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<String>,
) -> Result<ResponseJson<Vec<IssueFollower>>, StatusCode> {
    let pool = &deployment.db().pool;
    let followers = sqlx::query_as::<_, IssueFollower>(
        "SELECT f.id, f.issue_id, f.user_id \
         FROM kanban_issue_followers f \
         INNER JOIN kanban_issues i ON i.id = f.issue_id \
         WHERE i.project_id = ?",
    )
    .bind(&project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list issue followers: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(ResponseJson(followers))
}

async fn get_issue_follower(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<ResponseJson<IssueFollower>, StatusCode> {
    let pool = &deployment.db().pool;
    let follower = sqlx::query_as::<_, IssueFollower>(
        "SELECT id, issue_id, user_id FROM kanban_issue_followers WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get issue follower: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;
    Ok(ResponseJson(follower))
}

async fn create_issue_follower(
    State(deployment): State<DeploymentImpl>,
    Json(req): Json<CreateIssueFollowerRequest>,
) -> Result<ResponseJson<MutationResponse<IssueFollower>>, StatusCode> {
    let pool = &deployment.db().pool;
    let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());

    sqlx::query("INSERT INTO kanban_issue_followers (id, issue_id, user_id) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(&req.issue_id)
        .bind(&req.user_id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create issue follower: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let follower = sqlx::query_as::<_, IssueFollower>(
        "SELECT id, issue_id, user_id FROM kanban_issue_followers WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch created follower: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(ResponseJson(MutationResponse { data: follower, txid: 1 }))
}

async fn delete_issue_follower(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let pool = &deployment.db().pool;
    let result = sqlx::query("DELETE FROM kanban_issue_followers WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete issue follower: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}
