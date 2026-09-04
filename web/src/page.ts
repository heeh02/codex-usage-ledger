export type AppPage =
  | 'overview'
  | 'project'
  | 'conversation'
  | 'unmatched'
  | 'session'
  | 'accounts'
  | 'quality';

export function isWorkDetailPage(page: AppPage): boolean {
  return page === 'project' || page === 'conversation' || page === 'unmatched' || page === 'session';
}
