import { create } from 'zustand';

interface WsConnectionStatusState {
  connections: Record<string, boolean>;
  setConnected: (endpoint: string, connected: boolean) => void;
  isAnyConnected: () => boolean;
  isConnected: (endpoint: string) => boolean;
}

export const useWsConnectionStatus = create<WsConnectionStatusState>(
  (set, get) => ({
    connections: {},
    setConnected: (endpoint, connected) =>
      set((state) => ({
        connections: { ...state.connections, [endpoint]: connected },
      })),
    isAnyConnected: () => Object.values(get().connections).some(Boolean),
    isConnected: (endpoint) => get().connections[endpoint] ?? false,
  })
);
