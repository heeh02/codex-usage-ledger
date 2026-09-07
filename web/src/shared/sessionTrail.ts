import type { DashboardFilters } from '../api/types';

export interface SessionTrailEntry<View> {
  id: string;
  title: string;
  view: View;
  nodes: Pick<DashboardFilters, 'nodeOffset' | 'nodeLimit' | 'nodeSearch'>;
  scrollTop: number;
}

export function parentSessionFilters<View>(current: DashboardFilters, entry: SessionTrailEntry<View>): DashboardFilters {
  return { ...current, session: entry.id, ...entry.nodes };
}
