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
struct Project {
    id: String,
    organization_id: String,
    name: String,
    color: String,
    issue_counter: i64,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct CreateProjectRequest {
    id: Option<String>,
    organization_id: String,
    name: String,
    color: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateProjectRequest {
    name: Option<String>,
    color: Option<String>,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/kanban/organizations/{org_id}/projects", get(list_projects))
        .route("/kanban/projects", post(create_project))
        .route("/kanban/projects/{id}", get(get_project).patch(update_project).delete(delete_project))
}

async fn list_projects(
    State(deployment): State<DeploymentImpl>,
    Path(org_id): Path<String>,
) -> Result<ResponseJson<Vec<Project>>, StatusCode> {
    let pool = &deployment.db().pool;
    let projects = sqlx::query_as::<_, Project>(
        "SELECT id, organization_id, name, color, issue_counter, created_at, updated_at \
         FROM kanban_projects WHERE organization_id = ?",
    )
    .bind(&org_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list projects: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(ResponseJson(projects))
}

async fn get_project(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<ResponseJson<Project>, StatusCode> {
    let pool = &deployment.db().pool;
    let project = sqlx::query_as::<_, Project>(
        "SELECT id, organization_id, name, color, issue_counter, created_at, updated_at \
         FROM kanban_projects WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get project: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;
    Ok(ResponseJson(project))
}

async fn create_project(
    State(deployment): State<DeploymentImpl>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<ResponseJson<MutationResponse<Project>>, StatusCode> {
    let pool = &deployment.db().pool;
    let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let color = req.color.unwrap_or_else(|| "#6366f1".to_string());
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    sqlx::query(
        "INSERT INTO kanban_projects (id, organization_id, name, color, issue_counter, created_at, updated_at) \
         VALUES (?, ?, ?, ?, 0, ?, ?)",
    )
    .bind(&id)
    .bind(&req.organization_id)
    .bind(&req.name)
    .bind(&color)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create project: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let project = sqlx::query_as::<_, Project>(
        "SELECT id, organization_id, name, color, issue_counter, created_at, updated_at \
         FROM kanban_projects WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch created project: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(ResponseJson(MutationResponse { data: project, txid: 1 }))
}

async fn update_project(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
    Json(req): Json<UpdateProjectRequest>,
) -> Result<ResponseJson<MutationResponse<Project>>, StatusCode> {
    let pool = &deployment.db().pool;
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    let existing = sqlx::query_as::<_, Project>(
        "SELECT id, organization_id, name, color, issue_counter, created_at, updated_at \
         FROM kanban_projects WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get project: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    let name = req.name.unwrap_or(existing.name);
    let color = req.color.unwrap_or(existing.color);

    sqlx::query("UPDATE kanban_projects SET name = ?, color = ?, updated_at = ? WHERE id = ?")
        .bind(&name)
        .bind(&color)
        .bind(&now)
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to update project: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let project = sqlx::query_as::<_, Project>(
        "SELECT id, organization_id, name, color, issue_counter, created_at, updated_at \
         FROM kanban_projects WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch updated project: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(ResponseJson(MutationResponse { data: project, txid: 1 }))
}

async fn delete_project(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let pool = &deployment.db().pool;
    let result = sqlx::query("DELETE FROM kanban_projects WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete project: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}
