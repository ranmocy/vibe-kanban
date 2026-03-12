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
struct PullRequest {
    id: String,
    url: String,
    number: i64,
    status: String,
    merged_at: Option<String>,
    merge_commit_sha: Option<String>,
    target_branch_name: String,
    issue_id: String,
    workspace_id: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct CreatePullRequestRequest {
    id: Option<String>,
    url: String,
    number: i64,
    status: Option<String>,
    merged_at: Option<String>,
    merge_commit_sha: Option<String>,
    target_branch_name: String,
    issue_id: String,
    workspace_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdatePullRequestRequest {
    url: Option<String>,
    number: Option<i64>,
    status: Option<String>,
    merged_at: Option<String>,
    merge_commit_sha: Option<String>,
    target_branch_name: Option<String>,
    workspace_id: Option<String>,
}

const PR_COLUMNS: &str = "id, url, number, status, merged_at, merge_commit_sha, target_branch_name, issue_id, workspace_id, created_at, updated_at";

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/kanban/project/{project_id}/pull_requests", get(list_pull_requests))
        .route("/kanban/pull_requests", post(create_pull_request))
        .route("/kanban/pull_requests/{id}", get(get_pull_request).patch(update_pull_request).delete(delete_pull_request))
}

async fn list_pull_requests(
    State(deployment): State<DeploymentImpl>,
    Path(project_id): Path<String>,
) -> Result<ResponseJson<Vec<PullRequest>>, StatusCode> {
    let pool = &deployment.db().pool;
    let query = format!(
        "SELECT pr.id, pr.url, pr.number, pr.status, pr.merged_at, pr.merge_commit_sha, \
         pr.target_branch_name, pr.issue_id, pr.workspace_id, pr.created_at, pr.updated_at \
         FROM kanban_pull_requests pr \
         INNER JOIN kanban_issues i ON i.id = pr.issue_id \
         WHERE i.project_id = ?"
    );
    let prs = sqlx::query_as::<_, PullRequest>(&query)
        .bind(&project_id)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to list pull requests: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(ResponseJson(prs))
}

async fn get_pull_request(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<ResponseJson<PullRequest>, StatusCode> {
    let pool = &deployment.db().pool;
    let query = format!(
        "SELECT {} FROM kanban_pull_requests WHERE id = ?",
        PR_COLUMNS
    );
    let pr = sqlx::query_as::<_, PullRequest>(&query)
        .bind(&id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get pull request: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(ResponseJson(pr))
}

async fn create_pull_request(
    State(deployment): State<DeploymentImpl>,
    Json(req): Json<CreatePullRequestRequest>,
) -> Result<ResponseJson<MutationResponse<PullRequest>>, StatusCode> {
    let pool = &deployment.db().pool;
    let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let status = req.status.unwrap_or_else(|| "open".to_string());
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    // Upsert: if url already exists, update it
    sqlx::query(
        "INSERT INTO kanban_pull_requests (id, url, number, status, merged_at, merge_commit_sha, \
         target_branch_name, issue_id, workspace_id, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(url) DO UPDATE SET \
         number = excluded.number, status = excluded.status, merged_at = excluded.merged_at, \
         merge_commit_sha = excluded.merge_commit_sha, target_branch_name = excluded.target_branch_name, \
         workspace_id = excluded.workspace_id, updated_at = excluded.updated_at",
    )
    .bind(&id)
    .bind(&req.url)
    .bind(req.number)
    .bind(&status)
    .bind(&req.merged_at)
    .bind(&req.merge_commit_sha)
    .bind(&req.target_branch_name)
    .bind(&req.issue_id)
    .bind(&req.workspace_id)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create/upsert pull request: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let query = format!(
        "SELECT {} FROM kanban_pull_requests WHERE url = ?",
        PR_COLUMNS
    );
    let pr = sqlx::query_as::<_, PullRequest>(&query)
        .bind(&req.url)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch created pull request: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(ResponseJson(MutationResponse { data: pr, txid: 1 }))
}

async fn update_pull_request(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
    Json(req): Json<UpdatePullRequestRequest>,
) -> Result<ResponseJson<MutationResponse<PullRequest>>, StatusCode> {
    let pool = &deployment.db().pool;
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    let query = format!(
        "SELECT {} FROM kanban_pull_requests WHERE id = ?",
        PR_COLUMNS
    );
    let existing = sqlx::query_as::<_, PullRequest>(&query)
        .bind(&id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get pull request: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let url = req.url.unwrap_or(existing.url);
    let number = req.number.unwrap_or(existing.number);
    let status = req.status.unwrap_or(existing.status);
    let merged_at = req.merged_at.or(existing.merged_at);
    let merge_commit_sha = req.merge_commit_sha.or(existing.merge_commit_sha);
    let target_branch_name = req.target_branch_name.unwrap_or(existing.target_branch_name);
    let workspace_id = req.workspace_id.or(existing.workspace_id);

    sqlx::query(
        "UPDATE kanban_pull_requests SET url = ?, number = ?, status = ?, merged_at = ?, \
         merge_commit_sha = ?, target_branch_name = ?, workspace_id = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&url)
    .bind(number)
    .bind(&status)
    .bind(&merged_at)
    .bind(&merge_commit_sha)
    .bind(&target_branch_name)
    .bind(&workspace_id)
    .bind(&now)
    .bind(&id)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to update pull request: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let query = format!(
        "SELECT {} FROM kanban_pull_requests WHERE id = ?",
        PR_COLUMNS
    );
    let pr = sqlx::query_as::<_, PullRequest>(&query)
        .bind(&id)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch updated pull request: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(ResponseJson(MutationResponse { data: pr, txid: 1 }))
}

async fn delete_pull_request(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    let pool = &deployment.db().pool;
    let result = sqlx::query("DELETE FROM kanban_pull_requests WHERE id = ?")
        .bind(&id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to delete pull request: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(StatusCode::NO_CONTENT)
}
