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
struct IssueRelationship {
    id: String,
    issue_id: String,
    related_issue_id: String,
    relationship_type: String,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct CreateIssueRelationshipRequest {
    id: Option<String>,
    issue_id: String,
    related_issue_id: String,
    relationship_type: String,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/kanban/project/{project_id}/issue_relationships", get(list_issue_relationships))
        .route("/kanban/issue_relationships", post(create_issue_relationship))
        .route("/kanban/issue_relationships/{id}", get(get_issue_relationship).delete(delete_issue_relationship))
}

async fn list_issue_relationships(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<String>,
) -> Result<ResponseJson<Vec<IssueRelationship>>, StatusCode> {
    let pool = &deployment.db().pool;
    let relationships = sqlx::query_as::<_, IssueRelationship>(
        "SELECT r.id, r.issue_id, r.related_issue_id, r.relationship_type, r.created_at \
         FROM kanban_issue_relationships r \
         INNER JOIN kanban_issues i ON i.id = r.issue_id \
         WHERE i.project_id = ?",
    )
    .bind(&project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list issue relationships: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(ResponseJson(relationships))
}

async fn get_issue_relationship(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<ResponseJson<IssueRelationship>, StatusCode> {
    let pool = &deployment.db().pool;
    let relationship = sqlx::query_as::<_, IssueRelationship>(
        "SELECT id, issue_id, related_issue_id, relationship_type, created_at \
         FROM kanban_issue_relationships WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get issue relationship: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;
    Ok(ResponseJson(relationship))
}

async fn create_issue_relationship(
    State(deployment): State<DeploymentImpl>,
    Json(req): Json<CreateIssueRelationshipRequest>,
) -> Result<ResponseJson<MutationResponse<IssueRelationship>>, StatusCode> {
    let pool = &deployment.db().pool;
    let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    sqlx::query(
        "INSERT INTO kanban_issue_relationships (id, issue_id, related_issue_id, relationship_type, created_at) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.issue_id)
    .bind(&req.related_issue_id)
    .bind(&req.relationship_type)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create issue relationship: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let relationship = sqlx::query_as::<_, IssueRelationship>(
        "SELECT id, issue_id, related_issue_id, relationship_type, created_at \
         FROM kanban_issue_relationships WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch created relationship: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(ResponseJson(MutationResponse { data: relationship, txid: 1 }))
}

async fn delete_issue_relationship(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let pool = &deployment.db().pool;
    let result = sqlx::query("DELETE FROM kanban_issue_relationships WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete issue relationship: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}
