import { useQuery } from '@tanstack/react-query';

interface ProjectItem {
  id: string;
  organization_id: string;
  name: string;
  color: string;
  created_at: string;
  updated_at: string;
}

export function useOrganizationProjects(organizationId: string | null) {
  const enabled = !!organizationId;

  const { data, isLoading, error } = useQuery<ProjectItem[]>({
    queryKey: ['kanban', 'projects', organizationId],
    queryFn: async () => {
      const res = await fetch(
        `/api/kanban/organizations/${organizationId}/projects`
      );
      if (!res.ok) throw new Error('Failed to fetch projects');
      return res.json();
    },
    enabled,
    staleTime: 30_000,
    refetchInterval: 30_000,
  });

  return {
    data: data || [],
    isLoading,
    isError: !!error,
    error,
  };
}
