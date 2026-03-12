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
struct IssueTag {
    id: String,
    issue_id: String,
    tag_id: String,
}

#[derive(Debug, Deserialize)]
struct CreateIssueTagRequest {
    id: Option<String>,
    issue_id: String,
    tag_id: String,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/kanban/project/{project_id}/issue_tags", get(list_issue_tags))
        .route("/kanban/issue_tags", post(create_issue_tag))
        .route("/kanban/issue_tags/{id}", get(get_issue_tag).delete(delete_issue_tag))
}

async fn list_issue_tags(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<String>,
) -> Result<ResponseJson<Vec<IssueTag>>, StatusCode> {
    let pool = &deployment.db().pool;
    let issue_tags = sqlx::query_as::<_, IssueTag>(
        "SELECT it.id, it.issue_id, it.tag_id \
         FROM kanban_issue_tags it \
         INNER JOIN kanban_issues i ON i.id = it.issue_id \
         WHERE i.project_id = ?",
    )
    .bind(&project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list issue tags: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(ResponseJson(issue_tags))
}

async fn get_issue_tag(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<ResponseJson<IssueTag>, StatusCode> {
    let pool = &deployment.db().pool;
    let issue_tag = sqlx::query_as::<_, IssueTag>(
        "SELECT id, issue_id, tag_id FROM kanban_issue_tags WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get issue tag: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;
    Ok(ResponseJson(issue_tag))
}

async fn create_issue_tag(
    State(deployment): State<DeploymentImpl>,
    Json(req): Json<CreateIssueTagRequest>,
) -> Result<ResponseJson<MutationResponse<IssueTag>>, StatusCode> {
    let pool = &deployment.db().pool;
    let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());

    sqlx::query("INSERT INTO kanban_issue_tags (id, issue_id, tag_id) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(&req.issue_id)
        .bind(&req.tag_id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create issue tag: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let issue_tag = sqlx::query_as::<_, IssueTag>(
        "SELECT id, issue_id, tag_id FROM kanban_issue_tags WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch created issue tag: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(ResponseJson(MutationResponse { data: issue_tag, txid: 1 }))
}

async fn delete_issue_tag(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let pool = &deployment.db().pool;
    let result = sqlx::query("DELETE FROM kanban_issue_tags WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete issue tag: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}
