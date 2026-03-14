import { useMemo } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useJsonPatchWsStream } from './useJsonPatchWsStream';

interface UseRealtimeQueryOptions<TWs extends object, THttp> {
  // WebSocket config
  wsEndpoint: string | undefined;
  wsEnabled: boolean;
  wsInitialData: () => TWs;

  // HTTP polling config
  queryKey: readonly unknown[];
  queryFn: () => Promise<THttp>;
  httpEnabled?: boolean;
  pollInterval?: number; // Polling interval when WS is down (default: 5000)

  // Data merging
  selectWsData: (wsData: TWs) => THttp;
}

interface UseRealtimeQueryResult<THttp> {
  data: THttp | undefined;
  isLoading: boolean;
  isConnected: boolean;
  source: 'ws' | 'http' | 'none';
  error: Error | null;
}

export function useRealtimeQuery<TWs extends object, THttp>(
  options: UseRealtimeQueryOptions<TWs, THttp>
): UseRealtimeQueryResult<THttp> {
  const {
    wsEndpoint,
    wsEnabled,
    wsInitialData,
    queryKey,
    queryFn,
    httpEnabled = true,
    pollInterval = 5000,
    selectWsData,
  } = options;

  // WebSocket stream (primary)
  const {
    data: wsRawData,
    isConnected,
    isInitialized: wsInitialized,
    error: wsError,
  } = useJsonPatchWsStream<TWs>(wsEndpoint, wsEnabled, wsInitialData);

  // Transform WS data to match HTTP shape
  const wsData = useMemo(
    () => (wsRawData ? selectWsData(wsRawData) : undefined),
    [wsRawData, selectWsData]
  );

  // HTTP polling (fallback) -- only poll when WS is not connected
  const shouldPoll = httpEnabled && !isConnected;
  const {
    data: httpData,
    isLoading: httpLoading,
    error: httpError,
  } = useQuery({
    queryKey,
    queryFn,
    enabled: shouldPoll,
    refetchInterval: shouldPoll ? pollInterval : false,
    staleTime: pollInterval,
  });

  // Prefer WS data when connected and initialized
  const data = isConnected && wsInitialized ? wsData : httpData;
  const source: 'ws' | 'http' | 'none' =
    isConnected && wsInitialized ? 'ws' : httpData !== undefined ? 'http' : 'none';
  const isLoading = isConnected ? !wsInitialized && !wsError : httpLoading;
  const error = useMemo(() => {
    const msg = isConnected ? wsError : (httpError?.message ?? null);
    return msg ? new Error(msg) : null;
  }, [isConnected, wsError, httpError]);

  return { data, isLoading, isConnected, source, error };
}
