import { useState, useMemo, useCallback, useRef, useEffect } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { generateUuid } from '@/lib/uuid';
import { makeRequest } from '@/lib/remoteApi';
import type { MutationDefinition, ShapeDefinition } from 'shared/remote-types';
import type { SyncError } from './types';

// Type helpers for extracting types from MutationDefinition
type MutationCreateType<M> =
  M extends MutationDefinition<unknown, infer C, unknown> ? C : never;
type MutationUpdateType<M> =
  M extends MutationDefinition<unknown, unknown, infer U> ? U : never;

/**
 * Result of an optimistic mutation operation.
 * Contains a promise that resolves when the backend confirms the change.
 */
export interface MutationResult {
  /** Promise that resolves when the mutation is confirmed by the backend */
  persisted: Promise<void>;
}

/**
 * Result of an insert operation, including the created row data.
 */
export interface InsertResult<TRow> {
  /** The optimistically created row with generated ID */
  data: TRow;
  /** Promise that resolves with the synced row (including server-generated fields) when confirmed by backend */
  persisted: Promise<TRow>;
}

/**
 * Base result type returned by useShape (read-only).
 */
export interface UseShapeResult<TRow> {
  /** The synced data array */
  data: TRow[];
  /** Whether the initial sync is still loading */
  isLoading: boolean;
  /** Sync error if one occurred */
  error: SyncError | null;
  /** Function to retry after an error */
  retry: () => void;
}

/**
 * Extended result when mutation is provided — adds insert/update/remove.
 */
export interface UseShapeMutationResult<TRow, TCreate, TUpdate>
  extends UseShapeResult<TRow> {
  /** Insert a new row (optimistic), returns row and persistence promise */
  insert: (data: TCreate) => InsertResult<TRow>;
  /** Update a row by ID (optimistic), returns persistence promise */
  update: (id: string, changes: Partial<TUpdate>) => MutationResult;
  /** Delete a row by ID (optimistic), returns persistence promise */
  remove: (id: string) => MutationResult;
}

/**
 * Options for the useShape hook.
 */
export interface UseShapeOptions<
  M extends
    | MutationDefinition<unknown, unknown, unknown>
    | undefined = undefined,
> {
  /**
   * Whether to enable the data subscription.
   * When false, returns empty data and no-op mutation functions.
   * @default true
   */
  enabled?: boolean;
  /**
   * Optional mutation definition. When provided, the hook returns
   * insert/update/remove functions for optimistic mutations.
   */
  mutation?: M;
}

/**
 * Build a local API URL from a shape definition URL template and params.
 * Maps Electric shape URLs to local kanban REST endpoints.
 *
 * Organization-scoped shapes (e.g. /v1/shape/projects with organization_id param)
 * are mapped to nested REST paths: /api/kanban/organizations/{org_id}/projects
 */
function buildLocalUrl(
  shapeUrl: string,
  params: Record<string, string>
): string {
  // Strip /v1/shape prefix and add /api/kanban prefix
  let url = shapeUrl.replace(/^\/v1\/shape/, '/api/kanban');

  // Substitute params in URL template
  for (const [key, value] of Object.entries(params)) {
    url = url.replace(`{${key}}`, encodeURIComponent(value));
  }

  // Find params that weren't substituted in the URL template
  const remainingParams = Object.entries(params).filter(
    ([key]) => !shapeUrl.includes(`{${key}}`)
  );

  // If organization_id is a remaining param, nest under /organizations/{org_id}/
  const orgEntry = remainingParams.find(([key]) => key === 'organization_id');
  if (orgEntry) {
    const [, orgId] = orgEntry;
    url = url.replace(
      '/api/kanban/',
      `/api/kanban/organizations/${encodeURIComponent(orgId)}/`
    );

    // Add remaining params (excluding organization_id) as query params
    const queryParams = remainingParams.filter(
      ([key]) => key !== 'organization_id'
    );
    if (queryParams.length > 0) {
      const searchParams = new URLSearchParams(queryParams);
      url += (url.includes('?') ? '&' : '?') + searchParams.toString();
    }
  } else if (remainingParams.length > 0) {
    const searchParams = new URLSearchParams(remainingParams);
    url += (url.includes('?') ? '&' : '?') + searchParams.toString();
  }

  return url;
}

/**
 * Build a mutation API URL from a mutation definition URL.
 * Maps /v1/... to /api/kanban/...
 */
function buildMutationUrl(mutationUrl: string): string {
  return mutationUrl.startsWith('/v1/')
    ? `/api/kanban/${mutationUrl.slice(4)}`
    : mutationUrl;
}

/**
 * Hook for subscribing to a shape's data via React Query polling,
 * with optional optimistic mutation support.
 *
 * @param shape - The shape definition from shared/remote-types.ts
 * @param params - URL parameters matching the shape's requirements
 * @param options - Optional configuration (enabled, mutation, etc.)
 */
export function useShape<
  T extends Record<string, unknown>,
  M extends
    | MutationDefinition<unknown, unknown, unknown>
    | undefined = undefined,
>(
  shape: ShapeDefinition<T>,
  params: Record<string, string>,
  options: UseShapeOptions<M> = {} as UseShapeOptions<M>
): M extends MutationDefinition<unknown, unknown, unknown>
  ? UseShapeMutationResult<T, MutationCreateType<M>, MutationUpdateType<M>>
  : UseShapeResult<T> {
  const { enabled = true, mutation } = options;
  const [error, setError] = useState<SyncError | null>(null);

  const paramsKey = JSON.stringify(params);
  const stableParams = useMemo(
    () => JSON.parse(paramsKey) as Record<string, string>,
    [paramsKey]
  );

  const url = useMemo(
    () => buildLocalUrl(shape.url, stableParams),
    [shape.url, stableParams]
  );

  const queryKey = useMemo(
    () => ['kanban', shape.table, stableParams],
    [shape.table, stableParams]
  );

  const queryClient = useQueryClient();

  const { data: rawData, isLoading } = useQuery<T[]>({
    queryKey,
    queryFn: async () => {
      const response = await fetch(url);
      if (!response.ok) {
        const errMsg = `Failed to fetch ${shape.table}: ${response.status}`;
        setError({ message: errMsg });
        throw new Error(errMsg);
      }
      setError(null);
      return response.json();
    },
    enabled,
    refetchInterval: enabled ? 2000 : false,
    staleTime: 1000,
  });

  const items = useMemo(() => {
    if (!enabled || !rawData) return [];
    return rawData;
  }, [enabled, rawData]);

  const retry = useCallback(() => {
    setError(null);
    queryClient.invalidateQueries({ queryKey });
  }, [queryClient, queryKey]);

  // --- Mutation support (only used when mutation is provided) ---

  const itemsRef = useRef<T[]>([]);
  useEffect(() => {
    itemsRef.current = items;
  }, [items]);

  const mutationUrl = useMemo(
    () => (mutation ? buildMutationUrl(mutation.url) : ''),
    [mutation]
  );

  const insert = useCallback(
    (insertData: unknown): InsertResult<T> => {
      const dataWithId = {
        id: generateUuid(),
        ...(insertData as Record<string, unknown>),
      };

      // Optimistic update
      queryClient.setQueryData<T[]>(queryKey, (old) => [
        ...(old || []),
        dataWithId as unknown as T,
      ]);

      const persisted = (async () => {
        try {
          const response = await makeRequest(mutationUrl, {
            method: 'POST',
            body: JSON.stringify(dataWithId),
          });
          if (!response.ok) {
            const err = await response.json();
            throw new Error(err.message || 'Failed to create');
          }
          const result = await response.json();
          // Invalidate to get the server's version
          queryClient.invalidateQueries({ queryKey });
          return (result.data ?? dataWithId) as unknown as T;
        } catch (e) {
          queryClient.invalidateQueries({ queryKey });
          throw e;
        }
      })();

      return {
        data: dataWithId as unknown as T,
        persisted,
      };
    },
    [queryClient, queryKey, mutationUrl]
  );

  const update = useCallback(
    (id: string, changes: unknown): MutationResult => {
      // Optimistic update
      queryClient.setQueryData<T[]>(queryKey, (old) =>
        (old || []).map((item) =>
          (item as unknown as { id: string }).id === id
            ? { ...item, ...(changes as Record<string, unknown>) }
            : item
        )
      );

      const persisted = (async () => {
        try {
          const response = await makeRequest(`${mutationUrl}/${id}`, {
            method: 'PATCH',
            body: JSON.stringify(changes),
          });
          if (!response.ok) {
            const err = await response.json();
            throw new Error(err.message || 'Failed to update');
          }
          queryClient.invalidateQueries({ queryKey });
        } catch (e) {
          queryClient.invalidateQueries({ queryKey });
          throw e;
        }
      })();

      return { persisted };
    },
    [queryClient, queryKey, mutationUrl]
  );

  const remove = useCallback(
    (id: string): MutationResult => {
      // Optimistic update
      queryClient.setQueryData<T[]>(queryKey, (old) =>
        (old || []).filter(
          (item) => (item as unknown as { id: string }).id !== id
        )
      );

      const persisted = (async () => {
        try {
          const response = await makeRequest(`${mutationUrl}/${id}`, {
            method: 'DELETE',
          });
          if (!response.ok) {
            const err = await response.json();
            throw new Error(err.message || 'Failed to delete');
          }
          queryClient.invalidateQueries({ queryKey });
        } catch (e) {
          queryClient.invalidateQueries({ queryKey });
          throw e;
        }
      })();

      return { persisted };
    },
    [queryClient, queryKey, mutationUrl]
  );

  const base: UseShapeResult<T> = {
    data: items,
    isLoading: enabled ? isLoading : false,
    error,
    retry,
  };

  if (mutation) {
    return {
      ...base,
      insert,
      update,
      remove,
    } as M extends MutationDefinition<unknown, unknown, unknown>
      ? UseShapeMutationResult<T, MutationCreateType<M>, MutationUpdateType<M>>
      : UseShapeResult<T>;
  }

  return base as M extends MutationDefinition<unknown, unknown, unknown>
    ? UseShapeMutationResult<T, MutationCreateType<M>, MutationUpdateType<M>>
    : UseShapeResult<T>;
}
