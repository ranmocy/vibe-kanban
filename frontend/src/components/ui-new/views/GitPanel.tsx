import { GitBranchIcon, ArrowsClockwiseIcon } from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import { cn } from '@/lib/utils';
import {
  RepoCard,
  type RepoAction,
} from '@/components/ui-new/primitives/RepoCard';
import { InputField } from '@/components/ui-new/primitives/InputField';
import { ErrorAlert } from '@/components/ui-new/primitives/ErrorAlert';

export interface RepoInfo {
  id: string;
  name: string;
  targetBranch: string;
  commitsAhead: number;
  commitsBehind: number;
  remoteCommitsAhead?: number;
  prNumber?: number;
  prUrl?: string;
  prStatus?: 'open' | 'merged' | 'closed' | 'unknown';
  showPushButton?: boolean;
  isPushPending?: boolean;
  isPushSuccess?: boolean;
  isPushError?: boolean;
  isTargetRemote?: boolean;
}

interface GitPanelProps {
  repos: RepoInfo[];
  workingBranchName: string;
  onWorkingBranchNameChange: (name: string) => void;
  onActionsClick?: (repoId: string, action: RepoAction) => void;
  onPushClick?: (repoId: string) => void;
  onMoreClick?: (repoId: string) => void;
  onAddRepo?: () => void;
  onRebaseAll?: () => void;
  isRebasingAll?: boolean;
  hasReposToRebase?: boolean;
  className?: string;
  error?: string | null;
}

export function GitPanel({
  repos,
  workingBranchName,
  onWorkingBranchNameChange,
  onActionsClick,
  onPushClick,
  onMoreClick,
  onRebaseAll,
  isRebasingAll,
  hasReposToRebase,
  className,
  error,
}: GitPanelProps) {
  const { t } = useTranslation(['tasks', 'common']);

  return (
    <div
      className={cn(
        'flex flex-col flex-1 w-full bg-secondary text-low overflow-y-auto',
        className
      )}
    >
      {error && <ErrorAlert message={error} />}
      <div className="gap-base px-base">
        {hasReposToRebase && (
          <div className="flex justify-end py-half">
            <button
              type="button"
              onClick={onRebaseAll}
              disabled={isRebasingAll}
              className="flex items-center gap-1 px-base py-half rounded text-sm text-low hover:text-normal bg-secondary hover:bg-primary border border-transparent hover:border-current transition-colors disabled:opacity-50"
            >
              <ArrowsClockwiseIcon
                className={`size-icon-sm ${isRebasingAll ? 'animate-spin' : ''}`}
              />
              <span>{isRebasingAll ? 'Rebasing…' : 'Rebase All'}</span>
            </button>
          </div>
        )}
        {repos.map((repo) => (
          <RepoCard
            key={repo.id}
            repoId={repo.id}
            name={repo.name}
            targetBranch={repo.targetBranch}
            commitsAhead={repo.commitsAhead}
            commitsBehind={repo.commitsBehind}
            prNumber={repo.prNumber}
            prUrl={repo.prUrl}
            prStatus={repo.prStatus}
            showPushButton={repo.showPushButton}
            isPushPending={repo.isPushPending}
            isPushSuccess={repo.isPushSuccess}
            isPushError={repo.isPushError}
            isTargetRemote={repo.isTargetRemote}
            onChangeTarget={() => onActionsClick?.(repo.id, 'change-target')}
            onRebase={() => onActionsClick?.(repo.id, 'rebase')}
            onActionsClick={(action) => onActionsClick?.(repo.id, action)}
            onPushClick={() => onPushClick?.(repo.id)}
            onMoreClick={() => onMoreClick?.(repo.id)}
          />
        ))}
        <div className="bg-primary flex flex-col gap-base w-full p-base rounded-sm my-base">
          <div className="flex gap-base items-center">
            <GitBranchIcon className="size-icon-md text-base" weight="fill" />
            <p className="font-medium truncate">
              {t('common:sections.workingBranch')}
            </p>
          </div>
          <InputField
            variant="editable"
            value={workingBranchName}
            onChange={onWorkingBranchNameChange}
            placeholder={t('gitPanel.advanced.placeholder')}
          />
        </div>
      </div>
    </div>
  );
}
