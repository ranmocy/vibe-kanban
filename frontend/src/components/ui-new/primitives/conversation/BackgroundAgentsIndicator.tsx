import { useState } from 'react';
import {
  CircleNotchIcon,
  CaretDownIcon,
  RobotIcon,
  CheckCircleIcon,
  XCircleIcon,
  TerminalIcon,
} from '@phosphor-icons/react';
import { cn } from '@/lib/utils';
import { useBackgroundProcessInfo } from '@/contexts/EntriesContext';
import type { BackgroundProcessItem } from 'shared/types';

function ProcessStatusIcon({ status }: { status: string }) {
  if (status === 'running') {
    return <CircleNotchIcon className="size-icon-xs text-info animate-spin" />;
  }
  if (status === 'completed') {
    return (
      <CheckCircleIcon className="size-icon-xs text-success" weight="fill" />
    );
  }
  return <XCircleIcon className="size-icon-xs text-error" weight="fill" />;
}

function ProcessTypeIcon({ processType }: { processType: string }) {
  if (processType === 'command') {
    return <TerminalIcon className="size-icon-xs text-low shrink-0" />;
  }
  return <RobotIcon className="size-icon-xs text-low shrink-0" />;
}

export function BackgroundAgentsIndicator() {
  const info = useBackgroundProcessInfo();
  const [expanded, setExpanded] = useState(false);

  if (!info || info.processes.length === 0) return null;

  const { processes, active_count } = info;
  const hasActive = active_count > 0;

  return (
    <div className="border-t border-border bg-secondary/50">
      <button
        type="button"
        className="flex items-center gap-base w-full px-double py-half text-left hover:bg-secondary/80"
        onClick={() => setExpanded(!expanded)}
      >
        {hasActive ? (
          <CircleNotchIcon className="size-icon-xs text-info animate-spin shrink-0" />
        ) : (
          <RobotIcon className="size-icon-xs text-low shrink-0" />
        )}
        <span className="text-sm text-normal flex-1">
          {hasActive
            ? `${active_count} background process${active_count > 1 ? 'es' : ''} running`
            : `${processes.length} background process${processes.length > 1 ? 'es' : ''} completed`}
        </span>
        <CaretDownIcon
          className={cn(
            'size-icon-xs text-low transition-transform',
            !expanded && '-rotate-90'
          )}
        />
      </button>

      {expanded && (
        <div className="px-double pb-half space-y-px">
          {processes.map((process: BackgroundProcessItem, i: number) => (
            <div key={i} className="flex items-center gap-base py-px">
              <ProcessStatusIcon status={process.status} />
              <ProcessTypeIcon processType={process.process_type} />
              <span className="text-sm text-normal truncate">
                {process.description}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
