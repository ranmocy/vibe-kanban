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
struct Tag {
    id: String,
    project_id: String,
    name: String,
    color: String,
}

#[derive(Debug, Deserialize)]
struct CreateTagRequest {
    id: Option<String>,
    project_id: String,
    name: String,
    color: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateTagRequest {
    name: Option<String>,
    color: Option<String>,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/kanban/project/{project_id}/tags", get(list_tags))
        .route("/kanban/tags", post(create_tag))
        .route("/kanban/tags/{id}", get(get_tag).patch(update_tag).delete(delete_tag))
}

async fn list_tags(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<String>,
) -> Result<ResponseJson<Vec<Tag>>, StatusCode> {
    let pool = &deployment.db().pool;
    let tags = sqlx::query_as::<_, Tag>(
        "SELECT id, project_id, name, color FROM kanban_tags WHERE project_id = ?",
    )
    .bind(&project_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list tags: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(ResponseJson(tags))
}

async fn get_tag(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<ResponseJson<Tag>, StatusCode> {
    let pool = &deployment.db().pool;
    let tag = sqlx::query_as::<_, Tag>(
        "SELECT id, project_id, name, color FROM kanban_tags WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get tag: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;
    Ok(ResponseJson(tag))
}

async fn create_tag(
    State(deployment): State<DeploymentImpl>,
    Json(req): Json<CreateTagRequest>,
) -> Result<ResponseJson<MutationResponse<Tag>>, StatusCode> {
    let pool = &deployment.db().pool;
    let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let color = req.color.unwrap_or_else(|| "#6b7280".to_string());

    sqlx::query("INSERT INTO kanban_tags (id, project_id, name, color) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(&req.project_id)
        .bind(&req.name)
        .bind(&color)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create tag: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let tag = sqlx::query_as::<_, Tag>(
        "SELECT id, project_id, name, color FROM kanban_tags WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch created tag: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(ResponseJson(MutationResponse { data: tag, txid: 1 }))
}

async fn update_tag(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
    Json(req): Json<UpdateTagRequest>,
) -> Result<ResponseJson<MutationResponse<Tag>>, StatusCode> {
    let pool = &deployment.db().pool;

    let existing = sqlx::query_as::<_, Tag>(
        "SELECT id, project_id, name, color FROM kanban_tags WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get tag: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    let name = req.name.unwrap_or(existing.name);
    let color = req.color.unwrap_or(existing.color);

    sqlx::query("UPDATE kanban_tags SET name = ?, color = ? WHERE id = ?")
        .bind(&name)
        .bind(&color)
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to update tag: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let tag = sqlx::query_as::<_, Tag>(
        "SELECT id, project_id, name, color FROM kanban_tags WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch updated tag: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(ResponseJson(MutationResponse { data: tag, txid: 1 }))
}

async fn delete_tag(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let pool = &deployment.db().pool;
    let result = sqlx::query("DELETE FROM kanban_tags WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete tag: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}
