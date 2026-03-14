use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use axum::{Extension, Json, extract::State, response::Json as ResponseJson};
use db::models::{
    coding_agent_turn::CodingAgentTurn,
    execution_process::{ExecutionProcess, ExecutionProcessStatus},
    merge::{Merge, MergeStatus},
    workspace::Workspace,
};
use deployment::Deployment;
use futures_util::stream::{self, StreamExt};
use moka::future::Cache;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

/// Request for fetching workspace summaries
#[derive(Debug, Deserialize, Serialize, TS)]
pub struct WorkspaceSummaryRequest {
    pub archived: bool,
}

/// Summary info for a single workspace
#[derive(Debug, Clone, Serialize, TS)]
pub struct WorkspaceSummary {
    pub workspace_id: Uuid,
    /// Session ID of the latest execution process
    pub latest_session_id: Option<Uuid>,
    /// Is a tool approval currently pending?
    pub has_pending_approval: bool,
    /// Number of files with changes
    pub files_changed: Option<usize>,
    /// Total lines added across all files
    pub lines_added: Option<usize>,
    /// Total lines removed across all files
    pub lines_removed: Option<usize>,
    /// When the latest execution process completed
    #[ts(optional)]
    pub latest_process_completed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Status of the latest execution process
    pub latest_process_status: Option<ExecutionProcessStatus>,
    /// Is a dev server currently running?
    pub has_running_dev_server: bool,
    /// Does this workspace have unseen coding agent turns?
    pub has_unseen_turns: bool,
    /// PR status for this workspace (if any PR exists)
    pub pr_status: Option<MergeStatus>,
}

/// Response containing summaries for requested workspaces
#[derive(Debug, Clone, Serialize, TS)]
pub struct WorkspaceSummaryResponse {
    pub summaries: Vec<WorkspaceSummary>,
}

#[derive(Debug, Clone, Default, Serialize, TS)]
pub struct DiffStats {
    pub files_changed: usize,
    pub lines_added: usize,
    pub lines_removed: usize,
}

/// Workspace summary cache keyed on `archived` (bool).
/// Holds up to 2 entries — one for `archived=false`, one for `archived=true`.
/// Each entry expires after 5 seconds.
#[derive(Clone)]
pub struct SummaryCache {
    inner: Cache<bool, Arc<WorkspaceSummaryResponse>>,
}

impl SummaryCache {
    pub fn new() -> Self {
        Self {
            inner: Cache::builder()
                .max_capacity(2)
                .time_to_live(Duration::from_secs(5))
                .build(),
        }
    }
}

impl Default for SummaryCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Fetch summary information for workspaces filtered by archived status.
/// This endpoint returns data that cannot be efficiently included in the streaming endpoint.
#[axum::debug_handler(state = DeploymentImpl)]
pub async fn get_workspace_summaries(
    State(deployment): State<DeploymentImpl>,
    Extension(cache): Extension<SummaryCache>,
    Json(request): Json<WorkspaceSummaryRequest>,
) -> Result<ResponseJson<ApiResponse<WorkspaceSummaryResponse>>, ApiError> {
    let archived = request.archived;

    // Fast path: return cached response if still within TTL.
    if let Some(cached) = cache.inner.get(&archived).await {
        return Ok(ResponseJson(ApiResponse::success((*cached).clone())));
    }

    let response = compute_workspace_summary(&deployment, archived).await?;
    let arc = Arc::new(response.clone());
    cache.inner.insert(archived, arc).await;

    Ok(ResponseJson(ApiResponse::success(response)))
}

/// Core computation for workspace summaries, separated from the handler so it can
/// be called without going through the cache (e.g. in tests).
async fn compute_workspace_summary(
    deployment: &DeploymentImpl,
    archived: bool,
) -> Result<WorkspaceSummaryResponse, ApiError> {
    let pool = &deployment.db().pool;

    // 1. Fetch all workspaces with the given archived status
    let workspaces: Vec<Workspace> = Workspace::find_all_with_status(pool, Some(archived), None)
        .await?
        .into_iter()
        .map(|ws| ws.workspace)
        .collect();

    if workspaces.is_empty() {
        return Ok(WorkspaceSummaryResponse { summaries: vec![] });
    }

    // 2. Fetch latest process info for workspaces with this archived status
    let latest_processes = ExecutionProcess::find_latest_for_workspaces(pool, archived).await?;

    // 3. Check which workspaces have running dev servers
    let dev_server_workspaces =
        ExecutionProcess::find_workspaces_with_running_dev_servers(pool, archived).await?;

    // 4. Check pending approvals for running processes
    let running_ep_ids: Vec<_> = latest_processes
        .values()
        .filter(|info| info.status == ExecutionProcessStatus::Running)
        .map(|info| info.execution_process_id)
        .collect();
    let pending_approval_eps = deployment
        .approvals()
        .get_pending_execution_process_ids(&running_ep_ids);

    // 5. Check which workspaces have unseen coding agent turns
    let unseen_workspaces = CodingAgentTurn::find_workspaces_with_unseen(pool, archived).await?;

    // 6. Get PR status for each workspace
    let pr_statuses = Merge::get_latest_pr_status_for_workspaces(pool, archived).await?;

    // 7. Compute diff stats for each workspace with bounded parallelism (max 10 in flight).
    let deployment_ref = deployment;
    let diff_futs: Vec<_> = workspaces
        .iter()
        .filter(|ws| ws.container_ref.is_some())
        .map(|ws| {
            let workspace = ws.clone();
            let dep = deployment_ref.clone();
            async move {
                compute_workspace_diff_stats(&dep, &workspace)
                    .await
                    .map(|stats| (workspace.id, stats))
            }
        })
        .collect();
    let diff_stats: HashMap<Uuid, DiffStats> = stream::iter(diff_futs)
        .buffer_unordered(10)
        .filter_map(|opt| async move { opt })
        .collect()
        .await;

    // 8. Assemble response
    let summaries: Vec<WorkspaceSummary> = workspaces
        .iter()
        .map(|ws| {
            let id = ws.id;
            let latest = latest_processes.get(&id);
            let has_pending = latest
                .map(|p| pending_approval_eps.contains(&p.execution_process_id))
                .unwrap_or(false);
            let stats = diff_stats.get(&id);

            WorkspaceSummary {
                workspace_id: id,
                latest_session_id: latest.map(|p| p.session_id),
                has_pending_approval: has_pending,
                files_changed: stats.map(|s| s.files_changed),
                lines_added: stats.map(|s| s.lines_added),
                lines_removed: stats.map(|s| s.lines_removed),
                latest_process_completed_at: latest.and_then(|p| p.completed_at),
                latest_process_status: latest.map(|p| p.status.clone()),
                has_running_dev_server: dev_server_workspaces.contains(&id),
                has_unseen_turns: unseen_workspaces.contains(&id),
                pr_status: pr_statuses.get(&id).cloned(),
            }
        })
        .collect();

    Ok(WorkspaceSummaryResponse { summaries })
}

/// Compute diff stats for a workspace.
pub async fn compute_workspace_diff_stats(
    deployment: &DeploymentImpl,
    workspace: &Workspace,
) -> Option<DiffStats> {
    let stats = services::services::diff_stream::compute_diff_stats(
        &deployment.db().pool,
        deployment.git(),
        workspace,
    )
    .await?;

    Some(DiffStats {
        files_changed: stats.files_changed,
        lines_added: stats.lines_added,
        lines_removed: stats.lines_removed,
    })
}
