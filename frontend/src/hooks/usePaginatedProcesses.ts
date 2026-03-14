import { useState, useCallback, useEffect, useRef } from 'react';
import { useQuery } from '@tanstack/react-query';
import type { ExecutionProcess } from 'shared/types';

interface PaginatedProcessesResult {
  /** All loaded processes in chronological order */
  processes: ExecutionProcess[];
  /** Whether more pages exist */
  hasMore: boolean;
  /** Whether the initial page is loading */
  isInitialLoading: boolean;
  /** Whether a subsequent page is loading */
  isLoadingMore: boolean;
  /** Load the next page of older processes. Returns the newly fetched processes
   *  directly so callers don't need to wait for React state propagation. */
  loadMore: () => Promise<ExecutionProcess[]>;
  /** Reset state (on session change) */
  reset: () => void;
  /** Error from the last failed load, if any */
  error: string | null;
}

interface HistoryResponse {
  processes: ExecutionProcess[];
  has_more: boolean;
  next_cursor: string | null;
}

const INITIAL_PAGE_SIZE = 10;
const LOAD_MORE_PAGE_SIZE = 10;

async function fetchPage(
  sid: string,
  limit: number,
  before?: string | null
): Promise<HistoryResponse> {
  const params = new URLSearchParams({
    session_id: sid,
    limit: String(limit),
    show_soft_deleted: 'true',
  });
  if (before) params.set('before', before);
  const res = await fetch(`/api/execution-processes/history?${params}`);
  if (!res.ok) throw new Error(`History fetch failed: ${res.status}`);
  return res.json() as Promise<HistoryResponse>;
}

export function usePaginatedProcesses(
  sessionId: string | undefined
): PaginatedProcessesResult {
  const [processes, setProcesses] = useState<ExecutionProcess[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const nextCursorRef = useRef<string | null>(null);
  const loadingRef = useRef(false);
  const hasLoadedMoreRef = useRef(false);

  const { data: initialPage, isLoading: isInitialLoading } = useQuery({
    queryKey: ['execution-processes', 'history', sessionId, INITIAL_PAGE_SIZE],
    queryFn: () => fetchPage(sessionId!, INITIAL_PAGE_SIZE),
    enabled: !!sessionId,
    staleTime: 10000,
  });

  useEffect(() => {
    hasLoadedMoreRef.current = false;
    nextCursorRef.current = null;
    loadingRef.current = false;
  }, [sessionId]);

  useEffect(() => {
    if (!initialPage) return;
    if (hasLoadedMoreRef.current) return;
    setProcesses(initialPage.processes);
    setHasMore(initialPage.has_more);
    nextCursorRef.current = initialPage.next_cursor;
  }, [initialPage]);

  const loadMore = useCallback(async (): Promise<ExecutionProcess[]> => {
    if (!sessionId || !hasMore || loadingRef.current) return [];
    loadingRef.current = true;
    hasLoadedMoreRef.current = true;
    setIsLoadingMore(true);
    setError(null);

    try {
      const data = await fetchPage(
        sessionId,
        LOAD_MORE_PAGE_SIZE,
        nextCursorRef.current
      );
      // Prepend older processes before existing ones
      setProcesses((prev) => [...data.processes, ...prev]);
      setHasMore(data.has_more);
      nextCursorRef.current = data.next_cursor;
      return data.processes;
    } catch (err) {
      const message =
        err instanceof Error ? err.message : 'Failed to load more';
      setError(message);
      return [];
    } finally {
      loadingRef.current = false;
      setIsLoadingMore(false);
    }
  }, [sessionId, hasMore]);

  const reset = useCallback(() => {
    setProcesses([]);
    setHasMore(false);
    setIsLoadingMore(false);
    setError(null);
    nextCursorRef.current = null;
    loadingRef.current = false;
    hasLoadedMoreRef.current = false;
  }, []);

  return {
    processes,
    hasMore,
    isInitialLoading,
    isLoadingMore,
    loadMore,
    reset,
    error,
  };
}
