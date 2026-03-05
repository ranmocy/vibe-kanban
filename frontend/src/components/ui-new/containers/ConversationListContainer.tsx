import {
  DataWithScrollModifier,
  ScrollModifier,
  VirtuosoMessageList,
  VirtuosoMessageListLicense,
  VirtuosoMessageListMethods,
  VirtuosoMessageListProps,
} from '@virtuoso.dev/message-list';
import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from 'react';

import { cn } from '@/lib/utils';
import NewDisplayConversationEntry from './NewDisplayConversationEntry';
import { ApprovalFormProvider } from '@/contexts/ApprovalFormContext';
import { useEntries } from '@/contexts/EntriesContext';
import {
  useResetProcess,
  type UseResetProcessResult,
} from '@/components/ui-new/hooks/useResetProcess';
import {
  AddEntryType,
  PatchTypeWithKey,
  DisplayEntry,
  isAggregatedGroup,
  isAggregatedDiffGroup,
  isAggregatedThinkingGroup,
  useConversationHistory,
} from '@/components/ui-new/hooks/useConversationHistory';
import { aggregateConsecutiveEntries } from '@/utils/aggregateEntries';
import type { WorkspaceWithSession } from '@/types/attempt';
import type { RepoWithTargetBranch } from 'shared/types';
import { useWorkspaceContext } from '@/contexts/WorkspaceContext';
import { ChatScriptPlaceholder } from '../primitives/conversation/ChatScriptPlaceholder';
import { ScriptFixerDialog } from '@/components/dialogs/scripts/ScriptFixerDialog';

interface ConversationListProps {
  attempt: WorkspaceWithSession;
}

export interface ConversationListHandle {
  scrollToPreviousUserMessage: () => void;
  scrollToBottom: () => void;
}

interface MessageListContext {
  attempt: WorkspaceWithSession;
  onConfigureSetup: (() => void) | undefined;
  onConfigureCleanup: (() => void) | undefined;
  showSetupPlaceholder: boolean;
  showCleanupPlaceholder: boolean;
  resetAction: UseResetProcessResult;
  isLoadingMore: boolean;
  hasMoreHistory: boolean;
}

const INITIAL_TOP_ITEM = { index: 'LAST' as const, align: 'end' as const };

const InitialDataScrollModifier: ScrollModifier = {
  type: 'item-location',
  location: INITIAL_TOP_ITEM,
  purgeItemSizes: true,
};

const AutoScrollToBottom: ScrollModifier = {
  type: 'auto-scroll-to-bottom',
  autoScroll: 'smooth',
};

const ScrollToTopOfLastItem: ScrollModifier = {
  type: 'item-location',
  location: {
    index: 'LAST',
    align: 'start',
  },
};

const ItemContent: VirtuosoMessageListProps<
  DisplayEntry,
  MessageListContext
>['ItemContent'] = ({ data, context }) => {
  const attempt = context?.attempt;
  const resetAction = context?.resetAction;

  // Handle aggregated tool groups (file_read, search, web_fetch)
  if (isAggregatedGroup(data)) {
    return (
      <NewDisplayConversationEntry
        expansionKey={data.patchKey}
        aggregatedGroup={data}
        aggregatedDiffGroup={null}
        aggregatedThinkingGroup={null}
        entry={null}
        executionProcessId={data.executionProcessId}
        taskAttempt={attempt}
        resetAction={resetAction}
      />
    );
  }

  // Handle aggregated diff groups (file_edit by same path)
  if (isAggregatedDiffGroup(data)) {
    return (
      <NewDisplayConversationEntry
        expansionKey={data.patchKey}
        aggregatedGroup={null}
        aggregatedDiffGroup={data}
        aggregatedThinkingGroup={null}
        entry={null}
        executionProcessId={data.executionProcessId}
        taskAttempt={attempt}
        resetAction={resetAction}
      />
    );
  }

  // Handle aggregated thinking groups (thinking entries in previous turns)
  if (isAggregatedThinkingGroup(data)) {
    return (
      <NewDisplayConversationEntry
        expansionKey={data.patchKey}
        aggregatedGroup={null}
        aggregatedDiffGroup={null}
        aggregatedThinkingGroup={data}
        entry={null}
        executionProcessId={data.executionProcessId}
        taskAttempt={attempt}
        resetAction={resetAction}
      />
    );
  }

  if (data.type === 'STDOUT') {
    return <p>{data.content}</p>;
  }
  if (data.type === 'STDERR') {
    return <p>{data.content}</p>;
  }
  if (data.type === 'NORMALIZED_ENTRY' && attempt) {
    return (
      <NewDisplayConversationEntry
        expansionKey={data.patchKey}
        entry={data.content}
        aggregatedGroup={null}
        aggregatedDiffGroup={null}
        aggregatedThinkingGroup={null}
        executionProcessId={data.executionProcessId}
        taskAttempt={attempt}
        resetAction={resetAction}
      />
    );
  }

  return null;
};

const computeItemKey: VirtuosoMessageListProps<
  DisplayEntry,
  MessageListContext
>['computeItemKey'] = ({ data }) => `conv-${data.patchKey}`;

const ConversationSkeleton = () => (
  <div className="flex flex-col gap-base pt-2 animate-pulse">
    <div className="py-base px-double">
      <div className="flex items-center gap-base mb-base">
        <div className="size-4 bg-secondary rounded" />
        <div className="h-3 bg-secondary rounded w-24" />
      </div>
      <div className="flex flex-col gap-1.5 pl-6">
        <div className="h-3 bg-secondary rounded w-3/4" />
        <div className="h-3 bg-secondary rounded w-1/2" />
      </div>
    </div>
    <div className="py-base px-double">
      <div className="flex flex-col gap-1.5">
        <div className="h-3 bg-secondary rounded w-full" />
        <div className="h-3 bg-secondary rounded w-5/6" />
        <div className="h-3 bg-secondary rounded w-4/6" />
      </div>
    </div>
    <div className="py-base px-double">
      <div className="flex items-center gap-base">
        <div className="size-3 bg-secondary rounded" />
        <div className="h-3 bg-secondary rounded w-48" />
      </div>
    </div>
  </div>
);

export const ConversationList = forwardRef<
  ConversationListHandle,
  ConversationListProps
>(function ConversationList({ attempt }, ref) {
  const resetAction = useResetProcess();
  const [channelData, setChannelData] =
    useState<DataWithScrollModifier<DisplayEntry> | null>(null);
  const [loading, setLoading] = useState(true);
  const { setEntries, reset } = useEntries();
  const pendingUpdateRef = useRef<{
    entries: PatchTypeWithKey[];
    addType: AddEntryType;
    loading: boolean;
  } | null>(null);
  const debounceTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingAddTypePriorityRef = useRef<number>(-1);

  const SCROLL_THRESHOLD_PX = 200;
  const LOAD_MORE_DEBOUNCE_MS = 150;
  const loadMoreDebounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Get repos from workspace context to check if scripts are configured
  let repos: RepoWithTargetBranch[] = [];
  try {
    const workspaceContext = useWorkspaceContext();
    repos = workspaceContext.repos;
  } catch {
    // Context not available
  }

  // Use ref to access current repos without causing callback recreation
  const reposRef = useRef(repos);
  reposRef.current = repos;

  // Check if any repo has setup or cleanup scripts configured
  const hasSetupScript = repos.some((repo) => repo.setup_script);
  const hasCleanupScript = repos.some((repo) => repo.cleanup_script);

  // Handlers to open script fixer dialog for setup/cleanup scripts
  const handleConfigureSetup = useCallback(() => {
    const currentRepos = reposRef.current;
    if (currentRepos.length === 0) return;

    ScriptFixerDialog.show({
      scriptType: 'setup',
      repos: currentRepos,
      workspaceId: attempt.id,
      sessionId: attempt.session?.id,
    });
  }, [attempt.id, attempt.session?.id]);

  const handleConfigureCleanup = useCallback(() => {
    const currentRepos = reposRef.current;
    if (currentRepos.length === 0) return;

    ScriptFixerDialog.show({
      scriptType: 'cleanup',
      repos: currentRepos,
      workspaceId: attempt.id,
      sessionId: attempt.session?.id,
    });
  }, [attempt.id, attempt.session?.id]);

  // Determine if configure buttons should be shown
  const canConfigure = repos.length > 0;

  useEffect(() => {
    setLoading(true);
    setChannelData(null);
    reset();
  }, [attempt.id, reset]);

  useEffect(() => {
    return () => {
      if (debounceTimeoutRef.current) {
        clearTimeout(debounceTimeoutRef.current);
      }
      if (loadMoreDebounceRef.current) {
        clearTimeout(loadMoreDebounceRef.current);
      }
    };
  }, []);

  const ADD_TYPE_PRIORITY: Record<AddEntryType, number> = {
    prepend: 4,
    plan: 3,
    initial: 2,
    running: 1,
    historic: 0,
  };

  const onEntriesUpdated = (
    newEntries: PatchTypeWithKey[],
    addType: AddEntryType,
    newLoading: boolean
  ) => {
    // Keep the highest-priority addType within the debounce window
    const priority = ADD_TYPE_PRIORITY[addType] ?? 0;
    if (priority > pendingAddTypePriorityRef.current) {
      pendingAddTypePriorityRef.current = priority;
    }

    const effectiveAddType =
      Object.entries(ADD_TYPE_PRIORITY).find(
        ([, v]) => v === pendingAddTypePriorityRef.current
      )?.[0] as AddEntryType ?? addType;

    pendingUpdateRef.current = {
      entries: newEntries,
      addType: effectiveAddType,
      loading: newLoading,
    };

    if (debounceTimeoutRef.current) {
      clearTimeout(debounceTimeoutRef.current);
    }

    debounceTimeoutRef.current = setTimeout(() => {
      const pending = pendingUpdateRef.current;
      if (!pending) return;

      let scrollModifier: ScrollModifier | undefined =
        InitialDataScrollModifier;

      if (pending.addType === 'prepend') {
        scrollModifier = undefined; // no scroll adjustment — Virtuoso preserves position via key identity
      } else if (pending.addType === 'plan' && !loading) {
        scrollModifier = ScrollToTopOfLastItem;
      } else if (pending.addType === 'running' && !loading) {
        scrollModifier = AutoScrollToBottom;
      }

      const aggregatedEntries = aggregateConsecutiveEntries(pending.entries);

      setChannelData({ data: aggregatedEntries, scrollModifier });
      setEntries(pending.entries);

      if (loading) {
        setLoading(pending.loading);
      }

      // Reset priority tracking for next debounce window
      pendingAddTypePriorityRef.current = -1;
    }, 100);
  };

  const {
    hasSetupScriptRun,
    hasCleanupScriptRun,
    hasRunningProcess,
    isFirstTurn,
    hasMoreHistory,
    isLoadingMore,
    loadOlderEntries,
  } = useConversationHistory({ attempt, onEntriesUpdated });

  // Determine if there are entries to show placeholders
  const entries = channelData?.data ?? [];
  const hasEntries = entries.length > 0;

  // Show placeholders only if script not configured AND not already run AND first turn
  const showSetupPlaceholder =
    !hasSetupScript && !hasSetupScriptRun && hasEntries;
  const showCleanupPlaceholder =
    !hasCleanupScript &&
    !hasCleanupScriptRun &&
    !hasRunningProcess &&
    hasEntries &&
    isFirstTurn;

  const messageListRef = useRef<VirtuosoMessageListMethods | null>(null);
  const messageListContext = useMemo(
    () => ({
      attempt,
      onConfigureSetup: canConfigure ? handleConfigureSetup : undefined,
      onConfigureCleanup: canConfigure ? handleConfigureCleanup : undefined,
      showSetupPlaceholder,
      showCleanupPlaceholder,
      resetAction,
      isLoadingMore,
      hasMoreHistory,
    }),
    [
      attempt,
      canConfigure,
      handleConfigureSetup,
      handleConfigureCleanup,
      showSetupPlaceholder,
      showCleanupPlaceholder,
      resetAction,
      isLoadingMore,
      hasMoreHistory,
    ]
  );

  // Expose scroll to previous user message functionality via ref
  useImperativeHandle(
    ref,
    () => ({
      scrollToPreviousUserMessage: () => {
        const data = channelData?.data;
        if (!data || !messageListRef.current) return;

        // Get currently rendered items to find visible range
        const rendered = messageListRef.current.data.getCurrentlyRendered();
        if (!rendered.length) return;

        // Find the index of the first visible item in the full data array
        const firstVisibleKey = rendered[0]?.patchKey;
        const firstVisibleIndex = data.findIndex(
          (item) => item.patchKey === firstVisibleKey
        );

        // Find all user message indices
        const userMessageIndices: number[] = [];
        data.forEach((item, index) => {
          if (
            item.type === 'NORMALIZED_ENTRY' &&
            item.content.entry_type.type === 'user_message'
          ) {
            userMessageIndices.push(index);
          }
        });

        // Find the user message before the first visible item
        const targetIndex = userMessageIndices
          .reverse()
          .find((idx) => idx < firstVisibleIndex);

        if (targetIndex !== undefined) {
          messageListRef.current.scrollToItem({
            index: targetIndex,
            align: 'start',
            behavior: 'smooth',
          });
        }
      },
      scrollToBottom: () => {
        if (!messageListRef.current) return;
        messageListRef.current.scrollToItem({
          index: 'LAST',
          align: 'end',
          behavior: 'smooth',
        });
      },
    }),
    [channelData]
  );

  // Scroll-triggered loading of older history
  const handleScroll = useCallback(
    (e: React.UIEvent<HTMLDivElement>) => {
      const { scrollTop } = e.currentTarget;
      if (
        scrollTop < SCROLL_THRESHOLD_PX &&
        hasMoreHistory &&
        !isLoadingMore
      ) {
        if (loadMoreDebounceRef.current)
          clearTimeout(loadMoreDebounceRef.current);
        loadMoreDebounceRef.current = setTimeout(() => {
          loadOlderEntries();
        }, LOAD_MORE_DEBOUNCE_MS);
      }
    },
    [hasMoreHistory, isLoadingMore, loadOlderEntries]
  );

  // Determine if content is ready to show (has data or finished loading)
  const hasContent = !loading;

  return (
    <ApprovalFormProvider>
      <div className="h-full relative">
        <div
          className={cn(
            'absolute inset-0 transition-opacity duration-300 z-10',
            hasContent ? 'opacity-0 pointer-events-none' : 'opacity-100'
          )}
        >
          <ConversationSkeleton />
        </div>
        <div
          className={cn(
            'h-full transition-opacity duration-300',
            hasContent ? 'opacity-100' : 'opacity-0'
          )}
          onScroll={handleScroll}
        >
        <VirtuosoMessageListLicense
          licenseKey={import.meta.env.VITE_PUBLIC_REACT_VIRTUOSO_LICENSE_KEY}
        >
          <VirtuosoMessageList<DisplayEntry, MessageListContext>
            ref={messageListRef}
            className="h-full scrollbar-none"
            data={channelData}
            initialLocation={INITIAL_TOP_ITEM}
            context={messageListContext}
            computeItemKey={computeItemKey}
            ItemContent={ItemContent}
            Header={({ context }) => (
              <div className="pt-2">
                {context?.isLoadingMore && (
                  <div className="flex justify-center items-center py-2 text-low text-sm">
                    <span>Loading older messages...</span>
                  </div>
                )}
                {context?.showSetupPlaceholder && (
                  <div className="my-base px-double">
                    <ChatScriptPlaceholder
                      type="setup"
                      onConfigure={context.onConfigureSetup}
                    />
                  </div>
                )}
              </div>
            )}
            Footer={({ context }) => (
              <div className="pb-2">
                {context?.showCleanupPlaceholder && (
                  <div className="my-base px-double">
                    <ChatScriptPlaceholder
                      type="cleanup"
                      onConfigure={context.onConfigureCleanup}
                    />
                  </div>
                )}
              </div>
            )}
          />
        </VirtuosoMessageListLicense>
        </div>
      </div>
    </ApprovalFormProvider>
  );
});

export default ConversationList;
