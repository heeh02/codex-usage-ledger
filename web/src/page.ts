export type AppPage =
  | 'overview'
  | 'chats'
  | 'project'
  | 'conversation'
  | 'unmatched'
  | 'session'
  | 'accounts'
  | 'quality';

export function isWorkDetailPage(page: AppPage): boolean {
  return page === 'chats' || page === 'project' || page === 'conversation' || page === 'unmatched' || page === 'session';
}
