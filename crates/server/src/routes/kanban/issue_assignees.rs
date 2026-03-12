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
struct IssueAssignee {
    id: String,
    issue_id: String,
    user_id: String,
    assigned_at: String,
}

#[derive(Debug, Deserialize)]
struct CreateIssueAssigneeRequest {
    id: Option<String>,
    issue_id: String,
    user_id: String,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/kanban/project/{project_id}/issue_assignees", get(list_issue_assignees))
        .route("/kanban/issue_assignees", post(create_issue_assignee))
        .route("/kanban/issue_assignees/{id}", get(get_issue_assignee).delete(delete_issue_assignee))
}

async fn list_issue_assignees(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<String>,
) -> Result<ResponseJson<Vec<IssueAssignee>>, StatusCode> {
    let pool = &deployment.db().pool;
    let assignees = sqlx::query_as::<_, IssueAssignee>(
        "SELECT a.id, a.issue_id, a.user_id, a.assigned_at \
         FROM kanban_issue_assignees a \
         INNER JOIN kanban_issues i ON i.id = a.issue_id \
         WHERE i.project_id = ?",
    )
    .bind(&project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list issue assignees: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(ResponseJson(assignees))
}

async fn get_issue_assignee(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<ResponseJson<IssueAssignee>, StatusCode> {
    let pool = &deployment.db().pool;
    let assignee = sqlx::query_as::<_, IssueAssignee>(
        "SELECT id, issue_id, user_id, assigned_at FROM kanban_issue_assignees WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get issue assignee: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;
    Ok(ResponseJson(assignee))
}

async fn create_issue_assignee(
    State(deployment): State<DeploymentImpl>,
    Json(req): Json<CreateIssueAssigneeRequest>,
) -> Result<ResponseJson<MutationResponse<IssueAssignee>>, StatusCode> {
    let pool = &deployment.db().pool;
    let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    sqlx::query(
        "INSERT INTO kanban_issue_assignees (id, issue_id, user_id, assigned_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.issue_id)
    .bind(&req.user_id)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create issue assignee: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let assignee = sqlx::query_as::<_, IssueAssignee>(
        "SELECT id, issue_id, user_id, assigned_at FROM kanban_issue_assignees WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch created assignee: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(ResponseJson(MutationResponse { data: assignee, txid: 1 }))
}

async fn delete_issue_assignee(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let pool = &deployment.db().pool;
    let result = sqlx::query("DELETE FROM kanban_issue_assignees WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete issue assignee: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}
