import type { DashboardBundle, DashboardFilters } from '../../api/types';
import { ConversationControls } from '../../components/ConversationControls';
import { OverviewSessions } from '../../components/Explorer';

export function ConversationsPage({ bundle, filters, onChange, onOpenSession }: {
  bundle: DashboardBundle;
  filters: DashboardFilters;
  onChange: (filters: DashboardFilters) => void;
  onOpenSession: (session: string) => void;
}) {
  return <section className="conversations-page">
    <ConversationControls explorer={bundle.explorer} filters={filters} onChange={onChange} />
    <OverviewSessions explorer={bundle.explorer} onOpenSession={onOpenSession} />
  </section>;
}
