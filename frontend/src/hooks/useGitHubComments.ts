import { useMemo, useCallback, useState, useEffect } from 'react';
import { useQueries } from '@tanstack/react-query';
import { attemptsApi } from '@/lib/api';
import { prCommentsKeys } from './usePrComments';
import {
  usePersistedExpanded,
  PERSIST_KEYS,
} from '@/stores/useUiPreferencesStore';
import type { UnifiedPrComment } from 'shared/types';
import { DiffSide } from '@/types/diff';

/**
 * Normalized GitHub comment for diff view display
 */
export interface NormalizedGitHubComment {
  id: string;
  author: string;
  body: string;
  createdAt: string;
  url: string | null;
  filePath: string;
  lineNumber: number;
  side: DiffSide;
  diffHunk: string | null;
}

interface UseGitHubCommentsOptions {
  workspaceId?: string;
  repoIds: string[];
  enabled?: boolean;
}

interface UseGitHubCommentsResult {
  gitHubComments: UnifiedPrComment[];
  isGitHubCommentsLoading: boolean;
  showGitHubComments: boolean;
  setShowGitHubComments: (show: boolean) => void;
  getGitHubCommentsForFile: (filePath: string) => NormalizedGitHubComment[];
  getGitHubCommentCountForFile: (filePath: string) => number;
  getFilesWithGitHubComments: () => string[];
  getFirstCommentLineForFile: (filePath: string) => number | null;
}

export function useGitHubComments({
  workspaceId,
  repoIds,
  enabled = true,
}: UseGitHubCommentsOptions): UseGitHubCommentsResult {
  // GitHub comments toggle state (persisted)
  const [showGitHubComments, setShowGitHubComments] = usePersistedExpanded(
    PERSIST_KEYS.showGitHubComments,
    true // Default to shown
  );

  // Defer activation until after first paint so comments never block LCP
  const [afterFirstPaint, setAfterFirstPaint] = useState(false);

  useEffect(() => {
    let handle: number | undefined;

    const schedule =
      typeof requestIdleCallback !== 'undefined'
        ? (cb: () => void) => {
            handle = requestIdleCallback(cb, { timeout: 2000 });
          }
        : (cb: () => void) => {
            handle = requestAnimationFrame(() => requestAnimationFrame(cb));
          };

    schedule(() => setAfterFirstPaint(true));

    return () => {
      if (handle !== undefined) {
        if (typeof cancelIdleCallback !== 'undefined') {
          cancelIdleCallback(handle);
        } else {
          cancelAnimationFrame(handle);
        }
      }
    };
  }, []);

  // Fetch PR comments for all repos with PRs in parallel
  const queries = useQueries({
    queries: repoIds.map((repoId) => ({
      queryKey: prCommentsKeys.byAttempt(workspaceId, repoId),
      queryFn: () => attemptsApi.getPrComments(workspaceId!, repoId),
      enabled: enabled && !!workspaceId && afterFirstPaint,
      staleTime: 30_000,
      gcTime: 60_000,
      refetchOnMount: false,
      retry: 2,
    })),
  });

  const isGitHubCommentsLoading = queries.some((q) => q.isLoading);

  // Stable reference: only recompute when query data actually changes
  const queryDataKey = queries.map((q) => q.dataUpdatedAt).join(',');
  const gitHubComments = useMemo(() => {
    const all: UnifiedPrComment[] = [];
    for (const q of queries) {
      if (q.data?.comments) {
        all.push(...q.data.comments);
      }
    }
    return all;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [queryDataKey]);

  // Normalize GitHub review comments for file matching
  const normalizedComments = useMemo(() => {
    const normalized: NormalizedGitHubComment[] = [];
    for (const comment of gitHubComments) {
      if (comment.comment_type !== 'review') continue;
      if (comment.line === null) continue; // Skip file-level comments

      normalized.push({
        id: String(comment.id),
        author: comment.author,
        body: comment.body,
        createdAt: comment.created_at,
        url: comment.url,
        filePath: comment.path,
        lineNumber: Number(comment.line),
        // Use side from API: "LEFT" = old/deleted side, "RIGHT" = new/added side (default)
        side: comment.side === 'LEFT' ? DiffSide.Old : DiffSide.New,
        diffHunk: comment.diff_hunk,
      });
    }
    return normalized;
  }, [gitHubComments]);

  // Helper to match paths - handles repo prefix in diff paths
  // GitHub paths: "frontend/src/file.ts"
  // Diff paths: "vibe-kanban/frontend/src/file.ts" (prefixed with repo name)
  const pathMatches = useCallback(
    (diffPath: string, githubPath: string): boolean => {
      return diffPath === githubPath || diffPath.endsWith('/' + githubPath);
    },
    []
  );

  // Get comments for a specific file (handles prefixed paths)
  const getGitHubCommentsForFile = useCallback(
    (filePath: string): NormalizedGitHubComment[] => {
      return normalizedComments.filter((c) =>
        pathMatches(filePath, c.filePath)
      );
    },
    [normalizedComments, pathMatches]
  );

  // Get comment count for a specific file (handles prefixed paths)
  const getGitHubCommentCountForFile = useCallback(
    (filePath: string): number => {
      return normalizedComments.filter((c) => pathMatches(filePath, c.filePath))
        .length;
    },
    [normalizedComments, pathMatches]
  );

  // Get list of unique file paths that have GitHub comments
  const getFilesWithGitHubComments = useCallback((): string[] => {
    const filesSet = new Set<string>();
    for (const comment of normalizedComments) {
      filesSet.add(comment.filePath);
    }
    return Array.from(filesSet);
  }, [normalizedComments]);

  // Get the first (lowest line number) comment's line for a file
  const getFirstCommentLineForFile = useCallback(
    (filePath: string): number | null => {
      const comments = normalizedComments.filter((c) =>
        pathMatches(filePath, c.filePath)
      );
      if (comments.length === 0) return null;
      return Math.min(...comments.map((c) => c.lineNumber));
    },
    [normalizedComments, pathMatches]
  );

  return {
    gitHubComments,
    isGitHubCommentsLoading,
    showGitHubComments,
    setShowGitHubComments,
    getGitHubCommentsForFile,
    getGitHubCommentCountForFile,
    getFilesWithGitHubComments,
    getFirstCommentLineForFile,
  };
}
