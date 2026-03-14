import { useState, useEffect, useMemo, useCallback } from 'react';
import type { Project } from 'shared/remote-types';
import { useUserOrganizations } from '@/hooks/useUserOrganizations';

/**
 * Hook that fetches remote projects across ALL user organizations.
 * Uses direct fetch to local kanban API with polling.
 */
export function useAllOrganizationProjects() {
  const { data: orgsData } = useUserOrganizations();

  const orgIds = useMemo(
    () => (orgsData?.organizations ?? []).map((o: { id: string }) => o.id),
    [orgsData?.organizations]
  );

  const [projects, setProjects] = useState<Project[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  const fetchAll = useCallback(async () => {
    if (orgIds.length === 0) {
      setProjects([]);
      setIsLoading(false);
      return;
    }

    try {
      const results = await Promise.all(
        orgIds.map(async (orgId: string) => {
          const res = await fetch(
            `/api/kanban/organizations/${orgId}/projects`
          );
          if (!res.ok) return [];
          return res.json() as Promise<Project[]>;
        })
      );
      setProjects(results.flat());
    } catch {
      // Keep existing data on error
    } finally {
      setIsLoading(false);
    }
  }, [orgIds]);

  useEffect(() => {
    void fetchAll();
  }, [fetchAll]);

  return { data: projects, isLoading };
}
