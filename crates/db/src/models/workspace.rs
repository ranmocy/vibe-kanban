use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use thiserror::Error;
use ts_rs::TS;
use uuid::Uuid;

/// Maximum length for auto-generated workspace names (derived from first user prompt)
const WORKSPACE_NAME_MAX_LEN: usize = 60;

use super::{
    project::Project,
    task::Task,
    workspace_repo::{RepoWithTargetBranch, WorkspaceRepo},
};

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("Task not found")]
    TaskNotFound,
    #[error("Project not found")]
    ProjectNotFound,
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Branch not found: {0}")]
    BranchNotFound(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct ContainerInfo {
    pub workspace_id: Uuid,
    pub task_id: Uuid,
    pub project_id: Uuid,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize, TS)]
pub struct Workspace {
    pub id: Uuid,
    pub task_id: Uuid,
    pub container_ref: Option<String>,
    pub branch: String,
    pub agent_working_dir: Option<String>,
    pub setup_completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived: bool,
    pub pinned: bool,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct WorkspaceWithStatus {
    #[serde(flatten)]
    #[ts(flatten)]
    pub workspace: Workspace,
    pub is_running: bool,
    pub is_errored: bool,
}

impl std::ops::Deref for WorkspaceWithStatus {
    type Target = Workspace;
    fn deref(&self) -> &Self::Target {
        &self.workspace
    }
}

/// GitHub PR creation parameters
pub struct CreatePrParams<'a> {
    pub workspace_id: Uuid,
    pub task_id: Uuid,
    pub project_id: Uuid,
    pub github_token: &'a str,
    pub title: &'a str,
    pub body: Option<&'a str>,
    pub base_branch: Option<&'a str>,
}

#[derive(Debug, Deserialize, TS)]
pub struct CreateFollowUpAttempt {
    pub prompt: String,
}

/// Context data for resume operations (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttemptResumeContext {
    pub execution_history: String,
    pub cumulative_diffs: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceContext {
    pub workspace: Workspace,
    pub task: Task,
    pub project: Project,
    pub workspace_repos: Vec<RepoWithTargetBranch>,
}

#[derive(Debug, Deserialize, TS)]
pub struct CreateWorkspace {
    pub branch: String,
    pub agent_working_dir: Option<String>,
}

impl Workspace {
    pub async fn parent_task(&self, pool: &SqlitePool) -> Result<Option<Task>, sqlx::Error> {
        Task::find_by_id(pool, self.task_id).await
    }

    /// Fetch all workspaces, optionally filtered by task_id. Newest first.
    pub async fn fetch_all(
        pool: &SqlitePool,
        task_id: Option<Uuid>,
    ) -> Result<Vec<Self>, WorkspaceError> {
        let workspaces = match task_id {
            Some(tid) => sqlx::query_as!(
                Workspace,
                r#"SELECT id AS "id!: Uuid",
                              task_id AS "task_id!: Uuid",
                              container_ref,
                              branch,
                              agent_working_dir,
                              setup_completed_at AS "setup_completed_at: DateTime<Utc>",
                              created_at AS "created_at!: DateTime<Utc>",
                              updated_at AS "updated_at!: DateTime<Utc>",
                              archived AS "archived!: bool",
                              pinned AS "pinned!: bool",
                              name
                       FROM workspaces
                       WHERE task_id = $1
                       ORDER BY created_at DESC"#,
                tid
            )
            .fetch_all(pool)
            .await
            .map_err(WorkspaceError::Database)?,
            None => sqlx::query_as!(
                Workspace,
                r#"SELECT id AS "id!: Uuid",
                              task_id AS "task_id!: Uuid",
                              container_ref,
                              branch,
                              agent_working_dir,
                              setup_completed_at AS "setup_completed_at: DateTime<Utc>",
                              created_at AS "created_at!: DateTime<Utc>",
                              updated_at AS "updated_at!: DateTime<Utc>",
                              archived AS "archived!: bool",
                              pinned AS "pinned!: bool",
                              name
                       FROM workspaces
                       ORDER BY created_at DESC"#
            )
            .fetch_all(pool)
            .await
            .map_err(WorkspaceError::Database)?,
        };

        Ok(workspaces)
    }

    /// Load full workspace context by workspace ID.
    pub async fn load_context(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<WorkspaceContext, WorkspaceError> {
        let workspace = Workspace::find_by_id(pool, workspace_id)
            .await?
            .ok_or(WorkspaceError::TaskNotFound)?;

        let task = Task::find_by_id(pool, workspace.task_id)
            .await?
            .ok_or(WorkspaceError::TaskNotFound)?;

        let project = Project::find_by_id(pool, task.project_id)
            .await?
            .ok_or(WorkspaceError::ProjectNotFound)?;

        let workspace_repos =
            WorkspaceRepo::find_repos_with_target_branch_for_workspace(pool, workspace_id).await?;

        Ok(WorkspaceContext {
            workspace,
            task,
            project,
            workspace_repos,
        })
    }

    /// Update container reference
    pub async fn update_container_ref(
        pool: &SqlitePool,
        workspace_id: Uuid,
        container_ref: &str,
    ) -> Result<(), sqlx::Error> {
        let now = Utc::now();
        sqlx::query!(
            "UPDATE workspaces SET container_ref = $1, updated_at = $2 WHERE id = $3",
            container_ref,
            now,
            workspace_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn clear_container_ref(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE workspaces SET container_ref = NULL, updated_at = datetime('now') WHERE id = ?",
            workspace_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Update the workspace's updated_at timestamp to prevent cleanup.
    /// Call this when the workspace is accessed (e.g., opened in editor).
    pub async fn touch(pool: &SqlitePool, workspace_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE workspaces SET updated_at = datetime('now', 'subsec') WHERE id = ?",
            workspace_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn find_by_id(pool: &SqlitePool, id: Uuid) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Workspace,
            r#"SELECT  id                AS "id!: Uuid",
                       task_id           AS "task_id!: Uuid",
                       container_ref,
                       branch,
                       agent_working_dir,
                       setup_completed_at AS "setup_completed_at: DateTime<Utc>",
                       created_at        AS "created_at!: DateTime<Utc>",
                       updated_at        AS "updated_at!: DateTime<Utc>",
                       archived          AS "archived!: bool",
                       pinned            AS "pinned!: bool",
                       name
               FROM    workspaces
               WHERE   id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn find_by_rowid(pool: &SqlitePool, rowid: i64) -> Result<Option<Self>, sqlx::Error> {
        sqlx::query_as!(
            Workspace,
            r#"SELECT  id                AS "id!: Uuid",
                       task_id           AS "task_id!: Uuid",
                       container_ref,
                       branch,
                       agent_working_dir,
                       setup_completed_at AS "setup_completed_at: DateTime<Utc>",
                       created_at        AS "created_at!: DateTime<Utc>",
                       updated_at        AS "updated_at!: DateTime<Utc>",
                       archived          AS "archived!: bool",
                       pinned            AS "pinned!: bool",
                       name
               FROM    workspaces
               WHERE   rowid = $1"#,
            rowid
        )
        .fetch_optional(pool)
        .await
    }

    pub async fn container_ref_exists(
        pool: &SqlitePool,
        container_ref: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"SELECT EXISTS(SELECT 1 FROM workspaces WHERE container_ref = ?) as "exists!: bool""#,
            container_ref
        )
        .fetch_one(pool)
        .await?;

        Ok(result.exists)
    }

    /// Find workspaces that are expired and eligible for cleanup.
    /// Uses accelerated cleanup (1 hour) for archived workspaces OR tasks not in progress/review.
    /// Uses standard cleanup (72 hours) only for non-archived workspaces on active tasks.
    pub async fn find_expired_for_cleanup(
        pool: &SqlitePool,
    ) -> Result<Vec<Workspace>, sqlx::Error> {
        sqlx::query_as!(
            Workspace,
            r#"
            SELECT
                w.id as "id!: Uuid",
                w.task_id as "task_id!: Uuid",
                w.container_ref,
                w.branch as "branch!",
                w.agent_working_dir,
                w.setup_completed_at as "setup_completed_at: DateTime<Utc>",
                w.created_at as "created_at!: DateTime<Utc>",
                w.updated_at as "updated_at!: DateTime<Utc>",
                w.archived as "archived!: bool",
                w.pinned as "pinned!: bool",
                w.name
            FROM workspaces w
            JOIN tasks t ON w.task_id = t.id
            LEFT JOIN sessions s ON w.id = s.workspace_id
            LEFT JOIN execution_processes ep ON s.id = ep.session_id AND ep.completed_at IS NOT NULL
            WHERE w.container_ref IS NOT NULL
                AND w.id NOT IN (
                    SELECT DISTINCT s2.workspace_id
                    FROM sessions s2
                    JOIN execution_processes ep2 ON s2.id = ep2.session_id
                    WHERE ep2.completed_at IS NULL
                )
            GROUP BY w.id, w.container_ref, w.updated_at
            HAVING datetime('now', 'localtime',
                CASE
                    WHEN w.archived = 1 OR t.status NOT IN ('inprogress', 'inreview')
                    THEN '-1 hours'
                    ELSE '-72 hours'
                END
            ) > datetime(
                MAX(
                    max(
                        datetime(w.updated_at),
                        datetime(ep.completed_at)
                    )
                )
            )
            ORDER BY MAX(
                CASE
                    WHEN ep.completed_at IS NOT NULL THEN ep.completed_at
                    ELSE w.updated_at
                END
            ) ASC
            "#
        )
        .fetch_all(pool)
        .await
    }

    pub async fn create(
        pool: &SqlitePool,
        data: &CreateWorkspace,
        id: Uuid,
        task_id: Uuid,
    ) -> Result<Self, WorkspaceError> {
        Ok(sqlx::query_as!(
            Workspace,
            r#"INSERT INTO workspaces (id, task_id, container_ref, branch, agent_working_dir, setup_completed_at)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id as "id!: Uuid", task_id as "task_id!: Uuid", container_ref, branch, agent_working_dir, setup_completed_at as "setup_completed_at: DateTime<Utc>", created_at as "created_at!: DateTime<Utc>", updated_at as "updated_at!: DateTime<Utc>", archived as "archived!: bool", pinned as "pinned!: bool", name"#,
            id,
            task_id,
            Option::<String>::None,
            data.branch,
            data.agent_working_dir,
            Option::<DateTime<Utc>>::None
        )
        .fetch_one(pool)
        .await?)
    }

    pub async fn update_branch_name(
        pool: &SqlitePool,
        workspace_id: Uuid,
        new_branch_name: &str,
    ) -> Result<(), WorkspaceError> {
        sqlx::query!(
            "UPDATE workspaces SET branch = $1, updated_at = datetime('now') WHERE id = $2",
            new_branch_name,
            workspace_id,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    pub async fn resolve_container_ref(
        pool: &SqlitePool,
        container_ref: &str,
    ) -> Result<ContainerInfo, sqlx::Error> {
        let result = sqlx::query!(
            r#"SELECT w.id as "workspace_id!: Uuid",
                      w.task_id as "task_id!: Uuid",
                      t.project_id as "project_id!: Uuid"
               FROM workspaces w
               JOIN tasks t ON w.task_id = t.id
               WHERE w.container_ref = ?"#,
            container_ref
        )
        .fetch_optional(pool)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;

        Ok(ContainerInfo {
            workspace_id: result.workspace_id,
            task_id: result.task_id,
            project_id: result.project_id,
        })
    }

    /// Find workspace by path, also trying the parent directory.
    /// Used by VSCode extension which may open a repo subfolder (single-repo case)
    /// rather than the workspace root directory (multi-repo case).
    pub async fn resolve_container_ref_by_prefix(
        pool: &SqlitePool,
        path: &str,
    ) -> Result<ContainerInfo, sqlx::Error> {
        // First try exact match
        if let Ok(info) = Self::resolve_container_ref(pool, path).await {
            return Ok(info);
        }

        if let Some(parent) = std::path::Path::new(path).parent()
            && let Some(parent_str) = parent.to_str()
            && let Ok(info) = Self::resolve_container_ref(pool, parent_str).await
        {
            return Ok(info);
        }

        Err(sqlx::Error::RowNotFound)
    }

    pub async fn set_archived(
        pool: &SqlitePool,
        workspace_id: Uuid,
        archived: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "UPDATE workspaces SET archived = $1, updated_at = datetime('now', 'subsec') WHERE id = $2",
            archived,
            workspace_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Update workspace fields. Only non-None values will be updated.
    /// For `name`, pass `Some("")` to clear the name, `Some("foo")` to set it, or `None` to leave unchanged.
    pub async fn update(
        pool: &SqlitePool,
        workspace_id: Uuid,
        archived: Option<bool>,
        pinned: Option<bool>,
        name: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        // Convert empty string to None for name field (to store as NULL)
        let name_value = name.filter(|s| !s.is_empty());
        let name_provided = name.is_some();

        sqlx::query!(
            r#"UPDATE workspaces SET
                archived = COALESCE($1, archived),
                pinned = COALESCE($2, pinned),
                name = CASE WHEN $3 THEN $4 ELSE name END,
                updated_at = datetime('now', 'subsec')
            WHERE id = $5"#,
            archived,
            pinned,
            name_provided,
            name_value,
            workspace_id
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn get_first_user_message(
        pool: &SqlitePool,
        workspace_id: Uuid,
    ) -> Result<Option<String>, sqlx::Error> {
        let result = sqlx::query!(
            r#"SELECT cat.prompt
               FROM sessions s
               JOIN execution_processes ep ON ep.session_id = s.id
               JOIN coding_agent_turns cat ON cat.execution_process_id = ep.id
               WHERE s.workspace_id = $1
                 AND s.executor IS NOT NULL
                 AND cat.prompt IS NOT NULL
               ORDER BY s.created_at ASC, ep.created_at ASC
               LIMIT 1"#,
            workspace_id
        )
        .fetch_optional(pool)
        .await?;
        Ok(result.and_then(|r| r.prompt))
    }

    pub fn truncate_to_name(prompt: &str, max_len: usize) -> String {
        let trimmed = prompt.trim();
        if trimmed.chars().count() <= max_len {
            trimmed.to_string()
        } else {
            let truncated: String = trimmed.chars().take(max_len).collect();
            if let Some(last_space) = truncated.rfind(' ') {
                format!("{}...", &truncated[..last_space])
            } else {
                format!("{}...", truncated)
            }
        }
    }

    /// Batch-fetch the first user message for each of the given workspace IDs.
    /// Returns a map of workspace_id -> first prompt string.
    pub async fn get_first_user_messages_batch(
        pool: &SqlitePool,
        workspace_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, String>, sqlx::Error> {
        if workspace_ids.is_empty() {
            return Ok(HashMap::new());
        }

        // Build a dynamic query with an IN clause over the provided IDs.
        // We cannot use sqlx::query! here because the list length is variable.
        let placeholders: String = workspace_ids
            .iter()
            .map(|_| "?")
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            r#"WITH ranked AS (
                SELECT
                    s.workspace_id,
                    cat.prompt,
                    ROW_NUMBER() OVER (
                        PARTITION BY s.workspace_id
                        ORDER BY s.created_at ASC, ep.created_at ASC
                    ) AS rn
                FROM sessions s
                JOIN execution_processes ep ON ep.session_id = s.id
                JOIN coding_agent_turns cat ON cat.execution_process_id = ep.id
                WHERE s.workspace_id IN ({placeholders})
                  AND s.executor IS NOT NULL
                  AND cat.prompt IS NOT NULL
            )
            SELECT workspace_id, prompt
            FROM ranked
            WHERE rn = 1"#,
            placeholders = placeholders
        );

        let mut query = sqlx::query_as::<_, (Uuid, String)>(&sql);
        for id in workspace_ids {
            query = query.bind(*id);
        }

        let rows: Vec<(Uuid, String)> = query.fetch_all(pool).await?;

        let mut map = HashMap::with_capacity(rows.len());
        for (workspace_id, prompt) in rows {
            map.insert(workspace_id, prompt);
        }
        Ok(map)
    }

    /// Batch-update workspace names in a single round-trip using a CASE expression.
    /// `names` is a slice of (workspace_id, name) pairs. Empty slices are no-ops.
    pub async fn update_names_batch(
        pool: &SqlitePool,
        names: &[(Uuid, String)],
    ) -> Result<(), sqlx::Error> {
        // Process in chunks of 100 to guard against very large SQL strings.
        for chunk in names.chunks(100) {
            if chunk.is_empty() {
                continue;
            }

            // Build:
            //   UPDATE workspaces
            //   SET name = CASE id WHEN ? THEN ? WHEN ? THEN ? ... END,
            //       updated_at = datetime('now', 'subsec')
            //   WHERE id IN (?, ?, ...)
            //
            // IDs are bound twice: once in the CASE WHEN branches, once in the IN list.
            // Using plain `?` (positional) to stay compatible with sqlx's sequential binding.
            let case_branches: String = chunk
                .iter()
                .map(|_| " WHEN ? THEN ?")
                .collect::<Vec<_>>()
                .join("");

            let in_placeholders: String = chunk
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(", ");

            let sql = format!(
                "UPDATE workspaces SET name = CASE id{case} END, \
                 updated_at = datetime('now', 'subsec') \
                 WHERE id IN ({ids})",
                case = case_branches,
                ids = in_placeholders
            );

            let mut query = sqlx::query(&sql);
            // Bind for the CASE WHEN branches
            for (id, name) in chunk {
                query = query.bind(*id).bind(name);
            }
            // Bind again for the IN clause
            for (id, _name) in chunk {
                query = query.bind(*id);
            }
            query.execute(pool).await?;
        }
        Ok(())
    }

    pub async fn find_all_with_status(
        pool: &SqlitePool,
        archived: Option<bool>,
        limit: Option<i64>,
    ) -> Result<Vec<WorkspaceWithStatus>, sqlx::Error> {
        // Intermediate row type so we can use sqlx::query_as with a dynamic SQL string.
        // This lets us push the optional WHERE and LIMIT clauses into the database
        // instead of fetching all rows and filtering in Rust.
        #[derive(sqlx::FromRow)]
        struct WorkspaceWithStatusRow {
            id: Uuid,
            task_id: Uuid,
            container_ref: Option<String>,
            branch: String,
            agent_working_dir: Option<String>,
            setup_completed_at: Option<DateTime<Utc>>,
            created_at: DateTime<Utc>,
            updated_at: DateTime<Utc>,
            archived: bool,
            pinned: bool,
            name: Option<String>,
            is_running: i64,
            is_errored: i64,
        }

        // Build SQL with optional WHERE and LIMIT pushed into the database.
        let where_clause = match archived {
            Some(_) => "WHERE w.archived = ?",
            None => "",
        };
        let limit_clause = match limit {
            Some(_) => "LIMIT ?",
            None => "",
        };

        let sql = format!(
            r#"WITH ep_ranked AS (
                SELECT
                    s.workspace_id,
                    ep.status,
                    ROW_NUMBER() OVER (
                        PARTITION BY s.workspace_id
                        ORDER BY ep.created_at DESC
                    ) AS rn
                FROM execution_processes ep
                JOIN sessions s ON ep.session_id = s.id
                WHERE ep.run_reason IN ('setupscript', 'cleanupscript', 'codingagent')
                  AND ep.dropped = FALSE
            ),
            workspace_status AS (
                SELECT
                    workspace_id,
                    MAX(CASE WHEN status = 'running' THEN 1 ELSE 0 END)      AS is_running,
                    MAX(CASE WHEN rn = 1 AND status IN ('failed','killed')
                             THEN 1 ELSE 0 END)                               AS is_errored
                FROM ep_ranked
                GROUP BY workspace_id
            )
            SELECT
                w.id,
                w.task_id,
                w.container_ref,
                w.branch,
                w.agent_working_dir,
                w.setup_completed_at,
                w.created_at,
                w.updated_at,
                w.archived,
                w.pinned,
                w.name,
                COALESCE(ws.is_running, 0) AS is_running,
                COALESCE(ws.is_errored, 0) AS is_errored
            FROM workspaces w
            LEFT JOIN workspace_status ws ON ws.workspace_id = w.id
            {where_clause}
            ORDER BY w.updated_at DESC
            {limit_clause}"#,
            where_clause = where_clause,
            limit_clause = limit_clause,
        );

        let mut query = sqlx::query_as::<_, WorkspaceWithStatusRow>(&sql);
        if let Some(a) = archived {
            query = query.bind(a);
        }
        if let Some(lim) = limit {
            query = query.bind(lim);
        }

        let mut workspaces: Vec<WorkspaceWithStatus> = query
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|rec| {
                WorkspaceWithStatus {
                    workspace: Workspace {
                        id: rec.id,
                        task_id: rec.task_id,
                        container_ref: rec.container_ref,
                        branch: rec.branch,
                        agent_working_dir: rec.agent_working_dir,
                        setup_completed_at: rec.setup_completed_at,
                        created_at: rec.created_at,
                        updated_at: rec.updated_at,
                        archived: rec.archived,
                        pinned: rec.pinned,
                        name: rec.name,
                    },
                    is_running: rec.is_running != 0,
                    is_errored: rec.is_errored != 0,
                }
            })
            .collect();

        // Collect IDs of workspaces that need a name derived from their first user message.
        let unnamed_ids: Vec<Uuid> = workspaces
            .iter()
            .filter(|ws| ws.workspace.name.is_none())
            .map(|ws| ws.workspace.id)
            .collect();

        if !unnamed_ids.is_empty() {
            // Batch-fetch first user messages for all unnamed workspaces (2 queries total).
            let prompts = Self::get_first_user_messages_batch(pool, &unnamed_ids).await?;

            let mut name_updates: Vec<(Uuid, String)> = Vec::with_capacity(prompts.len());
            for ws in &mut workspaces {
                if ws.workspace.name.is_none() {
                    if let Some(prompt) = prompts.get(&ws.workspace.id) {
                        let name = Self::truncate_to_name(prompt, WORKSPACE_NAME_MAX_LEN);
                        ws.workspace.name = Some(name.clone());
                        name_updates.push((ws.workspace.id, name));
                    }
                }
            }

            // Persist all derived names in a single batch UPDATE.
            Self::update_names_batch(pool, &name_updates).await?;
        }

        Ok(workspaces)
    }

    /// Delete a workspace by ID
    pub async fn delete(pool: &SqlitePool, id: Uuid) -> Result<u64, sqlx::Error> {
        let result = sqlx::query!("DELETE FROM workspaces WHERE id = $1", id)
            .execute(pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Count total workspaces across all projects
    pub async fn count_all(pool: &SqlitePool) -> Result<i64, WorkspaceError> {
        sqlx::query_scalar!(r#"SELECT COUNT(*) as "count!: i64" FROM workspaces"#)
            .fetch_one(pool)
            .await
            .map_err(WorkspaceError::Database)
    }

    pub async fn find_by_id_with_status(
        pool: &SqlitePool,
        id: Uuid,
    ) -> Result<Option<WorkspaceWithStatus>, sqlx::Error> {
        let rec = sqlx::query!(
            r#"SELECT
                w.id AS "id!: Uuid",
                w.task_id AS "task_id!: Uuid",
                w.container_ref,
                w.branch,
                w.agent_working_dir,
                w.setup_completed_at AS "setup_completed_at: DateTime<Utc>",
                w.created_at AS "created_at!: DateTime<Utc>",
                w.updated_at AS "updated_at!: DateTime<Utc>",
                w.archived AS "archived!: bool",
                w.pinned AS "pinned!: bool",
                w.name,

                CASE WHEN EXISTS (
                    SELECT 1
                    FROM sessions s
                    JOIN execution_processes ep ON ep.session_id = s.id
                    WHERE s.workspace_id = w.id
                      AND ep.status = 'running'
                      AND ep.run_reason IN ('setupscript','cleanupscript','codingagent')
                    LIMIT 1
                ) THEN 1 ELSE 0 END AS "is_running!: i64",

                CASE WHEN (
                    SELECT ep.status
                    FROM sessions s
                    JOIN execution_processes ep ON ep.session_id = s.id
                    WHERE s.workspace_id = w.id
                      AND ep.run_reason IN ('setupscript','cleanupscript','codingagent')
                    ORDER BY ep.created_at DESC
                    LIMIT 1
                ) IN ('failed','killed') THEN 1 ELSE 0 END AS "is_errored!: i64"

            FROM workspaces w
            WHERE w.id = $1"#,
            id
        )
        .fetch_optional(pool)
        .await?;

        let Some(rec) = rec else {
            return Ok(None);
        };

        let mut ws = WorkspaceWithStatus {
            workspace: Workspace {
                id: rec.id,
                task_id: rec.task_id,
                container_ref: rec.container_ref,
                branch: rec.branch,
                agent_working_dir: rec.agent_working_dir,
                setup_completed_at: rec.setup_completed_at,
                created_at: rec.created_at,
                updated_at: rec.updated_at,
                archived: rec.archived,
                pinned: rec.pinned,
                name: rec.name,
            },
            is_running: rec.is_running != 0,
            is_errored: rec.is_errored != 0,
        };

        if ws.workspace.name.is_none()
            && let Some(prompt) = Self::get_first_user_message(pool, ws.workspace.id).await?
        {
            let name = Self::truncate_to_name(&prompt, WORKSPACE_NAME_MAX_LEN);
            Self::update(pool, ws.workspace.id, None, None, Some(&name)).await?;
            ws.workspace.name = Some(name);
        }

        Ok(Some(ws))
    }
}
