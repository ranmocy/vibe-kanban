use deployment::Deployment;
use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::get,
};
use serde::Serialize;
use sqlx::FromRow;

use crate::DeploymentImpl;

#[derive(Debug, Serialize, FromRow)]
struct User {
    id: String,
    email: String,
    first_name: Option<String>,
    last_name: Option<String>,
    username: Option<String>,
    created_at: String,
    updated_at: String,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/kanban/organizations/{org_id}/users", get(list_users))
}

async fn list_users(
    State(deployment): State<DeploymentImpl>,
    Path(org_id): Path<String>,
) -> Result<ResponseJson<Vec<User>>, StatusCode> {
    let pool = &deployment.db().pool;
    let users = sqlx::query_as::<_, User>(
        "SELECT u.id, u.email, u.first_name, u.last_name, u.username, u.created_at, u.updated_at \
         FROM kanban_users u \
         INNER JOIN kanban_organization_members m ON m.user_id = u.id \
         WHERE m.organization_id = ?",
    )
    .bind(&org_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list users: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(ResponseJson(users))
}
