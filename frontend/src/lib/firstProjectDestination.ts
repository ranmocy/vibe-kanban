import type { Project } from 'shared/remote-types';

const FIRST_PROJECT_LOOKUP_TIMEOUT_MS = 3000;

interface OrganizationLike {
  id: string;
  is_personal?: boolean;
}

function getFirstOrganization(
  organizations: OrganizationLike[]
): OrganizationLike | null {
  if (organizations.length === 0) return null;
  const firstNonPersonal = organizations.find((org) => !org.is_personal);
  return firstNonPersonal ?? organizations[0];
}

function getFirstProject(projects: Project[]): Project | null {
  if (projects.length === 0) return null;

  const sorted = [...projects].sort((a, b) => {
    const aTime = new Date(a.created_at).getTime();
    const bTime = new Date(b.created_at).getTime();
    if (aTime !== bTime) return aTime - bTime;
    const nameCompare = a.name.localeCompare(b.name);
    if (nameCompare !== 0) return nameCompare;
    return a.id.localeCompare(b.id);
  });

  return sorted[0];
}

export async function getFirstProjectDestination(
  setSelectedOrgId: (orgId: string | null) => void
): Promise<string | null> {
  try {
    const controller = new AbortController();
    const timeout = setTimeout(
      () => controller.abort(),
      FIRST_PROJECT_LOOKUP_TIMEOUT_MS
    );

    // Fetch organizations from local backend
    const orgsRes = await fetch('/api/kanban/organizations', {
      signal: controller.signal,
    });
    if (!orgsRes.ok) {
      clearTimeout(timeout);
      return null;
    }
    const organizations: OrganizationLike[] = await orgsRes.json();
    const firstOrg = getFirstOrganization(organizations);
    if (!firstOrg) {
      clearTimeout(timeout);
      return null;
    }

    setSelectedOrgId(firstOrg.id);

    // Fetch projects from local backend
    const projRes = await fetch(
      `/api/kanban/organizations/${firstOrg.id}/projects`,
      { signal: controller.signal }
    );
    clearTimeout(timeout);
    if (!projRes.ok) return null;
    const projects: Project[] = await projRes.json();

    const firstProject = getFirstProject(projects);
    if (!firstProject) return null;

    return `/projects/${firstProject.id}`;
  } catch (error) {
    if ((error as Error).name !== 'AbortError') {
      console.error('Failed to resolve first project destination:', error);
    }
    return null;
  }
}
