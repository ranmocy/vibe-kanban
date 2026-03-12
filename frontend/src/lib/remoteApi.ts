import type {
  UpdateIssueRequest,
  UpdateProjectStatusRequest,
} from 'shared/remote-types';

// No longer need a remote API URL - everything is local
export const REMOTE_API_URL = '';

export const makeRequest = async (
  path: string,
  options: RequestInit = {},
): Promise<Response> => {
  const headers = new Headers(options.headers ?? {});
  if (!headers.has('Content-Type')) {
    headers.set('Content-Type', 'application/json');
  }

  // Route mutations through local backend
  // The mutation URLs from remote-types.ts are like '/v1/issues'
  // Map them to local: '/api/kanban/issues'
  const localPath = path.startsWith('/v1/')
    ? `/api/kanban/${path.slice(4)}`
    : path.startsWith('/api/')
      ? path
      : `/api/kanban${path}`;

  return fetch(localPath, {
    ...options,
    headers,
  });
};

export interface BulkUpdateIssueItem {
  id: string;
  changes: Partial<UpdateIssueRequest>;
}

export async function bulkUpdateIssues(
  updates: BulkUpdateIssueItem[]
): Promise<void> {
  const response = await makeRequest('/api/kanban/issues/bulk', {
    method: 'POST',
    body: JSON.stringify({
      updates: updates.map((u) => ({ id: u.id, ...u.changes })),
    }),
  });
  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.message || 'Failed to bulk update issues');
  }
}

export interface BulkUpdateProjectStatusItem {
  id: string;
  changes: Partial<UpdateProjectStatusRequest>;
}

export async function bulkUpdateProjectStatuses(
  updates: BulkUpdateProjectStatusItem[]
): Promise<void> {
  const response = await makeRequest('/api/kanban/project_statuses/bulk', {
    method: 'POST',
    body: JSON.stringify({
      updates: updates.map((u) => ({ id: u.id, ...u.changes })),
    }),
  });
  if (!response.ok) {
    const error = await response.json();
    throw new Error(error.message || 'Failed to bulk update project statuses');
  }
}
