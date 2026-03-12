import { useQuery, type UseQueryResult } from '@tanstack/react-query';
import { organizationKeys } from './organizationKeys';
import type { ListOrganizationsResponse } from 'shared/types';

/**
 * Hook to fetch all organizations from the local kanban backend
 */
export function useUserOrganizations(): UseQueryResult<ListOrganizationsResponse> {
  return useQuery({
    queryKey: organizationKeys.userList(),
    queryFn: async (): Promise<ListOrganizationsResponse> => {
      const res = await fetch('/api/kanban/organizations');
      if (!res.ok) throw new Error('Failed to fetch organizations');
      const orgs = await res.json();
      // The local backend returns a plain array; wrap to match expected shape
      // and add user_role since we're always admin locally
      const organizations = (Array.isArray(orgs) ? orgs : []).map(
        (org: Record<string, unknown>) => ({
          ...org,
          user_role: org.user_role ?? 'ADMIN',
        })
      );
      return { organizations } as ListOrganizationsResponse;
    },
    staleTime: 5 * 60 * 1000,
  });
}
