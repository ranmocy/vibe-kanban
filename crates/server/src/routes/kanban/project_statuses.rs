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
struct ProjectStatus {
    id: String,
    project_id: String,
    name: String,
    color: String,
    sort_order: i64,
    hidden: bool,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct CreateProjectStatusRequest {
    id: Option<String>,
    project_id: String,
    name: String,
    color: Option<String>,
    sort_order: Option<i64>,
    hidden: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct UpdateProjectStatusRequest {
    name: Option<String>,
    color: Option<String>,
    sort_order: Option<i64>,
    hidden: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct BulkUpdateProjectStatusesRequest {
    updates: Vec<BulkUpdateProjectStatusItem>,
}

#[derive(Debug, Deserialize)]
struct BulkUpdateProjectStatusItem {
    id: String,
    name: Option<String>,
    color: Option<String>,
    sort_order: Option<i64>,
    hidden: Option<bool>,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/kanban/project/{project_id}/project_statuses", get(list_project_statuses))
        .route("/kanban/project_statuses", post(create_project_status))
        .route("/kanban/project_statuses/bulk", post(bulk_update_project_statuses))
        .route("/kanban/project_statuses/{id}", get(get_project_status).patch(update_project_status).delete(delete_project_status))
}

async fn list_project_statuses(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<String>,
) -> Result<ResponseJson<Vec<ProjectStatus>>, StatusCode> {
    let pool = &deployment.db().pool;
    let statuses = sqlx::query_as::<_, ProjectStatus>(
        "SELECT id, project_id, name, color, sort_order, hidden, created_at \
         FROM kanban_project_statuses WHERE project_id = ? ORDER BY sort_order",
    )
    .bind(&project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list project statuses: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(ResponseJson(statuses))
}

async fn get_project_status(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<ResponseJson<ProjectStatus>, StatusCode> {
    let pool = &deployment.db().pool;
    let status = sqlx::query_as::<_, ProjectStatus>(
        "SELECT id, project_id, name, color, sort_order, hidden, created_at \
         FROM kanban_project_statuses WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get project status: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;
    Ok(ResponseJson(status))
}

async fn create_project_status(
    State(deployment): State<DeploymentImpl>,
    Json(req): Json<CreateProjectStatusRequest>,
) -> Result<ResponseJson<MutationResponse<ProjectStatus>>, StatusCode> {
    let pool = &deployment.db().pool;
    let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let color = req.color.unwrap_or_else(|| "#6b7280".to_string());
    let sort_order = req.sort_order.unwrap_or(0);
    let hidden = req.hidden.unwrap_or(false);
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    sqlx::query(
        "INSERT INTO kanban_project_statuses (id, project_id, name, color, sort_order, hidden, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.project_id)
    .bind(&req.name)
    .bind(&color)
    .bind(sort_order)
    .bind(hidden)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create project status: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let status = sqlx::query_as::<_, ProjectStatus>(
        "SELECT id, project_id, name, color, sort_order, hidden, created_at \
         FROM kanban_project_statuses WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch created status: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(ResponseJson(MutationResponse { data: status, txid: 1 }))
}

async fn update_project_status(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
    Json(req): Json<UpdateProjectStatusRequest>,
) -> Result<ResponseJson<MutationResponse<ProjectStatus>>, StatusCode> {
    let pool = &deployment.db().pool;

    let existing = sqlx::query_as::<_, ProjectStatus>(
        "SELECT id, project_id, name, color, sort_order, hidden, created_at \
         FROM kanban_project_statuses WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get project status: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    let name = req.name.unwrap_or(existing.name);
    let color = req.color.unwrap_or(existing.color);
    let sort_order = req.sort_order.unwrap_or(existing.sort_order);
    let hidden = req.hidden.unwrap_or(existing.hidden);

    sqlx::query(
        "UPDATE kanban_project_statuses SET name = ?, color = ?, sort_order = ?, hidden = ? WHERE id = ?",
    )
    .bind(&name)
    .bind(&color)
    .bind(sort_order)
    .bind(hidden)
    .bind(&id)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update project status: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let status = sqlx::query_as::<_, ProjectStatus>(
        "SELECT id, project_id, name, color, sort_order, hidden, created_at \
         FROM kanban_project_statuses WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch updated status: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(ResponseJson(MutationResponse { data: status, txid: 1 }))
}

async fn delete_project_status(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let pool = &deployment.db().pool;
    let result = sqlx::query("DELETE FROM kanban_project_statuses WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete project status: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn bulk_update_project_statuses(
    State(deployment): State<DeploymentImpl>,
    Json(req): Json<BulkUpdateProjectStatusesRequest>,
) -> Result<ResponseJson<MutationResponse<Vec<ProjectStatus>>>, StatusCode> {
    let pool = &deployment.db().pool;
    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("Failed to begin transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    for item in &req.updates {
        let existing = sqlx::query_as::<_, ProjectStatus>(
            "SELECT id, project_id, name, color, sort_order, hidden, created_at \
             FROM kanban_project_statuses WHERE id = ?",
        )
        .bind(&item.id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get project status: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

        let name = item.name.as_deref().unwrap_or(&existing.name);
        let color = item.color.as_deref().unwrap_or(&existing.color);
        let sort_order = item.sort_order.unwrap_or(existing.sort_order);
        let hidden = item.hidden.unwrap_or(existing.hidden);

        sqlx::query(
            "UPDATE kanban_project_statuses SET name = ?, color = ?, sort_order = ?, hidden = ? WHERE id = ?",
        )
        .bind(name)
        .bind(color)
        .bind(sort_order)
        .bind(hidden)
        .bind(&item.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("Failed to update project status: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("Failed to commit transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Re-fetch all updated statuses
    let ids: Vec<&str> = req.updates.iter().map(|u| u.id.as_str()).collect();
    let mut statuses = Vec::new();
    for id in ids {
        let status = sqlx::query_as::<_, ProjectStatus>(
            "SELECT id, project_id, name, color, sort_order, hidden, created_at \
             FROM kanban_project_statuses WHERE id = ?",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch updated status: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        statuses.push(status);
    }

    Ok(ResponseJson(MutationResponse { data: statuses, txid: 1 }))
}
