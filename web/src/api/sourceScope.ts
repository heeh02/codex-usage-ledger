import type { TokenUsage } from './types';
import { validRequestUsage } from './requestEvidence';
import { ledgerResponseError } from './errors';

export interface ScopeQuery { start: string; end: string; timezone: string; grain: 'day' | 'week' | 'month'; account?: string; project?: string; thread?: string; model?: string; includeDescendants?: boolean }
export interface ScopeRow { id: string | null; events: number; usage: TokenUsage }
export interface ScopeResponse { version: 2; status: string; query: ScopeQuery; historyComplete: false; productionPolicyChanged: false; data: { records: number } | null; display: { usage: TokenUsage; byTime: ScopeRow[]; byModel: ScopeRow[]; byAccount: ScopeRow[]; byProject: ScopeRow[]; byThread: ScopeRow[] } | null }
export interface ScopeCatalog { version: 1; projects: {id:string;label:string}[]; roots: {id:string;project:string|null;label:string|null}[]; accounts:string[]; rootLimit:number; search?:string }

export function validateScope(value: ScopeResponse, query: ScopeQuery): ScopeResponse {
  const fail = () => { throw new Error('Invalid scoped usage response'); };
  if (!value || value.version !== 2 || value.historyComplete !== false || value.productionPolicyChanged !== false
    || !value.query || Date.parse(value.query.start)!==Date.parse(query.start) || Date.parse(value.query.end)!==Date.parse(query.end)
    || value.query.timezone!==query.timezone || value.query.grain!==query.grain
    || (value.query.includeDescendants??false)!==(query.includeDescendants??false)) fail();
  for (const key of ['account','project','thread','model'] as const) if ((value.query[key]??null)!==(query[key]??null)) fail();
  if (value.status!=='available') {
    if (!['pending','unresolved','no_records','unconfirmed_only'].includes(value.status) || value.display!==null) fail();
    return value;
  }
  const display=value.display;
  if (!display || !validRequestUsage(display.usage) || !Number.isSafeInteger(value.data?.records) || value.data!.records<1) fail();
  const fields=['input','cached','cacheWrite','cacheWriteObservedInput','uncached','output','reasoning','total'] as const;
  for (const dimension of ['byTime','byModel','byAccount','byProject','byThread'] as const) {
    const rows=display![dimension];
    if (!Array.isArray(rows) || rows.some(r=>!r || (r.id!==null && typeof r.id!=='string') || !Number.isSafeInteger(r.events) || r.events<1 || !validRequestUsage(r.usage))) fail();
    if (new Set(rows.map(r=>r.id)).size!==rows.length || rows.reduce((n,r)=>n+r.events,0)!==value.data!.records) fail();
    for (const field of fields) if(rows.reduce((n,r)=>n+r.usage[field],0)!==display!.usage[field]) fail();
  }
  return value;
}

const base = () => (import.meta.env.VITE_LEDGER_API_BASE ?? '').replace(/\/$/,'');
export async function getSourceScope(query: ScopeQuery, signal: AbortSignal): Promise<ScopeResponse> {
  const params=new URLSearchParams();
  for (const [key,value] of Object.entries(query)) if(value!==undefined)params.set(key,String(value));
  const response=await fetch(`${base()}/v1/source-union?${params}`,{signal});
  if(!response.ok)throw await ledgerResponseError(response);
  return validateScope(await response.json(),query);
}
export async function getSourceCatalog(signal: AbortSignal, search=''): Promise<ScopeCatalog> {
  const expected=search.trim();
  const response=await fetch(`${base()}/v1/source-catalog?${new URLSearchParams({search:expected})}`,{signal});
  if(!response.ok)throw await ledgerResponseError(response);
  const value=await response.json() as ScopeCatalog;
  if(!value || value.version!==1 || !Array.isArray(value.projects) || !Array.isArray(value.roots) || !Array.isArray(value.accounts)
    || value.projects.some(p=>!p||typeof p.id!=='string'||typeof p.label!=='string')
    || value.roots.some(r=>!r||typeof r.id!=='string'||(r.label!==null&&typeof r.label!=='string')||(r.project!==null&&typeof r.project!=='string'))
    || value.accounts.some(id=>typeof id!=='string') || value.rootLimit!==500 || (value.search??'')!==expected)throw new Error('Invalid source catalog');
  return value;
}
