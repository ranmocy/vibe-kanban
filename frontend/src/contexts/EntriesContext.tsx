import { useContext, useState, useMemo, useCallback, ReactNode } from 'react';
import { createHmrContext } from '@/lib/hmrContext.ts';
import type { PatchTypeWithKey } from '@/hooks/useConversationHistory';
import { TokenUsageInfo, BackgroundProcessInfo } from 'shared/types';

interface EntriesContextType {
  entries: PatchTypeWithKey[];
  setEntries: (entries: PatchTypeWithKey[]) => void;
  setTokenUsageInfo: (info: TokenUsageInfo | null) => void;
  setBackgroundProcessInfo: (info: BackgroundProcessInfo | null) => void;
  reset: () => void;
  tokenUsageInfo: TokenUsageInfo | null;
  backgroundProcessInfo: BackgroundProcessInfo | null;
}

const EntriesContext = createHmrContext<EntriesContextType | null>(
  'EntriesContext',
  null
);

interface EntriesProviderProps {
  children: ReactNode;
}

export const EntriesProvider = ({ children }: EntriesProviderProps) => {
  const [entries, setEntriesState] = useState<PatchTypeWithKey[]>([]);
  const [tokenUsageInfo, setTokenUsageInfo] = useState<TokenUsageInfo | null>(
    null
  );
  const [backgroundProcessInfo, setBackgroundProcessInfo] =
    useState<BackgroundProcessInfo | null>(null);

  const setEntries = useCallback((newEntries: PatchTypeWithKey[]) => {
    setEntriesState(newEntries);
  }, []);

  const setTokenUsageInfoCallback = useCallback(
    (info: TokenUsageInfo | null) => {
      setTokenUsageInfo(info);
    },
    []
  );

  const setBackgroundProcessInfoCallback = useCallback(
    (info: BackgroundProcessInfo | null) => {
      setBackgroundProcessInfo(info);
    },
    []
  );

  const reset = useCallback(() => {
    setEntriesState([]);
    setTokenUsageInfo(null);
    setBackgroundProcessInfo(null);
  }, []);

  const value = useMemo(
    () => ({
      entries,
      setEntries,
      setTokenUsageInfo: setTokenUsageInfoCallback,
      setBackgroundProcessInfo: setBackgroundProcessInfoCallback,
      reset,
      tokenUsageInfo,
      backgroundProcessInfo,
    }),
    [
      entries,
      setEntries,
      setTokenUsageInfoCallback,
      setBackgroundProcessInfoCallback,
      reset,
      tokenUsageInfo,
      backgroundProcessInfo,
    ]
  );

  return (
    <EntriesContext.Provider value={value}>{children}</EntriesContext.Provider>
  );
};

export const useEntries = (): EntriesContextType => {
  const context = useContext(EntriesContext);
  if (!context) {
    throw new Error('useEntries must be used within an EntriesProvider');
  }
  return context;
};

export const useTokenUsage = () => {
  const context = useContext(EntriesContext);
  if (!context) {
    throw new Error('useTokenUsage must be used within an EntriesProvider');
  }
  return context.tokenUsageInfo;
};

export const useBackgroundProcessInfo = () => {
  const context = useContext(EntriesContext);
  if (!context) {
    throw new Error(
      'useBackgroundProcessInfo must be used within an EntriesProvider'
    );
  }
  return context.backgroundProcessInfo;
};
