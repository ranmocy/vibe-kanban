# Architecture

Vibe Kanban is a task orchestration platform for AI coding agents. Users manage tasks on a kanban board; each task spawns an isolated workspace where an agent (Claude Code, Gemini, or Codex) writes code in a git worktree. The system captures logs in real time and streams them to the browser.

## Deployment Modes

| Mode      | Database   | Backend                                                 | Real-Time Sync     | Auth               |
| --------- | ---------- | ------------------------------------------------------- | ------------------ | ------------------ |
| **Local** | SQLite     | `server` + `local-deployment` crate, launched via `npx` | WebSocket + SSE    | None (single user) |
| **Cloud** | PostgreSQL | `remote` crate                                          | ElectricSQL shapes | OAuth / JWT        |

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Frontend (React)                         │
│  React Query ─ Zustand ─ Context ─ ElectricSQL (cloud only)     │
└────────┬──────────────┬──────────────┬──────────────────────────┘
         │ REST         │ WebSocket    │ SSE
         │ (CRUD)       │ (logs)       │ (events)
┌────────▼──────────────▼──────────────▼──────────────────────────┐
│                     Backend (Axum / Tokio)                       │
│  Routes → Services → Executors                                  │
│                         │ spawn                                  │
│                         ▼                                        │
│              ┌─────────────────────┐                             │
│              │  Agent Processes    │                              │
│              │  Claude │ Gemini    │                              │
│              │  Codex  │ ACP      │                              │
│              └─────────────────────┘                             │
└────────┬────────────────────────────────────────────────────────┘
         │ SQLx
┌────────▼────────────────────────────────────────────────────────┐
│              Database (SQLite or PostgreSQL)                     │
└─────────────────────────────────────────────────────────────────┘
```

## Crate Structure

All Rust code lives under `crates/`:

| Crate              | Purpose                                                                      |
| ------------------ | ---------------------------------------------------------------------------- |
| `server`           | Axum HTTP server, API routes, middleware, `main.rs` entry point              |
| `db`               | SQLx models, migrations, database connection pool                            |
| `services`         | Business logic: container orchestration, events, git, config, file search    |
| `executors`        | Agent execution engine: Claude (MCP), Gemini (API), Codex (JSON-RPC), ACP    |
| `deployment`       | `Deployment` trait — abstract interface over all services                    |
| `local-deployment` | Local desktop implementation: SQLite, git worktrees, PTY terminals           |
| `remote`           | Cloud server: PostgreSQL, ElectricSQL, OAuth, multi-tenant                   |
| `api-types`        | Shared request/response types between local and remote servers               |
| `utils`            | Common utilities: asset embedding, port management, Sentry, response helpers |
| `git`              | Git operations wrapper (libgit2)                                             |
| `review`           | Code review utilities                                                        |

### Dependency Graph

```
server ──→ services ──→ executors
  │            │            │
  │            ▼            │
  ├──→ db ◀───────────────┘
  │
  ├──→ deployment (trait)
  │       ▲
  │       ├── local-deployment
  │       └── remote
  │
  └──→ utils, git, api-types
```

## Data Model

### Entity Relationships

```
Project
  └── Task (many)
        └── Workspace (many — one per attempt)
              ├── WorkspaceRepo (many — git worktrees)
              └── Session (many — one per agent run)
                    ├── ExecutionProcess (many — setup, agent, cleanup)
                    │     └── ExecutionProcessLogs (stdout/stderr capture)
                    │     └── ExecutionProcessRepoState (git snapshot)
                    └── CodingAgentTurn (agent message history)

Repo ──── WorkspaceRepo (junction)
Tag  ──── TaskTag (junction) ──── Task
Image ─── TaskImage (junction) ── Task
```

### Key Entities

| Entity               | Table                 | Role                                                     |
| -------------------- | --------------------- | -------------------------------------------------------- |
| **Project**          | `projects`            | Top-level grouping with repo associations and config     |
| **Task**             | `tasks`               | A unit of work with title, description, status, priority |
| **Workspace**        | `workspaces`          | Isolated execution environment for one task attempt      |
| **Session**          | `sessions`            | A coding agent session within a workspace                |
| **ExecutionProcess** | `execution_processes` | A single process run (setup script, agent, or cleanup)   |
| **Repo**             | `repos`               | Git repository metadata (path, name, remote URL)         |

### ExecutionProcess Run Reasons

Each `ExecutionProcess` has a `run_reason` indicating its role:

- `SetupScript` — project setup commands before agent starts
- `CodingAgent` — the AI agent process itself
- `CleanupScript` — post-execution cleanup
- `DevServer` — long-running dev server

## API Layer

### Route Structure

All routes are mounted under `/api` (defined in `crates/server/src/routes/mod.rs`):

| Path                     | Methods    | Purpose                                                     |
| ------------------------ | ---------- | ----------------------------------------------------------- |
| `/health`                | GET        | Health check                                                |
| `/config/*`              | GET, PATCH | User config, system info                                    |
| `/projects/*`            | CRUD       | Project management                                          |
| `/tasks/*`               | CRUD + WS  | Task CRUD, WebSocket task streaming                         |
| `/task-attempts/*`       | CRUD       | Workspace/attempt management, git ops (merge, push, rebase) |
| `/execution-processes/*` | GET + WS   | Process status, WebSocket log streaming                     |
| `/sessions/*`            | POST       | Session creation, follow-up messages, review                |
| `/events/*`              | GET (SSE)  | Server-sent event stream                                    |
| `/approvals/*`           | POST       | Approval workflow                                           |
| `/repos/*`               | CRUD       | Repository management                                       |
| `/tags/*`                | CRUD       | Tag management                                              |
| `/filesystem/*`          | GET        | File system browsing                                        |
| `/search/*`              | GET        | Full-text search                                            |
| `/images/*`              | GET, POST  | Image upload and serving                                    |
| `/terminal/*`            | WS         | PTY terminal access                                         |
| `/oauth/*`               | GET        | OAuth flows (GitHub, Azure)                                 |

### Request Lifecycle

```
HTTP Request
  → Origin validation middleware
  → Route matching
  → Model loader middleware (extracts Task/ExecutionProcess from path params)
  → Handler function
    → Service layer call
    → Database query (SQLx)
  → ApiResponse<T> serialization
  → HTTP Response
```

## End-to-End Data Flow: Task Execution

This traces the most important flow — creating a task and running an agent:

```
1. User clicks "Create & Start"
   │
   ▼
2. POST /api/tasks/create-and-start
   Body: { task, executor_profile_id, repos, message }
   │
   ▼
3. Handler: create Task record in DB
   INSERT INTO tasks (project_id, title, description, status, ...)
   │
   ▼
4. Create Workspace record
   INSERT INTO workspaces (task_id, ...)
   │
   ▼
5. ContainerService::create(&workspace)
   ├── Create git worktree for each repo
   ├── INSERT INTO workspace_repos
   └── Return container reference (file path)
   │
   ▼
6. Create Session record
   INSERT INTO sessions (workspace_id, executor_profile_id, ...)
   │
   ▼
7. Run setup script (if configured)
   ├── Create ExecutionProcess (run_reason: SetupScript)
   ├── Spawn subprocess in worktree directory
   ├── Capture stdout/stderr → ExecutionProcessLogs
   ├── Stream logs to WebSocket subscribers
   └── Wait for completion
   │
   ▼
8. Run coding agent
   ├── Create ExecutionProcess (run_reason: CodingAgent)
   ├── Spawn agent (Claude via MCP / Gemini via API / Codex via JSON-RPC)
   ├── Pass task prompt + context to agent
   ├── Agent reads/writes files in worktree
   ├── Stdout/stderr captured as LogMsg stream
   │     ├── LogMsg::Stdout(content)
   │     ├── LogMsg::Stderr(content)
   │     └── LogMsg::Finished
   ├── Convert to ConversationPatch (JSON Patch format)
   ├── Store in ExecutionProcessLogs
   ├── Stream to WebSocket subscribers
   └── Emit events via EventService
   │
   ▼
9. Agent completes
   ├── ExecutionProcess status → Completed/Failed
   ├── Snapshot git state → ExecutionProcessRepoState
   └── Run cleanup script (if configured)
   │
   ▼
10. User reviews changes
    ├── View diff via GET /api/task-attempts/:id/diff
    ├── Merge via POST /api/task-attempts/:id/merge
    └── Push via POST /api/task-attempts/:id/push
```

## Real-Time Communication

Three patterns deliver live data to the frontend:

### 1. WebSocket — Log Streaming

Used for streaming agent stdout/stderr to the browser in real time.

**Endpoints:**
- `/api/execution-processes/:id/logs/raw/ws` — raw stdout/stderr
- `/api/execution-processes/:id/logs/normalized/ws` — structured log entries

**Protocol:**
```
Server → Client: JSON messages
  {
    "JsonPatch": [{
      "value": { "type": "STDOUT", "content": "..." }
    }]
  }
  // or
  { "finished": true }
```

**Frontend consumption** (`useLogStream` hook):
- Connects WebSocket, accumulates log entries
- Auto-reconnects with exponential backoff (up to 6 retries)
- Clears state when process ID changes

### 2. Server-Sent Events (SSE)

Unidirectional stream for task and execution status updates.

**Endpoint:** `GET /api/events` → `text/event-stream`

The `EventService` publishes events when tasks or execution processes change state. The frontend subscribes and invalidates React Query caches accordingly.

### 3. JSON Patch (ConversationPatch)

Incremental updates to conversation state. Rather than sending full conversation objects, the server sends RFC 6902 JSON Patch operations:

```json
{ "op": "add", "path": "/stdout/42", "value": "Building project..." }
```

This minimizes bandwidth for long-running agent sessions with extensive output.

## Frontend Architecture

### Technology Stack

- **React 18** with TypeScript
- **Vite** for bundling
- **Tailwind CSS** for styling
- **React Router v6** for routing
- **React Query** for server state
- **Zustand** for UI state
- **ElectricSQL** for real-time sync (cloud mode)

### Provider Hierarchy

```
QueryClientProvider (React Query — stale time: 5min)
  └── PostHogProvider (analytics)
    └── Sentry.ErrorBoundary
      └── BrowserRouter
        └── UserSystemProvider (config, auth, environment)
          └── ThemeProvider (dark/light)
            └── HotkeysProvider
              └── Routes
                └── SharedAppLayout
                  └── OrgProvider (cloud)
                    └── ProjectProvider (ElectricSQL mutations)
                      └── Page components
```

### State Management

| Layer          | Tool          | Purpose                                                          |
| -------------- | ------------- | ---------------------------------------------------------------- |
| Server state   | React Query   | Tasks, projects, sessions, config — cached with 5-min stale time |
| UI preferences | Zustand       | Pane sizes, collapse states, kanban filters, diff view mode      |
| Cross-cutting  | React Context | User system info, theme, search, terminal, organization          |
| Real-time sync | ElectricSQL   | Cloud mode: issues, statuses, tags — optimistic mutations        |

### Component Organization

```
frontend/src/components/
├── ui-new/
│   ├── containers/    # Smart components (data fetching, mutations)
│   ├── views/         # Presentational components (props only)
│   ├── primitives/    # Reusable UI atoms (Button, Input, Dialog, ...)
│   └── dialogs/       # Modal dialogs by domain (org/, projects/, tasks/)
├── legacy-design/     # Old UI components
├── layout/            # App shell, navigation
└── panels/            # Sidebars, detail panels
```

### Frontend-Backend Data Flow

**Local mode:**
```
Component → React Query hook → api.ts fetch() → /api/* → Backend
                                                            │
Component ← React Query cache ← JSON response ◄────────────┘
```

**Cloud mode:**
```
Component → useShape() hook → ElectricSQL shape subscription
                                       │
                                       ▼
                              Electric sync layer
                                       │
                    ┌──────────────────┤
                    ▼                   ▼
          Optimistic update     Remote API (POST /v1/*)
          (instant UI)          (persisted Promise)
```

## Agent Execution Engine

### Supported Agents

| Agent       | Protocol                     | Transport | Key File                                   |
| ----------- | ---------------------------- | --------- | ------------------------------------------ |
| Claude Code | MCP (Model Context Protocol) | stdio     | `crates/executors/src/executors/claude.rs` |
| Gemini      | REST API                     | HTTP      | `crates/executors/src/executors/gemini.rs` |
| Codex       | JSON-RPC                     | stdio     | `crates/executors/src/executors/codex/`    |
| ACP         | Agent Communication Protocol | HTTP      | `crates/executors/src/executors/acp/`      |

### Executor Profile System

Agents are configured via profiles stored in user config. Each profile specifies:
- Agent type (Claude, Gemini, Codex)
- Model parameters
- System prompt overrides
- MCP server configurations

Profiles are selected when creating a session and determine which executor handles the task.

### Log Processing Pipeline

```
Agent Process (stdout/stderr)
  → LogMsg stream (Stdout | Stderr | Finished | JsonPatch)
  → NormalizedEntry (structured: role, content, tool calls)
  → ConversationPatch (JSON Patch for incremental delivery)
  → WebSocket broadcast to connected clients
  → ExecutionProcessLogs table (persistent storage)
```

## Type Safety Bridge

Rust types are the source of truth. TypeScript types are generated, never hand-written.

```
Rust struct with #[derive(TS)]
  → ts-rs generates shared/types.ts
  → Frontend imports from 'shared/types'
  → TypeScript compiler validates API usage
```

**Generation:** `pnpm run generate-types` (runs `crates/server/src/bin/generate_types.rs`)

**SQLx compile-time checking:** All SQL queries are verified against the database schema at compile time. Offline mode (`sqlx-data.json`) enables CI builds without a live database.

## Configuration

User configuration is stored in `~/.vibe-kanban/config.toml` with a versioned schema (v1 through v8). The `ConfigService` handles automatic migration between versions on startup.

Key configuration areas:
- Agent profiles and defaults
- Project-repo associations
- Setup/cleanup scripts per project
- UI preferences
- GitHub/Azure integration tokens

Environment variables (`FRONTEND_PORT`, `BACKEND_PORT`, `HOST`) control runtime binding. Dev ports are auto-assigned by `scripts/setup-dev-environment.js`.
