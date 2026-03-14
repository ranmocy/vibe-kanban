import { useContext, ReactNode, useMemo, useCallback, useEffect } from 'react';
import { createHmrContext } from '@/lib/hmrContext.ts';
import { useParams, useNavigate, useLocation } from 'react-router-dom';
import { useQueryClient } from '@tanstack/react-query';
import {
  useWorkspaces,
  workspaceSummaryKeys,
  type SidebarWorkspace,
} from '@/components/ui-new/hooks/useWorkspaces';
import { useAttempt } from '@/hooks/useAttempt';
import { useAttemptRepo } from '@/hooks/useAttemptRepo';
import { useWorkspaceSessions } from '@/hooks/useWorkspaceSessions';
import {
  useGitHubComments,
  type NormalizedGitHubComment,
} from '@/hooks/useGitHubComments';
import { useBranchStatus } from '@/hooks/useBranchStatus';
import { useDiffStream } from '@/hooks/useDiffStream';
import { attemptsApi } from '@/lib/api';
import { useDiffViewStore } from '@/stores/useDiffViewStore';
import type {
  Workspace as ApiWorkspace,
  Session,
  RepoWithTargetBranch,
  UnifiedPrComment,
  Diff,
  DiffStats,
} from 'shared/types';

export type { NormalizedGitHubComment } from '@/hooks/useGitHubComments';

// ---------------------------------------------------------------------------
// Shared internal hook — reads workspaceId and isCreateMode from the URL.
// Each sub-provider calls this independently so they never subscribe to each
// other's context; the values come straight from React Router (synchronous).
// ---------------------------------------------------------------------------

function useWorkspaceRouteInfo() {
  const { workspaceId } = useParams<{ workspaceId: string }>();
  const location = useLocation();
  const isCreateMode = location.pathname === '/workspaces/create';
  return { workspaceId, isCreateMode };
}

// ===========================================================================
// 1. WorkspaceSelectionContext
//    Changes only when the user navigates to a different workspace or session.
// ===========================================================================

export interface WorkspaceSelectionContextValue {
  workspaceId: string | undefined;
  workspace: ApiWorkspace | undefined;
  isLoading: boolean;
  isCreateMode: boolean;
  selectWorkspace: (id: string) => void;
  navigateToCreate: () => void;
  sessions: Session[];
  selectedSession: Session | undefined;
  selectedSessionId: string | undefined;
  selectSession: (sessionId: string) => void;
  selectLatestSession: () => void;
  isSessionsLoading: boolean;
  isNewSessionMode: boolean;
  startNewSession: () => void;
  repos: RepoWithTargetBranch[];
  isReposLoading: boolean;
}

export const WorkspaceSelectionContext =
  createHmrContext<WorkspaceSelectionContextValue | null>(
    'WorkspaceSelectionContext',
    null
  );

function WorkspaceSelectionProvider({ children }: { children: ReactNode }) {
  const { workspaceId, isCreateMode } = useWorkspaceRouteInfo();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  // Fetch real workspace data for the selected workspace
  const { data: workspace, isLoading: isLoadingWorkspace } = useAttempt(
    workspaceId,
    { enabled: !!workspaceId && !isCreateMode }
  );

  // Fetch sessions for the current workspace
  const {
    sessions,
    selectedSession,
    selectedSessionId,
    selectSession,
    selectLatestSession,
    isLoading: isSessionsLoading,
    isNewSessionMode,
    startNewSession,
  } = useWorkspaceSessions(workspaceId, { enabled: !isCreateMode });

  // Fetch repos for the current workspace
  const { repos, isLoading: isReposLoading } = useAttemptRepo(workspaceId, {
    enabled: !isCreateMode,
  });

  const isLoading = isLoadingWorkspace;

  const selectWorkspace = useCallback(
    (id: string) => {
      attemptsApi
        .markSeen(id)
        .then(() => {
          queryClient.invalidateQueries({ queryKey: workspaceSummaryKeys.all });
        })
        .catch((error) => {
          console.warn('Failed to mark workspace as seen:', error);
        });
      navigate(`/workspaces/${id}`);
    },
    [navigate, queryClient]
  );

  const navigateToCreate = useCallback(
    () => navigate('/workspaces/create'),
    [navigate]
  );

  const value = useMemo<WorkspaceSelectionContextValue>(
    () => ({
      workspaceId,
      workspace,
      isLoading,
      isCreateMode,
      selectWorkspace,
      navigateToCreate,
      sessions,
      selectedSession,
      selectedSessionId,
      selectSession,
      selectLatestSession,
      isSessionsLoading,
      isNewSessionMode,
      startNewSession,
      repos,
      isReposLoading,
    }),
    [
      workspaceId,
      workspace,
      isLoading,
      isCreateMode,
      selectWorkspace,
      navigateToCreate,
      sessions,
      selectedSession,
      selectedSessionId,
      selectSession,
      selectLatestSession,
      isSessionsLoading,
      isNewSessionMode,
      startNewSession,
      repos,
      isReposLoading,
    ]
  );

  return (
    <WorkspaceSelectionContext.Provider value={value}>
      {children}
    </WorkspaceSelectionContext.Provider>
  );
}

export function useWorkspaceSelectionContext(): WorkspaceSelectionContextValue {
  const context = useContext(WorkspaceSelectionContext);
  if (!context) {
    throw new Error(
      'useWorkspaceSelectionContext must be used within a WorkspaceProvider'
    );
  }
  return context;
}

// ===========================================================================
// 2. WorkspaceListContext
//    Changes on workspace list polls / WebSocket updates.
// ===========================================================================

export interface WorkspaceListContextValue {
  activeWorkspaces: SidebarWorkspace[];
  archivedWorkspaces: SidebarWorkspace[];
}

export const WorkspaceListContext =
  createHmrContext<WorkspaceListContextValue | null>(
    'WorkspaceListContext',
    null
  );

function WorkspaceListProvider({ children }: { children: ReactNode }) {
  const {
    workspaces: activeWorkspaces,
    archivedWorkspaces,
  } = useWorkspaces();

  const value = useMemo<WorkspaceListContextValue>(
    () => ({ activeWorkspaces, archivedWorkspaces }),
    [activeWorkspaces, archivedWorkspaces]
  );

  return (
    <WorkspaceListContext.Provider value={value}>
      {children}
    </WorkspaceListContext.Provider>
  );
}

export function useWorkspaceListContext(): WorkspaceListContextValue {
  const context = useContext(WorkspaceListContext);
  if (!context) {
    throw new Error(
      'useWorkspaceListContext must be used within a WorkspaceProvider'
    );
  }
  return context;
}

// ===========================================================================
// 3. WorkspaceDiffContext
//    Changes on every diff stream message — highest update frequency.
// ===========================================================================

export interface WorkspaceDiffContextValue {
  diffs: Diff[];
  diffPaths: Set<string>;
  diffStats: DiffStats;
}

export const WorkspaceDiffContext =
  createHmrContext<WorkspaceDiffContextValue | null>(
    'WorkspaceDiffContext',
    null
  );

function WorkspaceDiffProvider({ children }: { children: ReactNode }) {
  const { workspaceId, isCreateMode } = useWorkspaceRouteInfo();

  const { diffs } = useDiffStream(workspaceId ?? null, !isCreateMode);

  const diffPaths = useMemo(
    () =>
      new Set(diffs.map((d) => d.newPath || d.oldPath || '').filter(Boolean)),
    [diffs]
  );

  // Sync diffPaths to store for expand/collapse all functionality
  useEffect(() => {
    useDiffViewStore.getState().setDiffPaths(Array.from(diffPaths));
    return () => useDiffViewStore.getState().setDiffPaths([]);
  }, [diffPaths]);

  const diffStats: DiffStats = useMemo(
    () => ({
      files_changed: diffs.length,
      lines_added: diffs.reduce((sum, d) => sum + (d.additions ?? 0), 0),
      lines_removed: diffs.reduce((sum, d) => sum + (d.deletions ?? 0), 0),
    }),
    [diffs]
  );

  const value = useMemo<WorkspaceDiffContextValue>(
    () => ({ diffs, diffPaths, diffStats }),
    [diffs, diffPaths, diffStats]
  );

  return (
    <WorkspaceDiffContext.Provider value={value}>
      {children}
    </WorkspaceDiffContext.Provider>
  );
}

export function useWorkspaceDiffContext(): WorkspaceDiffContextValue {
  const context = useContext(WorkspaceDiffContext);
  if (!context) {
    throw new Error(
      'useWorkspaceDiffContext must be used within a WorkspaceProvider'
    );
  }
  return context;
}

// ===========================================================================
// 4. WorkspaceGitHubContext
//    Changes when PR comments load (burst during init, then stable).
// ===========================================================================

export interface WorkspaceGitHubContextValue {
  gitHubComments: UnifiedPrComment[];
  isGitHubCommentsLoading: boolean;
  showGitHubComments: boolean;
  setShowGitHubComments: (show: boolean) => void;
  getGitHubCommentsForFile: (filePath: string) => NormalizedGitHubComment[];
  getGitHubCommentCountForFile: (filePath: string) => number;
  getFilesWithGitHubComments: () => string[];
  getFirstCommentLineForFile: (filePath: string) => number | null;
}

export const WorkspaceGitHubContext =
  createHmrContext<WorkspaceGitHubContextValue | null>(
    'WorkspaceGitHubContext',
    null
  );

function WorkspaceGitHubProvider({ children }: { children: ReactNode }) {
  const { workspaceId, isCreateMode } = useWorkspaceRouteInfo();

  // React Query deduplicates these calls — same query keys as in
  // WorkspaceSelectionProvider so no extra network traffic.
  const { repos } = useAttemptRepo(workspaceId, {
    enabled: !isCreateMode,
  });

  const { data: branchStatus } = useBranchStatus(
    !isCreateMode ? workspaceId : undefined
  );

  // Reuse the list context's activeWorkspaces to check for PR attachment.
  // We call useWorkspaceListContext here (safe: WorkspaceListProvider is an
  // ancestor of WorkspaceGitHubProvider in the nesting order).
  const { activeWorkspaces } = useWorkspaceListContext();

  const prRepoIds = useMemo(() => {
    if (!branchStatus) return repos[0]?.id ? [repos[0].id] : [];
    const ids = branchStatus
      .filter((s) => s.merges?.some((m) => m.type === 'pr'))
      .map((s) => s.repo_id);
    return ids.length > 0 ? ids : repos[0]?.id ? [repos[0].id] : [];
  }, [branchStatus, repos]);

  const currentWorkspaceSummary = activeWorkspaces.find(
    (w) => w.id === workspaceId
  );
  const hasPrAttached = !!currentWorkspaceSummary?.prStatus;

  const {
    gitHubComments,
    isGitHubCommentsLoading,
    showGitHubComments,
    setShowGitHubComments,
    getGitHubCommentsForFile,
    getGitHubCommentCountForFile,
    getFilesWithGitHubComments,
    getFirstCommentLineForFile,
  } = useGitHubComments({
    workspaceId,
    repoIds: prRepoIds,
    enabled: !isCreateMode && hasPrAttached,
  });

  const value = useMemo<WorkspaceGitHubContextValue>(
    () => ({
      gitHubComments,
      isGitHubCommentsLoading,
      showGitHubComments,
      setShowGitHubComments,
      getGitHubCommentsForFile,
      getGitHubCommentCountForFile,
      getFilesWithGitHubComments,
      getFirstCommentLineForFile,
    }),
    [
      gitHubComments,
      isGitHubCommentsLoading,
      showGitHubComments,
      setShowGitHubComments,
      getGitHubCommentsForFile,
      getGitHubCommentCountForFile,
      getFilesWithGitHubComments,
      getFirstCommentLineForFile,
    ]
  );

  return (
    <WorkspaceGitHubContext.Provider value={value}>
      {children}
    </WorkspaceGitHubContext.Provider>
  );
}

export function useWorkspaceGitHubContext(): WorkspaceGitHubContextValue {
  const context = useContext(WorkspaceGitHubContext);
  if (!context) {
    throw new Error(
      'useWorkspaceGitHubContext must be used within a WorkspaceProvider'
    );
  }
  return context;
}

// ===========================================================================
// Composite WorkspaceProvider
//    Nests the four sub-providers in order. Innermost contexts have the
//    highest update frequency; outermost are most stable.
// ===========================================================================

interface WorkspaceProviderProps {
  children: ReactNode;
}

export function WorkspaceProvider({ children }: WorkspaceProviderProps) {
  return (
    <WorkspaceListProvider>
      <WorkspaceSelectionProvider>
        <WorkspaceDiffProvider>
          <WorkspaceGitHubProvider>{children}</WorkspaceGitHubProvider>
        </WorkspaceDiffProvider>
      </WorkspaceSelectionProvider>
    </WorkspaceListProvider>
  );
}

// ===========================================================================
// Legacy WorkspaceContext type + compatibility shim
//    All 34 existing consumers continue to work without any change.
//    The shim subscribes to all four contexts, so components that still use
//    useWorkspaceContext() will re-render on any change — just like before.
//    Migrate consumers to the specific hooks to gain the perf benefit.
// ===========================================================================

interface WorkspaceContextValue
  extends WorkspaceSelectionContextValue,
    WorkspaceListContextValue,
    WorkspaceDiffContextValue,
    WorkspaceGitHubContextValue {}

/** @deprecated Use the specific sub-context hooks instead for better performance. */
export const WorkspaceContext = createHmrContext<WorkspaceContextValue | null>(
  'WorkspaceContext',
  null
);

/** @deprecated Use useWorkspaceSelectionContext / useWorkspaceListContext /
 *  useWorkspaceDiffContext / useWorkspaceGitHubContext for better performance. */
export function useWorkspaceContext(): WorkspaceContextValue {
  const selection = useWorkspaceSelectionContext();
  const list = useWorkspaceListContext();
  const diff = useWorkspaceDiffContext();
  const github = useWorkspaceGitHubContext();
  return useMemo(
    () => ({ ...selection, ...list, ...diff, ...github }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [selection, list, diff, github]
  );
}
