import { UsersThreeIcon, CircleNotchIcon } from '@phosphor-icons/react';
import { cn } from '@/lib/utils';
import type { TeamState } from '@/hooks/useTeamStatus';

interface TeamStatusIndicatorProps {
  teams: Map<string, TeamState>;
  className?: string;
}

export function TeamStatusIndicator({
  teams,
  className,
}: TeamStatusIndicatorProps) {
  const activeTeams = Array.from(teams.values()).filter((t) => !t.deleted);

  if (activeTeams.length === 0) return null;

  return (
    <div className={cn('flex flex-col gap-half', className)}>
      {activeTeams.map((team) => {
        const total = team.members.length;
        const hasRunning = team.running > 0;

        return (
          <div
            key={team.name}
            className="flex items-center gap-base px-double py-half text-xs text-low border-b border-border"
          >
            <UsersThreeIcon className="size-icon-xs shrink-0" />
            <span className="font-medium">{team.name}</span>
            <span className="flex items-center gap-half">
              {hasRunning && (
                <>
                  <CircleNotchIcon className="size-icon-xs animate-spin text-brand" />
                  <span className="text-brand">{team.running} running</span>
                </>
              )}
              {team.completed > 0 && (
                <span className="text-success">
                  {team.completed} done
                </span>
              )}
              {team.failed > 0 && (
                <span className="text-error">{team.failed} failed</span>
              )}
              {!hasRunning && total > 0 && (
                <span>{total} total</span>
              )}
            </span>
          </div>
        );
      })}
    </div>
  );
}
