import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CaretDownIcon,
  PaperPlaneRightIcon,
  CheckCircleIcon,
  XCircleIcon,
  CircleNotchIcon,
} from '@phosphor-icons/react';
import { cn } from '@/lib/utils';
import { ToolStatus, ToolResult } from 'shared/types';
import { ChatMarkdown } from './ChatMarkdown';

interface ChatAgentMessageProps {
  recipient: string;
  message: string;
  teamName?: string | null;
  result?: ToolResult | null;
  status?: ToolStatus;
  expanded?: boolean;
  onToggle?: () => void;
  className?: string;
  workspaceId?: string;
}

export function ChatAgentMessage({
  recipient,
  message,
  teamName,
  result,
  status,
  expanded = false,
  onToggle,
  className,
  workspaceId,
}: ChatAgentMessageProps) {
  const { t } = useTranslation('common');

  const StatusIcon = useMemo(() => {
    if (!status) return null;
    const statusType = status.status;
    const isSuccess = statusType === 'success';
    const isError =
      statusType === 'failed' ||
      statusType === 'denied' ||
      statusType === 'timed_out';
    const isPending =
      statusType === 'created' || statusType === 'pending_approval';

    if (isSuccess) {
      return (
        <CheckCircleIcon className="size-icon-xs text-success" weight="fill" />
      );
    }
    if (isError) {
      return <XCircleIcon className="size-icon-xs text-error" weight="fill" />;
    }
    if (isPending) {
      return <CircleNotchIcon className="size-icon-xs text-low animate-spin" />;
    }
    return null;
  }, [status]);

  const isErrorStatus = useMemo(() => {
    if (!status) return false;
    return (
      status.status === 'failed' ||
      status.status === 'denied' ||
      status.status === 'timed_out'
    );
  }, [status]);

  const resultContent = useMemo(() => {
    if (!result?.value) return null;
    if (typeof result.value === 'string') return result.value;
    return JSON.stringify(result.value, null, 2);
  }, [result]);

  const hasContent = Boolean(resultContent);

  return (
    <div
      className={cn(
        'rounded-sm border overflow-hidden',
        isErrorStatus && 'border-error bg-error/5',
        status?.status === 'success' && 'border-success/50',
        !isErrorStatus && status?.status !== 'success' && 'border-border',
        className
      )}
    >
      <div
        className={cn(
          'flex items-center px-double py-base gap-base',
          isErrorStatus && 'bg-error/10',
          status?.status === 'success' && 'bg-success/5',
          onToggle && hasContent && 'cursor-pointer'
        )}
        onClick={hasContent ? onToggle : undefined}
      >
        <span className="relative shrink-0">
          <PaperPlaneRightIcon className="size-icon-base text-low" />
        </span>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-base flex-wrap">
            <span className="text-xs font-medium text-low uppercase tracking-wide">
              {t('conversation.agentMessage.sentTo', {
                recipient,
              })}
            </span>
            {StatusIcon}
            {teamName && (
              <span className="text-xs text-low bg-secondary px-base rounded">
                Team: {teamName}
              </span>
            )}
          </div>
          <span className="text-sm text-normal truncate block">{message}</span>
        </div>
        {onToggle && hasContent && (
          <CaretDownIcon
            className={cn(
              'size-icon-xs shrink-0 text-low transition-transform',
              !expanded && '-rotate-90'
            )}
          />
        )}
      </div>

      {expanded && hasContent && (
        <div className="border-t p-double bg-panel/50">
          <div className="prose prose-sm dark:prose-invert max-w-none">
            <ChatMarkdown content={resultContent!} workspaceId={workspaceId} />
          </div>
        </div>
      )}
    </div>
  );
}
