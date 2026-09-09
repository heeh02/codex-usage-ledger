export type AppPage =
  | 'overview'
  | 'chats'
  | 'models'
  | 'project'
  | 'conversation'
  | 'unmatched'
  | 'session'
  | 'accounts'
  | 'quality';

export function isWorkDetailPage(page: AppPage): boolean {
  return page === 'models' || page === 'chats' || page === 'project' || page === 'conversation' || page === 'unmatched' || page === 'session';
}
