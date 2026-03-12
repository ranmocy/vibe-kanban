# Workspace Switching: API Calls and Data Flow

This document traces exactly what happens when a user switches from one workspace to another — every API call, WebSocket connection, and state transition.

## Trigger

The user clicks a workspace in the sidebar. This calls `selectWorkspace(id)` in `WorkspaceContext`, which:

1. Marks the workspace as seen: `PUT /api/task-attempts/{id}` (fire-and-forget)
2. Invalidates workspace summary cache so the sidebar refreshes unread indicators
3. Navigates to `/workspaces/{id}` via React Router

## Timeline of Events

```
User clicks workspace B (currently viewing workspace A)
  │
  ▼
1. URL changes: /workspaces/A → /workspaces/B
  │
  ▼
2. useParams() returns new workspaceId
  │
  ▼
3. WorkspaceContext re-renders, all hooks re-fire with new ID
  │
  ├─── REST queries (parallel) ──────────────────────┐
  │    GET /api/task-attempts/{B}                     │ workspace metadata
  │    GET /api/sessions?workspace_id={B}             │ sessions list
  │    GET /api/task-attempts/{B}/repos               │ repo associations
  │    GET /api/task-attempts/{B}/branch-status       │ git branch info
  │    GET /api/task-attempts/{B}/first-message       │ original prompt
  │    GET /api/execution-processes/history?session_id={S} │ process history
  │    GET /api/scratch/DraftFollowUp/{B}             │ saved draft
  │    GET /api/task-attempts/{B}/pr/comments?repo_id={R}  │ PR comments (if PR)
  │                                                    │
  ▼                                                    │
4. ExecutionProcessesProvider remounts (key changes)   │
  │                                                    │
  ├─── Old WebSocket connections close ────────────┐  │
  │    WS /execution-processes/stream/session/ws   │  │
  │    WS /execution-processes/{old}/raw-logs/ws   │  │
  │    WS /task-attempts/{A}/diff/ws               │  │
  │                                                 │  │
  ├─── New WebSocket connections open ─────────────┤  │
  │    WS /execution-processes/stream/session/ws?session_id={S}  │
  │    WS /execution-processes/{new}/raw-logs/ws                 │
  │    WS /task-attempts/{B}/diff/ws                             │
  │                                                    │
  ▼                                                    ▼
5. UI rebuilds with new data
   ├── File tree renders from new diffs
   ├── Conversation panel shows new session history
   ├── Git panel shows new branch status
   └── Terminal tabs switch (preserved per workspace)
```

## API Calls in Detail

### Phase 1: Core Workspace Data (REST)

These fire immediately when `workspaceId` changes, in parallel via React Query:

| # | Endpoint | Hook | Returns | Cache |
|---|----------|------|---------|-------|
| 1 | `GET /api/task-attempts/{id}` | `useAttempt()` | Workspace metadata (id, task_id, branch, container_ref, archived, pinned) | Standard |
| 2 | `GET /api/sessions?workspace_id={id}` | `useWorkspaceSessions()` | All sessions for this workspace; auto-selects the first one | Standard |
| 3 | `GET /api/task-attempts/{id}/repos` | `useAttemptRepo()` | Repos with target branches, setup/cleanup scripts | Standard |
| 4 | `GET /api/task-attempts/{id}/branch-status` | `useBranchStatus()` | Git branch name, ahead/behind counts, commit info | Polls every 5s |
| 5 | `GET /api/task-attempts/{id}/first-message` | via WorkspaceContext | The original task prompt | Standard |

### Phase 2: Session-Scoped Data (REST)

Once `useWorkspaceSessions` returns and a session is selected:

| # | Endpoint | Hook | Returns | Cache |
|---|----------|------|---------|-------|
| 6 | `GET /api/execution-processes/history?session_id={id}&limit=50` | `useExecutionProcessHistory()` | Paginated execution process records | Standard |
| 7 | `GET /api/scratch/DraftFollowUp/{workspaceId}` | via scratch hooks | Saved draft follow-up message | Standard |

### Phase 3: Conditional Data (REST)

Only fetched when certain conditions are met:

| # | Endpoint | Condition | Returns |
|---|----------|-----------|---------|
| 8 | `GET /api/task-attempts/{id}/pr/comments?repo_id={R}` | Workspace has a PR attached | PR review comments per repo (stale time: 30s, retries: 2) |

### Phase 4: Real-Time Streams (WebSocket)

These open after REST data arrives and provide live updates:

| # | Endpoint | Hook | Streams |
|---|----------|------|---------|
| 9 | `WS /api/task-attempts/{id}/diff/ws` | `useDiffStream()` | JSON Patch updates to file diffs as agent writes code |
| 10 | `WS /api/execution-processes/stream/session/ws?session_id={id}` | `useExecutionProcesses()` | Real-time execution process status changes |
| 11 | `WS /api/execution-processes/{id}/raw-logs/ws` | `useLogStream()` | Stdout/stderr from the currently running process |

### Background Streams (Always Open)

These WebSockets stay open across workspace switches:

| Endpoint | Hook | Purpose |
|----------|------|---------|
| `WS /api/task-attempts/stream/ws?archived=false` | `useWorkspaces()` | Sidebar: active workspace list updates |
| `WS /api/task-attempts/stream/ws?archived=true` | `useWorkspaces()` | Sidebar: archived workspace list updates |
| `SSE /api/events/` | Event listener | Global task/execution event stream |

### Summary Polling (Periodic)

| Endpoint | Interval | Purpose |
|----------|----------|---------|
| `POST /api/task-attempts/summary` | Every 15s | Sidebar badges: file counts, running status, unseen activity |
| `GET /api/task-attempts/{id}/branch-status` | Every 5s | Git panel: branch ahead/behind status |

## WebSocket Connection Lifecycle

The key mechanism for clean workspace switching is the `ExecutionProcessesProvider` component, which uses a **React key** tied to `${workspaceId}-${selectedSessionId}`:

```
<ExecutionProcessesProvider key={`${workspace.id}-${sessionId}`}>
```

When the key changes, React:
1. **Unmounts** the old provider instance → all WebSocket connections in that subtree close
2. **Mounts** a new provider instance → fresh WebSocket connections open with new IDs

This prevents stale log data from workspace A leaking into workspace B.

### Log Stream Safety

The `useLogStream` hook adds a second layer of protection:
- Tracks `currentProcessIdRef` to detect stale messages
- Clears accumulated logs when the process ID changes
- Implements exponential backoff reconnection (max 6 retries, capped at 1.5s)

## Backend Processing per Request

### `GET /api/task-attempts/{id}` (Workspace Load)

```
Request
  → load_workspace_middleware
    → Workspace::get_by_id(&pool, id)   [SELECT * FROM workspaces WHERE id = ?]
  → Handler returns workspace from middleware context
```

### `GET /api/sessions?workspace_id={id}` (Session List)

```
Request
  → Handler
    → Session::get_by_workspace_id(&pool, id)
      [SELECT * FROM sessions WHERE workspace_id = ? ORDER BY created_at DESC]
  → Returns Vec<Session>
```

### `GET /api/task-attempts/{id}/repos` (Repo List)

```
Request
  → load_workspace_middleware
  → Handler
    → WorkspaceRepo::get_by_workspace_id(&pool, id)
      [SELECT r.*, wr.target_branch FROM repos r
       JOIN workspace_repos wr ON r.id = wr.repo_id
       WHERE wr.workspace_id = ?
       ORDER BY r.display_name]
  → Returns Vec<RepoWithTargetBranch>
```

### `WS /api/execution-processes/{id}/raw-logs/ws` (Log Stream)

```
WebSocket Upgrade
  → Verify stream exists: container.stream_raw_logs(exec_id)
  → Split socket into sender/receiver
  → Spawn task to drain inbound pings
  → Forward LogMsg stream to client:
      LogMsg::Stdout(content) → ConversationPatch::add_stdout → JSON
      LogMsg::Stderr(content) → ConversationPatch::add_stderr → JSON
      LogMsg::Finished → close
```

### `WS /api/task-attempts/{id}/diff/ws` (Diff Stream)

```
WebSocket Upgrade
  → container.stream_diff_stats(workspace)
  → Streams JSON Patch with file-level diff statistics
  → Updates as agent modifies files in the worktree
```

## State Management During Switch

| State Layer | Behavior on Switch |
|-------------|-------------------|
| **React Query cache** | Old workspace data stays cached (5-min stale time); new workspace data fetched or served from cache if previously visited |
| **Zustand UI store** | Pane sizes, filter settings persist (not workspace-scoped) |
| **Terminal tabs** | Preserved per workspace in `tabsByWorkspace[workspaceId]`; tabs for workspace A stay alive |
| **Log accumulator** | Cleared on process ID change; fresh start for new workspace's processes |
| **Diff state** | Replaced entirely; new diff stream starts from scratch |
| **Conversation panel** | Re-renders with new session's execution process history |

## Sequence Diagram

```
 Browser                    Axum Server                    Database
    │                           │                              │
    │──── navigate(/workspaces/B) ────────────────────────────│
    │                           │                              │
    │── GET /task-attempts/B ──→│── SELECT workspace ─────────→│
    │←── Workspace JSON ───────│←── row ──────────────────────│
    │                           │                              │
    │── GET /sessions?wid=B ──→│── SELECT sessions ───────────→│
    │←── [Session] ────────────│←── rows ─────────────────────│
    │                           │                              │
    │── GET /task-attempts/B/repos →│── SELECT repos+workspace_repos →│
    │←── [RepoWithTargetBranch] ──│←── joined rows ──────────────│
    │                           │                              │
    │── GET /branch-status ───→│── git status (worktree) ─────│
    │←── RepoBranchStatus ─────│                              │
    │                           │                              │
    │── WS /diff/ws ──────────→│── subscribe to file watcher ─│
    │←── JSON Patch stream ────│                              │
    │                           │                              │
    │── WS /stream/session/ws →│── subscribe to exec events ──│
    │←── ExecutionProcess stream│                              │
    │                           │                              │
    │── WS /raw-logs/ws ──────→│── subscribe to process logs ─│
    │←── LogMsg stream ────────│                              │
    │                           │                              │
    │  [UI fully loaded with workspace B data]                 │
```
