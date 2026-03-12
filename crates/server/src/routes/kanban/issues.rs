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
struct Issue {
    id: String,
    project_id: String,
    status_id: String,
    issue_number: i64,
    simple_id: String,
    title: String,
    description: Option<String>,
    priority: Option<String>,
    start_date: Option<String>,
    target_date: Option<String>,
    completed_at: Option<String>,
    sort_order: f64,
    parent_issue_id: Option<String>,
    parent_issue_sort_order: Option<f64>,
    extension_metadata: Option<String>,
    creator_user_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct CreateIssueRequest {
    id: Option<String>,
    project_id: String,
    status_id: String,
    title: String,
    description: Option<String>,
    priority: Option<String>,
    start_date: Option<String>,
    target_date: Option<String>,
    completed_at: Option<String>,
    sort_order: Option<f64>,
    parent_issue_id: Option<String>,
    parent_issue_sort_order: Option<f64>,
    extension_metadata: Option<serde_json::Value>,
    creator_user_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateIssueRequest {
    status_id: Option<String>,
    title: Option<String>,
    description: Option<String>,
    priority: Option<String>,
    start_date: Option<String>,
    target_date: Option<String>,
    completed_at: Option<String>,
    sort_order: Option<f64>,
    parent_issue_id: Option<String>,
    parent_issue_sort_order: Option<f64>,
    extension_metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct BulkUpdateIssuesRequest {
    updates: Vec<BulkUpdateIssueItem>,
}

#[derive(Debug, Deserialize)]
struct BulkUpdateIssueItem {
    id: String,
    status_id: Option<String>,
    title: Option<String>,
    description: Option<String>,
    priority: Option<String>,
    sort_order: Option<f64>,
    parent_issue_id: Option<String>,
    parent_issue_sort_order: Option<f64>,
    completed_at: Option<String>,
}

const ISSUE_COLUMNS: &str = "id, project_id, status_id, issue_number, simple_id, title, description, priority, start_date, target_date, completed_at, sort_order, parent_issue_id, parent_issue_sort_order, extension_metadata, creator_user_id, created_at, updated_at";

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/kanban/project/{project_id}/issues", get(list_issues))
        .route("/kanban/issues", post(create_issue))
        .route("/kanban/issues/bulk", post(bulk_update_issues))
        .route("/kanban/issues/{id}", get(get_issue).patch(update_issue).delete(delete_issue))
}

async fn list_issues(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<String>,
) -> Result<ResponseJson<Vec<Issue>>, StatusCode> {
    let pool = &deployment.db().pool;
    let query = format!(
        "SELECT {} FROM kanban_issues WHERE project_id = ? ORDER BY sort_order",
        ISSUE_COLUMNS
    );
    let issues = sqlx::query_as::<_, Issue>(&query)
        .bind(&project_id)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to list issues: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(ResponseJson(issues))
}

async fn get_issue(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<ResponseJson<Issue>, StatusCode> {
    let pool = &deployment.db().pool;
    let query = format!(
        "SELECT {} FROM kanban_issues WHERE id = ?",
        ISSUE_COLUMNS
    );
    let issue = sqlx::query_as::<_, Issue>(&query)
        .bind(&id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get issue: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(ResponseJson(issue))
}

async fn create_issue(
    State(deployment): State<DeploymentImpl>,
    Json(req): Json<CreateIssueRequest>,
) -> Result<ResponseJson<MutationResponse<Issue>>, StatusCode> {
    let pool = &deployment.db().pool;
    let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let sort_order = req.sort_order.unwrap_or(0.0);
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("Failed to begin transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Increment issue_counter and get new issue_number
    sqlx::query("UPDATE kanban_projects SET issue_counter = issue_counter + 1 WHERE id = ?")
        .bind(&req.project_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("Failed to increment issue counter: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    #[derive(FromRow)]
    struct CounterRow {
        issue_counter: i64,
    }

    let counter = sqlx::query_as::<_, CounterRow>(
        "SELECT issue_counter FROM kanban_projects WHERE id = ?",
    )
    .bind(&req.project_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get issue counter: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let issue_number = counter.issue_counter;

    // Get org's issue_prefix
    #[derive(FromRow)]
    struct PrefixRow {
        issue_prefix: String,
    }

    let prefix = sqlx::query_as::<_, PrefixRow>(
        "SELECT o.issue_prefix FROM kanban_organizations o \
         INNER JOIN kanban_projects p ON p.organization_id = o.id \
         WHERE p.id = ?",
    )
    .bind(&req.project_id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get issue prefix: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let simple_id = match prefix {
        Some(p) => format!("{}-{}", p.issue_prefix, issue_number),
        None => format!("ISS-{}", issue_number),
    };

    let extension_metadata = req
        .extension_metadata
        .map(|v| serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string()))
        .unwrap_or_else(|| "{}".to_string());

    sqlx::query(
        "INSERT INTO kanban_issues (id, project_id, status_id, issue_number, simple_id, title, description, priority, \
         start_date, target_date, completed_at, sort_order, parent_issue_id, parent_issue_sort_order, \
         extension_metadata, creator_user_id, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.project_id)
    .bind(&req.status_id)
    .bind(issue_number)
    .bind(&simple_id)
    .bind(&req.title)
    .bind(&req.description)
    .bind(&req.priority)
    .bind(&req.start_date)
    .bind(&req.target_date)
    .bind(&req.completed_at)
    .bind(sort_order)
    .bind(&req.parent_issue_id)
    .bind(req.parent_issue_sort_order)
    .bind(&extension_metadata)
    .bind(&req.creator_user_id)
    .bind(&now)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create issue: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tx.commit().await.map_err(|e| {
        tracing::error!("Failed to commit transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let query = format!("SELECT {} FROM kanban_issues WHERE id = ?", ISSUE_COLUMNS);
    let issue = sqlx::query_as::<_, Issue>(&query)
        .bind(&id)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch created issue: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(ResponseJson(MutationResponse { data: issue, txid: 1 }))
}

async fn update_issue(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
    Json(req): Json<UpdateIssueRequest>,
) -> Result<ResponseJson<MutationResponse<Issue>>, StatusCode> {
    let pool = &deployment.db().pool;
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    let query = format!("SELECT {} FROM kanban_issues WHERE id = ?", ISSUE_COLUMNS);
    let existing = sqlx::query_as::<_, Issue>(&query)
        .bind(&id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get issue: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let status_id = req.status_id.unwrap_or(existing.status_id);
    let title = req.title.unwrap_or(existing.title);
    let description = req.description.or(existing.description);
    let priority = req.priority.or(existing.priority);
    let start_date = req.start_date.or(existing.start_date);
    let target_date = req.target_date.or(existing.target_date);
    let completed_at = req.completed_at.or(existing.completed_at);
    let sort_order = req.sort_order.unwrap_or(existing.sort_order);
    let parent_issue_id = req.parent_issue_id.or(existing.parent_issue_id);
    let parent_issue_sort_order = req.parent_issue_sort_order.or(existing.parent_issue_sort_order);
    let extension_metadata = req
        .extension_metadata
        .map(|v| serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string()))
        .or(existing.extension_metadata);

    sqlx::query(
        "UPDATE kanban_issues SET status_id = ?, title = ?, description = ?, priority = ?, \
         start_date = ?, target_date = ?, completed_at = ?, sort_order = ?, parent_issue_id = ?, \
         parent_issue_sort_order = ?, extension_metadata = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&status_id)
    .bind(&title)
    .bind(&description)
    .bind(&priority)
    .bind(&start_date)
    .bind(&target_date)
    .bind(&completed_at)
    .bind(sort_order)
    .bind(&parent_issue_id)
    .bind(parent_issue_sort_order)
    .bind(&extension_metadata)
    .bind(&now)
    .bind(&id)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update issue: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let query = format!("SELECT {} FROM kanban_issues WHERE id = ?", ISSUE_COLUMNS);
    let issue = sqlx::query_as::<_, Issue>(&query)
        .bind(&id)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch updated issue: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(ResponseJson(MutationResponse { data: issue, txid: 1 }))
}

async fn delete_issue(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let pool = &deployment.db().pool;
    let result = sqlx::query("DELETE FROM kanban_issues WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete issue: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn bulk_update_issues(
    State(deployment): State<DeploymentImpl>,
    Json(req): Json<BulkUpdateIssuesRequest>,
) -> Result<ResponseJson<MutationResponse<Vec<Issue>>>, StatusCode> {
    let pool = &deployment.db().pool;
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("Failed to begin transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    for item in &req.updates {
        let query = format!("SELECT {} FROM kanban_issues WHERE id = ?", ISSUE_COLUMNS);
        let existing = sqlx::query_as::<_, Issue>(&query)
            .bind(&item.id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get issue: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?
            .ok_or(StatusCode::NOT_FOUND)?;

        let status_id = item.status_id.as_deref().unwrap_or(&existing.status_id);
        let title = item.title.as_deref().unwrap_or(&existing.title);
        let description = item.description.as_deref().or(existing.description.as_deref());
        let priority = item.priority.as_deref().or(existing.priority.as_deref());
        let sort_order = item.sort_order.unwrap_or(existing.sort_order);
        let parent_issue_id = item.parent_issue_id.as_deref().or(existing.parent_issue_id.as_deref());
        let parent_issue_sort_order = item.parent_issue_sort_order.or(existing.parent_issue_sort_order);
        let completed_at = item.completed_at.as_deref().or(existing.completed_at.as_deref());

        sqlx::query(
            "UPDATE kanban_issues SET status_id = ?, title = ?, description = ?, priority = ?, sort_order = ?, \
             parent_issue_id = ?, parent_issue_sort_order = ?, completed_at = ?, updated_at = ? WHERE id = ?",
        )
        .bind(status_id)
        .bind(title)
        .bind(description)
        .bind(priority)
        .bind(sort_order)
        .bind(parent_issue_id)
        .bind(parent_issue_sort_order)
        .bind(completed_at)
        .bind(&now)
        .bind(&item.id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("Failed to update issue: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("Failed to commit transaction: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut issues = Vec::new();
    let query = format!("SELECT {} FROM kanban_issues WHERE id = ?", ISSUE_COLUMNS);
    for item in &req.updates {
        let issue = sqlx::query_as::<_, Issue>(&query)
            .bind(&item.id)
            .fetch_one(pool)
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch updated issue: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            })?;
        issues.push(issue);
    }

    Ok(ResponseJson(MutationResponse { data: issues, txid: 1 }))
}
