import { useMemo } from 'react';
import type { ToolStatus } from 'shared/types';
import type { PatchTypeWithKey } from '@/hooks/useConversationHistory';

export interface TeamMember {
  agentName: string | null;
  subagentType: string | null;
  status: ToolStatus;
  description: string;
}

export interface TeamState {
  name: string;
  members: TeamMember[];
  running: number;
  completed: number;
  failed: number;
  deleted: boolean;
}

/**
 * Derives team state from conversation entries.
 * Scans for team_create, task_create (with team_name), and team_delete events.
 */
export function useTeamStatus(
  entries: PatchTypeWithKey[]
): Map<string, TeamState> {
  return useMemo(() => {
    const teams = new Map<string, TeamState>();

    for (const entry of entries) {
      if (entry.type !== 'NORMALIZED_ENTRY') continue;
      const { content } = entry;
      if (content.entry_type.type !== 'tool_use') continue;

      const { action_type, status } = content.entry_type;

      if (action_type.action === 'team_create') {
        const name = action_type.name;
        if (!teams.has(name)) {
          teams.set(name, {
            name,
            members: [],
            running: 0,
            completed: 0,
            failed: 0,
            deleted: false,
          });
        }
      } else if (
        action_type.action === 'task_create' &&
        action_type.team_name
      ) {
        const teamName = action_type.team_name;
        if (!teams.has(teamName)) {
          teams.set(teamName, {
            name: teamName,
            members: [],
            running: 0,
            completed: 0,
            failed: 0,
            deleted: false,
          });
        }
        const team = teams.get(teamName)!;
        const member: TeamMember = {
          agentName: action_type.agent_name ?? null,
          subagentType: action_type.subagent_type ?? null,
          status,
          description: action_type.description,
        };
        team.members.push(member);

        const s = status.status;
        if (s === 'created' || s === 'pending_approval') {
          team.running++;
        } else if (s === 'success') {
          team.completed++;
        } else if (
          s === 'failed' ||
          s === 'denied' ||
          s === 'timed_out'
        ) {
          team.failed++;
        }
      } else if (action_type.action === 'team_delete') {
        const name = action_type.name;
        const team = teams.get(name);
        if (team) {
          team.deleted = true;
        }
      }
    }

    return teams;
  }, [entries]);
}
