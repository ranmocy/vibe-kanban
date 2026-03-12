use deployment::Deployment;
use axum::{
    Router,
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use super::MutationResponse;
use crate::DeploymentImpl;

#[derive(Debug, Serialize, FromRow)]
struct Notification {
    id: String,
    organization_id: String,
    user_id: String,
    notification_type: String,
    payload: String,
    issue_id: Option<String>,
    comment_id: Option<String>,
    seen: bool,
    dismissed_at: Option<String>,
    created_at: String,
}

#[derive(Debug, Deserialize)]
struct ListNotificationsQuery {
    user_id: String,
}

#[derive(Debug, Deserialize)]
struct UpdateNotificationRequest {
    seen: Option<bool>,
    dismissed_at: Option<String>,
}

const NOTIF_COLUMNS: &str = "id, organization_id, user_id, notification_type, payload, issue_id, comment_id, seen, dismissed_at, created_at";

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/kanban/organizations/{org_id}/notifications", get(list_notifications))
        .route("/kanban/notifications/{id}", get(get_notification).patch(update_notification))
}

async fn list_notifications(
    State(deployment): State<DeploymentImpl>,
    Path(org_id): Path<String>,
    Query(query): Query<ListNotificationsQuery>,
) -> Result<ResponseJson<Vec<Notification>>, StatusCode> {
    let pool = &deployment.db().pool;
    let q = format!(
        "SELECT {} FROM kanban_notifications WHERE organization_id = ? AND user_id = ? ORDER BY created_at DESC",
        NOTIF_COLUMNS
    );
    let notifications = sqlx::query_as::<_, Notification>(&q)
        .bind(&org_id)
        .bind(&query.user_id)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to list notifications: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(ResponseJson(notifications))
}

async fn get_notification(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<ResponseJson<Notification>, StatusCode> {
    let pool = &deployment.db().pool;
    let q = format!(
        "SELECT {} FROM kanban_notifications WHERE id = ?",
        NOTIF_COLUMNS
    );
    let notification = sqlx::query_as::<_, Notification>(&q)
        .bind(&id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get notification: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(ResponseJson(notification))
}

async fn update_notification(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
    Json(req): Json<UpdateNotificationRequest>,
) -> Result<ResponseJson<MutationResponse<Notification>>, StatusCode> {
    let pool = &deployment.db().pool;

    let q = format!(
        "SELECT {} FROM kanban_notifications WHERE id = ?",
        NOTIF_COLUMNS
    );
    let existing = sqlx::query_as::<_, Notification>(&q)
        .bind(&id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get notification: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let seen = req.seen.unwrap_or(existing.seen);
    let dismissed_at = req.dismissed_at.or(existing.dismissed_at);

    sqlx::query("UPDATE kanban_notifications SET seen = ?, dismissed_at = ? WHERE id = ?")
        .bind(seen)
        .bind(&dismissed_at)
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to update notification: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let q = format!(
        "SELECT {} FROM kanban_notifications WHERE id = ?",
        NOTIF_COLUMNS
    );
    let notification = sqlx::query_as::<_, Notification>(&q)
        .bind(&id)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch updated notification: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(ResponseJson(MutationResponse { data: notification, txid: 1 }))
}
