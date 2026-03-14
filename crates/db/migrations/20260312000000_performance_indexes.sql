-- Performance indexes for high-frequency query paths.
-- These cover the filtering predicates used by workspace-status CTEs,
-- the archived filter, the task_id join, and the coding_agent_turns lookup.

-- 1. Covering index for the run_reason + dropped filter used by ALL workspace-status
--    queries. The status and created_at columns are appended so they can be read
--    from the index leaf page without a heap lookup.
CREATE INDEX IF NOT EXISTS idx_ep_session_reason_dropped_status
ON execution_processes (session_id, run_reason, dropped, status, created_at DESC);

-- 2. Cover the archived filter in find_latest_for_workspaces and
--    find_workspaces_with_running_dev_servers.
CREATE INDEX IF NOT EXISTS idx_workspaces_archived_updated
ON workspaces (archived, updated_at DESC);

-- 3. Cover the task_id join used by the task_status CTE.
CREATE INDEX IF NOT EXISTS idx_workspaces_task_id
ON workspaces (task_id);

-- 4. Cover the execution_process_id lookup in coding_agent_turns used
--    by the batch naming query.
CREATE INDEX IF NOT EXISTS idx_cat_ep_id_prompt
ON coding_agent_turns (execution_process_id, prompt)
WHERE prompt IS NOT NULL;

PRAGMA optimize;
