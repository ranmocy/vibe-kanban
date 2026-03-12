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
struct KanbanWorkspace {
    id: String,
    project_id: String,
    owner_user_id: String,
    issue_id: Option<String>,
    local_workspace_id: Option<String>,
    name: Option<String>,
    archived: bool,
    files_changed: Option<i64>,
    lines_added: Option<i64>,
    lines_removed: Option<i64>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct CreateKanbanWorkspaceRequest {
    id: Option<String>,
    project_id: String,
    owner_user_id: String,
    issue_id: Option<String>,
    local_workspace_id: Option<String>,
    name: Option<String>,
    archived: Option<bool>,
    files_changed: Option<i64>,
    lines_added: Option<i64>,
    lines_removed: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct UpdateKanbanWorkspaceRequest {
    issue_id: Option<String>,
    name: Option<String>,
    archived: Option<bool>,
    files_changed: Option<i64>,
    lines_added: Option<i64>,
    lines_removed: Option<i64>,
}

const WS_COLUMNS: &str = "id, project_id, owner_user_id, issue_id, local_workspace_id, name, archived, files_changed, lines_added, lines_removed, created_at, updated_at";

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/kanban/project/{project_id}/workspaces", get(list_workspaces))
        .route("/kanban/workspaces", post(create_workspace))
        .route("/kanban/workspaces/{id}", get(get_workspace).patch(update_workspace).delete(delete_workspace))
}

async fn list_workspaces(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<String>,
) -> Result<ResponseJson<Vec<KanbanWorkspace>>, StatusCode> {
    let pool = &deployment.db().pool;
    let query = format!(
        "SELECT {} FROM kanban_workspaces WHERE project_id = ?",
        WS_COLUMNS
    );
    let workspaces = sqlx::query_as::<_, KanbanWorkspace>(&query)
        .bind(&project_id)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to list workspaces: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(ResponseJson(workspaces))
}

async fn get_workspace(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<ResponseJson<KanbanWorkspace>, StatusCode> {
    let pool = &deployment.db().pool;
    let query = format!(
        "SELECT {} FROM kanban_workspaces WHERE id = ?",
        WS_COLUMNS
    );
    let workspace = sqlx::query_as::<_, KanbanWorkspace>(&query)
        .bind(&id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get workspace: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(ResponseJson(workspace))
}

async fn create_workspace(
    State(deployment): State<DeploymentImpl>,
    Json(req): Json<CreateKanbanWorkspaceRequest>,
) -> Result<ResponseJson<MutationResponse<KanbanWorkspace>>, StatusCode> {
    let pool = &deployment.db().pool;
    let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let archived = req.archived.unwrap_or(false);
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    sqlx::query(
        "INSERT INTO kanban_workspaces (id, project_id, owner_user_id, issue_id, local_workspace_id, \
         name, archived, files_changed, lines_added, lines_removed, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.project_id)
    .bind(&req.owner_user_id)
    .bind(&req.issue_id)
    .bind(&req.local_workspace_id)
    .bind(&req.name)
    .bind(archived)
    .bind(req.files_changed)
    .bind(req.lines_added)
    .bind(req.lines_removed)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create workspace: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let query = format!(
        "SELECT {} FROM kanban_workspaces WHERE id = ?",
        WS_COLUMNS
    );
    let workspace = sqlx::query_as::<_, KanbanWorkspace>(&query)
        .bind(&id)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch created workspace: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(ResponseJson(MutationResponse { data: workspace, txid: 1 }))
}

async fn update_workspace(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
    Json(req): Json<UpdateKanbanWorkspaceRequest>,
) -> Result<ResponseJson<MutationResponse<KanbanWorkspace>>, StatusCode> {
    let pool = &deployment.db().pool;
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    let query = format!(
        "SELECT {} FROM kanban_workspaces WHERE id = ?",
        WS_COLUMNS
    );
    let existing = sqlx::query_as::<_, KanbanWorkspace>(&query)
        .bind(&id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get workspace: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let issue_id = req.issue_id.or(existing.issue_id);
    let name = req.name.or(existing.name);
    let archived = req.archived.unwrap_or(existing.archived);
    let files_changed = req.files_changed.or(existing.files_changed);
    let lines_added = req.lines_added.or(existing.lines_added);
    let lines_removed = req.lines_removed.or(existing.lines_removed);

    sqlx::query(
        "UPDATE kanban_workspaces SET issue_id = ?, name = ?, archived = ?, \
         files_changed = ?, lines_added = ?, lines_removed = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&issue_id)
    .bind(&name)
    .bind(archived)
    .bind(files_changed)
    .bind(lines_added)
    .bind(lines_removed)
    .bind(&now)
    .bind(&id)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update workspace: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let query = format!(
        "SELECT {} FROM kanban_workspaces WHERE id = ?",
        WS_COLUMNS
    );
    let workspace = sqlx::query_as::<_, KanbanWorkspace>(&query)
        .bind(&id)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch updated workspace: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(ResponseJson(MutationResponse { data: workspace, txid: 1 }))
}

async fn delete_workspace(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let pool = &deployment.db().pool;
    let result = sqlx::query("DELETE FROM kanban_workspaces WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete workspace: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}
