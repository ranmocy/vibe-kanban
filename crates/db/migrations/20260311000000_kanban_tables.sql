-- Kanban organizations
CREATE TABLE IF NOT EXISTS kanban_organizations (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    is_personal INTEGER NOT NULL DEFAULT 0,
    issue_prefix TEXT NOT NULL DEFAULT 'ISS',
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Kanban users (local user, not requiring OAuth)
CREATE TABLE IF NOT EXISTS kanban_users (
    id TEXT PRIMARY KEY NOT NULL,
    email TEXT NOT NULL,
    first_name TEXT,
    last_name TEXT,
    username TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Organization membership
CREATE TABLE IF NOT EXISTS kanban_organization_members (
    organization_id TEXT NOT NULL REFERENCES kanban_organizations(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES kanban_users(id) ON DELETE CASCADE,
    role TEXT NOT NULL DEFAULT 'ADMIN',
    joined_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_seen_at TEXT,
    PRIMARY KEY (organization_id, user_id)
);

-- Projects within organizations
CREATE TABLE IF NOT EXISTS kanban_projects (
    id TEXT PRIMARY KEY NOT NULL,
    organization_id TEXT NOT NULL REFERENCES kanban_organizations(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    color TEXT NOT NULL DEFAULT '#6366f1',
    issue_counter INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Project statuses (kanban columns)
CREATE TABLE IF NOT EXISTS kanban_project_statuses (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES kanban_projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    color TEXT NOT NULL DEFAULT '#6b7280',
    sort_order INTEGER NOT NULL DEFAULT 0,
    hidden INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Issues (kanban cards)
CREATE TABLE IF NOT EXISTS kanban_issues (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES kanban_projects(id) ON DELETE CASCADE,
    status_id TEXT NOT NULL REFERENCES kanban_project_statuses(id),
    issue_number INTEGER NOT NULL,
    simple_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    priority TEXT CHECK(priority IN ('urgent', 'high', 'medium', 'low')),
    start_date TEXT,
    target_date TEXT,
    completed_at TEXT,
    sort_order REAL NOT NULL DEFAULT 0,
    parent_issue_id TEXT REFERENCES kanban_issues(id) ON DELETE SET NULL,
    parent_issue_sort_order REAL,
    extension_metadata TEXT DEFAULT '{}',
    creator_user_id TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(project_id, issue_number)
);

-- Tags for issues
CREATE TABLE IF NOT EXISTS kanban_tags (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES kanban_projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    color TEXT NOT NULL DEFAULT '#6b7280',
    UNIQUE(project_id, name)
);

-- Issue-to-assignee junction
CREATE TABLE IF NOT EXISTS kanban_issue_assignees (
    id TEXT PRIMARY KEY NOT NULL,
    issue_id TEXT NOT NULL REFERENCES kanban_issues(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    assigned_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(issue_id, user_id)
);

-- Issue-to-follower junction
CREATE TABLE IF NOT EXISTS kanban_issue_followers (
    id TEXT PRIMARY KEY NOT NULL,
    issue_id TEXT NOT NULL REFERENCES kanban_issues(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    UNIQUE(issue_id, user_id)
);

-- Issue-to-tag junction
CREATE TABLE IF NOT EXISTS kanban_issue_tags (
    id TEXT PRIMARY KEY NOT NULL,
    issue_id TEXT NOT NULL REFERENCES kanban_issues(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES kanban_tags(id) ON DELETE CASCADE,
    UNIQUE(issue_id, tag_id)
);

-- Issue relationships (blocking, related, duplicate)
CREATE TABLE IF NOT EXISTS kanban_issue_relationships (
    id TEXT PRIMARY KEY NOT NULL,
    issue_id TEXT NOT NULL REFERENCES kanban_issues(id) ON DELETE CASCADE,
    related_issue_id TEXT NOT NULL REFERENCES kanban_issues(id) ON DELETE CASCADE,
    relationship_type TEXT NOT NULL CHECK(relationship_type IN ('blocking', 'related', 'has_duplicate')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(issue_id, related_issue_id, relationship_type)
);

-- Issue comments
CREATE TABLE IF NOT EXISTS kanban_issue_comments (
    id TEXT PRIMARY KEY NOT NULL,
    issue_id TEXT NOT NULL REFERENCES kanban_issues(id) ON DELETE CASCADE,
    author_id TEXT,
    parent_id TEXT REFERENCES kanban_issue_comments(id) ON DELETE CASCADE,
    message TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Comment reactions
CREATE TABLE IF NOT EXISTS kanban_issue_comment_reactions (
    id TEXT PRIMARY KEY NOT NULL,
    comment_id TEXT NOT NULL REFERENCES kanban_issue_comments(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    emoji TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(comment_id, user_id, emoji)
);

-- Pull requests linked to issues
CREATE TABLE IF NOT EXISTS kanban_pull_requests (
    id TEXT PRIMARY KEY NOT NULL,
    url TEXT NOT NULL UNIQUE,
    number INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open', 'merged', 'closed')),
    merged_at TEXT,
    merge_commit_sha TEXT,
    target_branch_name TEXT NOT NULL,
    issue_id TEXT NOT NULL REFERENCES kanban_issues(id) ON DELETE CASCADE,
    workspace_id TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Workspaces (work sessions linked to issues)
CREATE TABLE IF NOT EXISTS kanban_workspaces (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES kanban_projects(id) ON DELETE CASCADE,
    owner_user_id TEXT NOT NULL,
    issue_id TEXT REFERENCES kanban_issues(id) ON DELETE SET NULL,
    local_workspace_id TEXT UNIQUE,
    name TEXT,
    archived INTEGER NOT NULL DEFAULT 0,
    files_changed INTEGER,
    lines_added INTEGER,
    lines_removed INTEGER,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Notifications
CREATE TABLE IF NOT EXISTS kanban_notifications (
    id TEXT PRIMARY KEY NOT NULL,
    organization_id TEXT NOT NULL REFERENCES kanban_organizations(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    notification_type TEXT NOT NULL,
    payload TEXT NOT NULL DEFAULT '{}',
    issue_id TEXT,
    comment_id TEXT,
    seen INTEGER NOT NULL DEFAULT 0,
    dismissed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Seed default data: a local user, personal org, default project with statuses and tags.
-- Uses fixed UUIDs so they're predictable.
INSERT OR IGNORE INTO kanban_users (id, email, first_name, last_name, username)
VALUES ('00000000-0000-0000-0000-000000000001', 'local@vibe-kanban.local', 'Local', 'User', 'local-user');

INSERT OR IGNORE INTO kanban_organizations (id, name, slug, is_personal)
VALUES ('00000000-0000-0000-0000-000000000010', 'My Workspace', 'my-workspace', 1);

INSERT OR IGNORE INTO kanban_organization_members (organization_id, user_id, role)
VALUES ('00000000-0000-0000-0000-000000000010', '00000000-0000-0000-0000-000000000001', 'ADMIN');

INSERT OR IGNORE INTO kanban_projects (id, organization_id, name, color)
VALUES ('00000000-0000-0000-0000-000000000100', '00000000-0000-0000-0000-000000000010', 'My Project', '#6366f1');

-- Default statuses (kanban columns)
INSERT OR IGNORE INTO kanban_project_statuses (id, project_id, name, color, sort_order, hidden) VALUES
('00000000-0000-0000-0000-000000001001', '00000000-0000-0000-0000-000000000100', 'Backlog', '#6b7280', 0, 0),
('00000000-0000-0000-0000-000000001002', '00000000-0000-0000-0000-000000000100', 'Todo', '#3b82f6', 1, 0),
('00000000-0000-0000-0000-000000001003', '00000000-0000-0000-0000-000000000100', 'In Progress', '#f59e0b', 2, 0),
('00000000-0000-0000-0000-000000001004', '00000000-0000-0000-0000-000000000100', 'In Review', '#8b5cf6', 3, 0),
('00000000-0000-0000-0000-000000001005', '00000000-0000-0000-0000-000000000100', 'Done', '#10b981', 4, 0),
('00000000-0000-0000-0000-000000001006', '00000000-0000-0000-0000-000000000100', 'Cancelled', '#ef4444', 5, 1);

-- Default tags
INSERT OR IGNORE INTO kanban_tags (id, project_id, name, color) VALUES
('00000000-0000-0000-0000-000000002001', '00000000-0000-0000-0000-000000000100', 'Bug', '#ef4444'),
('00000000-0000-0000-0000-000000002002', '00000000-0000-0000-0000-000000000100', 'Feature', '#3b82f6'),
('00000000-0000-0000-0000-000000002003', '00000000-0000-0000-0000-000000000100', 'Enhancement', '#10b981'),
('00000000-0000-0000-0000-000000002004', '00000000-0000-0000-0000-000000000100', 'Documentation', '#6b7280');
