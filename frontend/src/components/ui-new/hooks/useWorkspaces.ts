import { useCallback, useMemo } from 'react';
import { useQuery, keepPreviousData } from '@tanstack/react-query';
import { useJsonPatchWsStream } from '@/hooks/useJsonPatchWsStream';
import type {
  WorkspaceWithStatus,
  WorkspaceSummary,
  WorkspaceSummaryResponse,
  ApiResponse,
} from 'shared/types';

// UI-specific workspace type for sidebar display
export interface SidebarWorkspace {
  id: string;
  taskId: string;
  name: string;
  branch: string;
  description: string;
  filesChanged?: number;
  linesAdded?: number;
  linesRemoved?: number;
  isRunning?: boolean;
  isPinned?: boolean;
  isArchived?: boolean;
  hasPendingApproval?: boolean;
  hasRunningDevServer?: boolean;
  hasUnseenActivity?: boolean;
  latestProcessCompletedAt?: string;
  latestProcessStatus?: 'running' | 'completed' | 'failed' | 'killed';
  prStatus?: 'open' | 'merged' | 'closed' | 'unknown';
}

// Keep the old export name for backwards compatibility
export type Workspace = SidebarWorkspace;

export interface UseWorkspacesResult {
  workspaces: SidebarWorkspace[];
  archivedWorkspaces: SidebarWorkspace[];
  isLoading: boolean;
  isConnected: boolean;
  error: string | null;
}

// State shape from the WebSocket stream
type WorkspacesState = {
  workspaces: Record<string, WorkspaceWithStatus>;
};

// Transform WorkspaceWithStatus to SidebarWorkspace, optionally merging summary data
function toSidebarWorkspace(
  ws: WorkspaceWithStatus,
  summary?: WorkspaceSummary
): SidebarWorkspace {
  return {
    id: ws.id,
    taskId: ws.task_id,
    name: ws.name ?? ws.branch, // Use name if available, fallback to branch
    branch: ws.branch,
    description: '',
    // Use real stats from summary if available
    filesChanged: summary?.files_changed ?? undefined,
    linesAdded: summary?.lines_added ?? undefined,
    linesRemoved: summary?.lines_removed ?? undefined,
    // Real data from stream
    isRunning: ws.is_running,
    isPinned: ws.pinned,
    isArchived: ws.archived,
    // Additional data from summary
    hasPendingApproval: summary?.has_pending_approval,
    hasRunningDevServer: summary?.has_running_dev_server,
    hasUnseenActivity: summary?.has_unseen_turns,
    latestProcessCompletedAt: summary?.latest_process_completed_at ?? undefined,
    latestProcessStatus: summary?.latest_process_status ?? undefined,
    prStatus: summary?.pr_status ?? undefined,
  };
}

export const workspaceKeys = {
  all: ['workspaces'] as const,
};

// Query key factory for workspace summaries
export const workspaceSummaryKeys = {
  all: ['workspace-summaries'] as const,
  byArchived: (archived: boolean) =>
    ['workspace-summaries', archived ? 'archived' : 'active'] as const,
};

// Fetch workspace summaries from the API by archived status
async function fetchWorkspaceSummariesByArchived(
  archived: boolean
): Promise<Map<string, WorkspaceSummary>> {
  try {
    const response = await fetch('/api/task-attempts/summary', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ archived }),
    });

    if (!response.ok) {
      console.warn('Failed to fetch workspace summaries:', response.status);
      return new Map();
    }

    const data: ApiResponse<WorkspaceSummaryResponse> = await response.json();
    if (!data.success || !data.data?.summaries) {
      return new Map();
    }

    const map = new Map<string, WorkspaceSummary>();
    for (const summary of data.data.summaries) {
      map.set(summary.workspace_id, summary);
    }
    return map;
  } catch (err) {
    console.warn('Error fetching workspace summaries:', err);
    return new Map();
  }
}

export function useWorkspaces(): UseWorkspacesResult {
  // Single WebSocket connection for all workspaces (no archived filter).
  // The backend streams all workspaces when no archived param is provided;
  // we split active vs archived client-side using the ws.archived field.
  const endpoint = '/api/task-attempts/stream/ws';

  const initialData = useCallback(
    (): WorkspacesState => ({ workspaces: {} }),
    []
  );

  const {
    data,
    isConnected,
    isInitialized,
    error,
  } = useJsonPatchWsStream<WorkspacesState>(endpoint, true, initialData);

  // Fetch summaries for active workspaces once the stream is ready
  const { data: activeSummaries = new Map<string, WorkspaceSummary>() } =
    useQuery({
      queryKey: workspaceSummaryKeys.byArchived(false),
      queryFn: () => fetchWorkspaceSummariesByArchived(false),
      enabled: isInitialized,
      staleTime: 30_000,
      refetchInterval: 30_000,
      refetchOnWindowFocus: false,
      placeholderData: keepPreviousData,
    });

  // Fetch summaries for archived workspaces once the stream is ready
  const { data: archivedSummaries = new Map<string, WorkspaceSummary>() } =
    useQuery({
      queryKey: workspaceSummaryKeys.byArchived(true),
      queryFn: () => fetchWorkspaceSummariesByArchived(true),
      enabled: isInitialized,
      staleTime: 30_000,
      refetchInterval: 30_000,
      refetchOnWindowFocus: false,
      placeholderData: keepPreviousData,
    });

  const workspaces = useMemo(() => {
    if (!data?.workspaces) return [];
    return Object.values(data.workspaces)
      .filter((ws) => !ws.archived)
      .sort((a, b) => {
        // First sort by pinned (pinned first)
        if (a.pinned !== b.pinned) {
          return a.pinned ? -1 : 1;
        }
        // Then by created_at (newest first)
        return (
          new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
        );
      })
      .map((ws) => toSidebarWorkspace(ws, activeSummaries.get(ws.id)));
  }, [data, activeSummaries]);

  const archivedWorkspaces = useMemo(() => {
    if (!data?.workspaces) return [];
    return Object.values(data.workspaces)
      .filter((ws) => ws.archived)
      .sort((a, b) => {
        // First sort by pinned (pinned first)
        if (a.pinned !== b.pinned) {
          return a.pinned ? -1 : 1;
        }
        // Then by created_at (newest first)
        return (
          new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
        );
      })
      .map((ws) => toSidebarWorkspace(ws, archivedSummaries.get(ws.id)));
  }, [data, archivedSummaries]);

  // isLoading is true when we haven't received initial data from the stream
  const isLoading = !isInitialized;

  return {
    workspaces,
    archivedWorkspaces,
    isLoading,
    isConnected,
    error,
  };
}
