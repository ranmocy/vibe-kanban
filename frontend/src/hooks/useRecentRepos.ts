import { useQuery } from '@tanstack/react-query';
import { repoApi } from '@/lib/api';

export function useRecentRepos() {
  return useQuery({
    queryKey: ['recentRepos'],
    queryFn: () => repoApi.listRecent(),
    staleTime: 30_000,
  });
}
