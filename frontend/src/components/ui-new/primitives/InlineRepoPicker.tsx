import { useMemo } from 'react';
import {
  GitBranchIcon,
  MagnifyingGlassIcon,
  PlusIcon,
  SpinnerIcon,
} from '@phosphor-icons/react';
import { useTranslation } from 'react-i18next';
import type { Repo } from 'shared/types';
import { cn } from '@/lib/utils';
import { Checkbox } from '@/components/ui/checkbox';

function getRepoDisplayName(repo: Repo): string {
  return repo.display_name || repo.name;
}

interface InlineRepoPickerProps {
  recentRepos: Repo[];
  selectedRepos: Repo[];
  selectedRepoIds: Set<string>;
  targetBranches: Record<string, string | null>;
  isLoading: boolean;
  onToggleRepo: (repo: Repo, selected: boolean) => void;
  onChangeBranch: (repo: Repo) => void;
  changingBranchRepoId: string | null;
  onBrowse: () => void;
  onCreate: () => void;
  isBrowsing: boolean;
  isCreating: boolean;
  disabled?: boolean;
}

export function InlineRepoPicker({
  recentRepos,
  selectedRepos,
  selectedRepoIds,
  targetBranches,
  isLoading,
  onToggleRepo,
  onChangeBranch,
  changingBranchRepoId,
  onBrowse,
  onCreate,
  isBrowsing,
  isCreating,
  disabled = false,
}: InlineRepoPickerProps) {
  const { t } = useTranslation('common');
  const isBusy = isBrowsing || isCreating || changingBranchRepoId !== null;

  // Combine recent repos with any selected repos not in the recent list
  const displayRepos = useMemo(() => {
    const recentIds = new Set(recentRepos.map((r) => r.id));
    const extraSelected = selectedRepos.filter((r) => !recentIds.has(r.id));
    return [...recentRepos, ...extraSelected];
  }, [recentRepos, selectedRepos]);

  if (isLoading) {
    return (
      <div className="border-t border-border/60 px-base py-1">
        <div className="flex items-center gap-half text-sm text-low">
          <SpinnerIcon className="size-icon-xs animate-spin" />
          <span>Loading repositories…</span>
        </div>
      </div>
    );
  }

  return (
    <div className="border-t border-border/60">
      {displayRepos.length > 0 && (
        <div className="max-h-[50vh] min-h-[100px] overflow-y-auto py-2">
          {displayRepos.map((repo) => {
            const isSelected = selectedRepoIds.has(repo.id);
            const branch = targetBranches[repo.id];
            const isChangingBranch = changingBranchRepoId === repo.id;

            return (
              <div
                key={repo.id}
                className="flex min-w-0 items-center gap-half px-base py-1"
              >
                <Checkbox
                  checked={isSelected}
                  onCheckedChange={(checked) =>
                    onToggleRepo(repo, checked === true)
                  }
                  disabled={disabled || isBusy}
                  className="h-4 w-4 shrink-0"
                />
                <span
                  className={cn(
                    'min-w-0 flex-1 truncate text-sm',
                    isSelected ? 'text-normal' : 'text-low'
                  )}
                >
                  {getRepoDisplayName(repo)}
                </span>
                <span className="h-3 w-px shrink-0 bg-border/70" />
                {isSelected ? (
                  <button
                    type="button"
                    onClick={() => onChangeBranch(repo)}
                    disabled={disabled || isBusy}
                    className="inline-flex items-center gap-half text-sm text-low hover:text-high disabled:cursor-not-allowed disabled:opacity-50"
                    title="Change branch"
                  >
                    {isChangingBranch ? (
                      <SpinnerIcon className="size-icon-xs animate-spin" />
                    ) : (
                      <GitBranchIcon
                        className="size-icon-xs"
                        weight="bold"
                      />
                    )}
                    <span className="max-w-[200px] truncate">
                      {branch ?? 'Select branch'}
                    </span>
                  </button>
                ) : (
                  <span className="inline-flex items-center gap-half text-sm text-low">
                    <GitBranchIcon className="size-icon-xs" weight="bold" />
                    <span className="max-w-[200px] truncate">
                      {repo.default_target_branch ?? 'main'}
                    </span>
                  </span>
                )}
              </div>
            );
          })}
        </div>
      )}

      <div
        className={cn(
          'flex items-center gap-half px-base py-1',
          displayRepos.length > 0 && 'border-t border-border/60'
        )}
      >
        <button
          type="button"
          onClick={onBrowse}
          disabled={disabled || isBusy}
          className="inline-flex items-center gap-half rounded-sm px-half py-half text-sm text-normal hover:text-high disabled:cursor-not-allowed disabled:opacity-50"
        >
          {isBrowsing ? (
            <SpinnerIcon className="size-icon-xs animate-spin" />
          ) : (
            <MagnifyingGlassIcon className="size-icon-xs" weight="bold" />
          )}
          <span>{t('createMode.repoPicker.actions.browse')}</span>
        </button>
        <button
          type="button"
          onClick={onCreate}
          disabled={disabled || isBusy}
          className="inline-flex items-center gap-half rounded-sm px-half py-half text-sm text-normal hover:text-high disabled:cursor-not-allowed disabled:opacity-50"
        >
          {isCreating ? (
            <SpinnerIcon className="size-icon-xs animate-spin" />
          ) : (
            <PlusIcon className="size-icon-xs" weight="bold" />
          )}
          <span>{t('createMode.repoPicker.actions.create')}</span>
        </button>
      </div>
    </div>
  );
}
