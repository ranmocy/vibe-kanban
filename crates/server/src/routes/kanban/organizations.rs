use deployment::Deployment;
use axum::{
    Router,
    extract::{Json, Path, State},
    http::StatusCode,
    response::Json as ResponseJson,
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use super::MutationResponse;
use crate::DeploymentImpl;

#[derive(Debug, Serialize, FromRow)]
struct Organization {
    id: String,
    name: String,
    slug: String,
    is_personal: bool,
    issue_prefix: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct CreateOrganizationRequest {
    id: Option<String>,
    name: String,
    slug: String,
    is_personal: Option<bool>,
    issue_prefix: Option<String>,
}

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/kanban/organizations", get(list_organizations).post(create_organization))
        .route("/kanban/organizations/{id}", get(get_organization))
        .route("/kanban/organizations/{org_id}/organization_members", get(list_organization_members))
        .route("/kanban/organizations/{org_id}/members", get(list_members_with_profiles))
}

async fn list_organizations(
    State(deployment): State<DeploymentImpl>,
) -> Result<ResponseJson<Vec<Organization>>, StatusCode> {
    let pool = &deployment.db().pool;
    let orgs = sqlx::query_as::<_, Organization>("SELECT id, name, slug, is_personal, issue_prefix, created_at, updated_at FROM kanban_organizations")
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to list organizations: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(ResponseJson(orgs))
}

async fn get_organization(
    State(deployment): State<DeploymentImpl>,
    Path(id): Path<String>,
) -> Result<ResponseJson<Organization>, StatusCode> {
    let pool = &deployment.db().pool;
    let org = sqlx::query_as::<_, Organization>(
        "SELECT id, name, slug, is_personal, issue_prefix, created_at, updated_at FROM kanban_organizations WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get organization: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;
    Ok(ResponseJson(org))
}

async fn create_organization(
    State(deployment): State<DeploymentImpl>,
    Json(req): Json<CreateOrganizationRequest>,
) -> Result<ResponseJson<MutationResponse<Organization>>, StatusCode> {
    let pool = &deployment.db().pool;
    let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let is_personal = req.is_personal.unwrap_or(false);
    let issue_prefix = req.issue_prefix.unwrap_or_else(|| "ISS".to_string());
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    sqlx::query(
        "INSERT INTO kanban_organizations (id, name, slug, is_personal, issue_prefix, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.name)
    .bind(&req.slug)
    .bind(is_personal)
    .bind(&issue_prefix)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create organization: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let org = sqlx::query_as::<_, Organization>(
        "SELECT id, name, slug, is_personal, issue_prefix, created_at, updated_at FROM kanban_organizations WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch created organization: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(ResponseJson(MutationResponse { data: org, txid: 1 }))
}

#[derive(Debug, Serialize, FromRow)]
struct OrganizationMember {
    organization_id: String,
    user_id: String,
    role: String,
    joined_at: String,
    last_seen_at: Option<String>,
}

async fn list_organization_members(
    State(deployment): State<DeploymentImpl>,
    Path(org_id): Path<String>,
) -> Result<ResponseJson<Vec<OrganizationMember>>, StatusCode> {
    let pool = &deployment.db().pool;
    let members = sqlx::query_as::<_, OrganizationMember>(
        "SELECT organization_id, user_id, role, joined_at, last_seen_at \
         FROM kanban_organization_members WHERE organization_id = ?",
    )
    .bind(&org_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list organization members: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(ResponseJson(members))
}

#[derive(Debug, Serialize, FromRow)]
struct MemberWithProfile {
    user_id: String,
    role: String,
    joined_at: String,
    first_name: Option<String>,
    last_name: Option<String>,
    username: Option<String>,
    email: Option<String>,
}

async fn list_members_with_profiles(
    State(deployment): State<DeploymentImpl>,
    Path(org_id): Path<String>,
) -> Result<ResponseJson<Vec<MemberWithProfile>>, StatusCode> {
    let pool = &deployment.db().pool;
    let members = sqlx::query_as::<_, MemberWithProfile>(
        "SELECT m.user_id, m.role, m.joined_at, \
         u.first_name, u.last_name, u.username, u.email \
         FROM kanban_organization_members m \
         LEFT JOIN kanban_users u ON m.user_id = u.id \
         WHERE m.organization_id = ?",
    )
    .bind(&org_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list members with profiles: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(ResponseJson(members))
}
