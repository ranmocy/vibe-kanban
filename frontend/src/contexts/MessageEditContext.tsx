import React, { useCallback, useContext, useMemo, useState } from 'react';
import { createHmrContext } from '@/lib/hmrContext.ts';
import { useEntries } from './EntriesContext';

interface EditState {
  entryKey: string;
  processId: string;
  originalMessage: string;
}

interface MessageEditContextType {
  activeEdit: EditState | null;
  startEdit: (
    entryKey: string,
    processId: string,
    originalMessage: string
  ) => void;
  cancelEdit: () => void;
  isEntryGreyed: (entryKey: string) => boolean;
  isInEditMode: boolean;
}

const MessageEditContext = createHmrContext<MessageEditContextType | null>(
  'MessageEditContext',
  null
);

const ALWAYS_FALSE = () => false;

export function MessageEditProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const [activeEdit, setActiveEdit] = useState<EditState | null>(null);
  const { entries } = useEntries();

  // Only compute entryOrder when in edit mode to avoid work on every entries update
  const entryOrder = useMemo(() => {
    if (!activeEdit) return null;
    const order: Record<string, number> = {};
    entries.forEach((entry, idx) => {
      order[entry.patchKey] = idx;
    });
    return order;
  }, [activeEdit, entries]);

  const startEdit = useCallback(
    (entryKey: string, processId: string, originalMessage: string) => {
      setActiveEdit({ entryKey, processId, originalMessage });
    },
    []
  );

  const cancelEdit = useCallback(() => {
    setActiveEdit(null);
  }, []);

  // When no edit is active (the common case), return a stable function reference
  // to prevent context value changes from cascading re-renders to all consumers
  const isEntryGreyed = useMemo(() => {
    if (!activeEdit || !entryOrder) return ALWAYS_FALSE;
    return (entryKey: string) => {
      const activeOrder = entryOrder[activeEdit.entryKey];
      const thisOrder = entryOrder[entryKey];
      // Grey out entries that come AFTER the edit target
      return thisOrder > activeOrder;
    };
  }, [activeEdit, entryOrder]);

  const isInEditMode = activeEdit !== null;

  const value = useMemo(
    () => ({
      activeEdit,
      startEdit,
      cancelEdit,
      isEntryGreyed,
      isInEditMode,
    }),
    [activeEdit, startEdit, cancelEdit, isEntryGreyed, isInEditMode]
  );

  return (
    <MessageEditContext.Provider value={value}>
      {children}
    </MessageEditContext.Provider>
  );
}

export function useMessageEditContext() {
  const ctx = useContext(MessageEditContext);
  if (!ctx) {
    return {
      activeEdit: null,
      startEdit: () => {},
      cancelEdit: () => {},
      isEntryGreyed: () => false,
      isInEditMode: false,
    } as MessageEditContextType;
  }
  return ctx;
}
