import { useQuery } from '@tanstack/react-query';

export function useCurrentUser() {
  return useQuery({
    queryKey: ['auth', 'user'],
    queryFn: async () => ({
      user_id: '00000000-0000-0000-0000-000000000001',
      email: 'local@vibe-kanban.local',
      first_name: 'Local',
      last_name: 'User',
    }),
    staleTime: Infinity,
  });
}
