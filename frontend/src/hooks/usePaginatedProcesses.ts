import { useState, useCallback, useEffect, useRef } from 'react';
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

export function usePaginatedProcesses(
  sessionId: string | undefined
): PaginatedProcessesResult {
  const [processes, setProcesses] = useState<ExecutionProcess[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [isInitialLoading, setIsInitialLoading] = useState(true);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const nextCursorRef = useRef<string | null>(null);
  const loadingRef = useRef(false);

  const fetchPage = useCallback(
    async (
      sid: string,
      limit: number,
      before?: string | null
    ): Promise<HistoryResponse> => {
      const params = new URLSearchParams({
        session_id: sid,
        limit: String(limit),
        show_soft_deleted: 'true',
      });
      if (before) params.set('before', before);
      const res = await fetch(
        `/api/execution-processes/history?${params}`
      );
      if (!res.ok) throw new Error(`History fetch failed: ${res.status}`);
      return res.json() as Promise<HistoryResponse>;
    },
    []
  );

  // Load initial page on sessionId change
  useEffect(() => {
    if (!sessionId) return;
    let cancelled = false;

    setIsInitialLoading(true);
    setError(null);
    nextCursorRef.current = null;
    loadingRef.current = false;

    fetchPage(sessionId, INITIAL_PAGE_SIZE)
      .then((data) => {
        if (cancelled) return;
        setProcesses(data.processes);
        setHasMore(data.has_more);
        nextCursorRef.current = data.next_cursor;
      })
      .catch((err) => {
        if (cancelled) return;
        const message =
          err instanceof Error ? err.message : 'Failed to load history';
        setError(message);
      })
      .finally(() => {
        if (!cancelled) setIsInitialLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [sessionId, fetchPage]);

  const loadMore = useCallback(async (): Promise<ExecutionProcess[]> => {
    if (!sessionId || !hasMore || loadingRef.current) return [];
    loadingRef.current = true;
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
  }, [sessionId, hasMore, fetchPage]);

  const reset = useCallback(() => {
    setProcesses([]);
    setHasMore(false);
    setIsInitialLoading(true);
    setIsLoadingMore(false);
    setError(null);
    nextCursorRef.current = null;
    loadingRef.current = false;
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
