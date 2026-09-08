import { useEffect, useRef, useState } from 'react';
import { getSourceCatalog, getSourceScope, type ScopeCatalog, type ScopeQuery, type ScopeResponse } from '../../api/sourceScope';
import { useI18n } from '../../i18n';
import { Panel } from '../../components/Ui';
import { UsageBreakdownTable } from '../../components/UsageBreakdownTable';
import { UsageTrendChart } from '../../components/UsageTrendChart';
import { runScopedRequest } from '../../shared/requestLifecycle';
import './scoped-evidence.css';

function dateInput(date: Date) { return `${date.getFullYear()}-${String(date.getMonth()+1).padStart(2,'0')}-${String(date.getDate()).padStart(2,'0')}`; }
export function ScopedEvidenceView() {
  const {t}=useI18n();
  const [catalog,setCatalog]=useState<ScopeCatalog|null>(null);
  const [catalogRevision,setCatalogRevision]=useState(0);
  const [project,setProject]=useState(''); const [thread,setThread]=useState(''); const [account,setAccount]=useState('');
  const [includeDescendants,setIncludeDescendants]=useState(false);
  const [start,setStart]=useState(()=>dateInput(new Date(new Date().getFullYear(),new Date().getMonth(),1)));
  const [end,setEnd]=useState(()=>{const d=new Date();d.setDate(d.getDate()+1);return dateInput(d);});
  const [grain,setGrain]=useState<ScopeQuery['grain']>('day');
  const [pending,setPending]=useState(false); const [failed,setFailed]=useState(false);
  const [result,setResult]=useState<{value:ScopeResponse;caption:{project:string|null;chat:string|null;account:string;dates:string}}|null>(null);
  const active=useRef<AbortController|null>(null);
  const timezone=Intl.DateTimeFormat().resolvedOptions().timeZone;
  useEffect(()=>{const controller=new AbortController();runScopedRequest(controller.signal,()=>getSourceCatalog(controller.signal),{success:value=>{setCatalog(value);setFailed(false);},failure:()=>setFailed(true),settled:()=>{}});return()=>{controller.abort();active.current?.abort();};},[catalogRevision]);
  const submit=async(startDate:string,endDate:string)=>{
    setStart(startDate);setEnd(endDate);
    active.current?.abort();const controller=new AbortController();active.current=controller;setPending(true);setFailed(false);
    try {
      const query:ScopeQuery={start:new Date(`${startDate}T00:00:00`).toISOString(),end:new Date(`${endDate}T00:00:00`).toISOString(),timezone,grain,...(account?{account}:{}),...(project?{project}:{}),...(thread?{thread,includeDescendants}:{})};
      if(Date.parse(query.start)>=Date.parse(query.end))throw new Error('Invalid interval');
      const caption={project:catalog?.projects.find(p=>p.id===project)?.label??null,chat:thread?(catalog?.roots.find(r=>r.id===thread)?.label??thread.slice(0,8)):null,account,dates:`${startDate} → ${endDate} · ${timezone}`};
      await runScopedRequest(controller.signal,()=>getSourceScope(query,controller.signal),{success:value=>setResult({value,caption}),failure:()=>setFailed(true),settled:()=>setPending(false)});
    } catch { if(!controller.signal.aborted){setFailed(true);setPending(false);} }
  };
  const display=result?.value.display;
  const caption=result?[result.caption.project??t('scope.all_projects'),result.caption.chat??t('scope.all_chats'),...(result.value.query.thread?[t(result.value.query.includeDescendants?'scope.tree':'scope.own')]:[]),result.caption.account?`${t('scope.account')} ${result.caption.account.slice(0,8)}`:t('scope.all_accounts'),result.caption.dates].join(' · '):'';
  return <Panel title={t('scope.title')} eyebrow={t('app.local_attribution')}>
    <p>{t('scope.description')}</p>
    <form className="scoped-evidence-form" onSubmit={event=>{event.preventDefault();const data=new FormData(event.currentTarget);void submit(String(data.get('start')),String(data.get('end')));}}>
      <label>{t('scope.project')}<select value={project} onChange={e=>{setProject(e.target.value);setThread('');}}><option value="">{t('scope.all_projects')}</option>{catalog?.projects.map(p=><option key={p.id} value={p.id}>{p.label}</option>)}</select></label>
      <label>{t('scope.chat')}<select value={thread} onChange={e=>setThread(e.target.value)}><option value="">{t('scope.all_chats')}</option>{catalog?.roots.filter(r=>!project||r.project===project).map(r=><option key={r.id} value={r.id}>{r.label??t('scope.untitled')}</option>)}</select></label>
      <label>{t('scope.account')}<select value={account} onChange={e=>setAccount(e.target.value)}><option value="">{t('scope.all_accounts')}</option>{catalog?.accounts.map((id,i)=><option key={id} value={id}>{t('scope.account')} {i+1} · {id.slice(0,8)}</option>)}</select></label>
      <label>{t('scope.start')}<input name="start" required type="date" defaultValue={start}/></label>
      <label>{t('scope.end')}<input name="end" required type="date" defaultValue={end}/></label>
      <label>{t('scope.grain')}<select value={grain} onChange={e=>setGrain(e.target.value as ScopeQuery['grain'])}>{(['day','week','month'] as const).map(g=><option key={g} value={g}>{t(`scope.${g}`)}</option>)}</select></label>
      <label><span>{t('scope.tree')}</span><input type="checkbox" style={{width:'auto',alignSelf:'start'}} checked={includeDescendants} disabled={!thread} onChange={e=>setIncludeDescendants(e.target.checked)}/></label>
      <button className="refresh-button" type="submit" disabled={!catalog||pending} aria-busy={pending}>{t('scope.query')}</button>
    </form>
    <p className="usage-breakdown-note">{t(includeDescendants&&thread?'scope.tree_note':'scope.own_note')} {t('scope.catalog_limit')}</p>
    {pending&&<p role="status">{t('scope.loading')}</p>}
    {failed&&<p role="alert">{t('scope.failed')}</p>}
    {failed&&!catalog&&<button type="button" onClick={()=>setCatalogRevision(n=>n+1)}>{t('scope.retry')}</button>}
    {result&&<section className="scoped-evidence-result" aria-label={t('scope.result')}>
      <p>{caption}</p>
      {!display?<p role="status">{t(result.value.status==='unresolved'?'scope.unresolved':result.value.status==='pending'?'scope.pending':'scope.no_records')}</p>:<>
        <UsageTrendChart key={JSON.stringify(result.value.query)} scope={result.value} metric="total"/>
        <UsageBreakdownTable rows={display.byModel} identityLabel={t('sessions.models_used')} scopeKey={caption}/>
        <details><summary>{t('sessions.accounts_used')}</summary><UsageBreakdownTable rows={display.byAccount} identityLabel={t('sessions.accounts_used')} scopeKey={caption} resolveLabel={r=>r.id?`${t('scope.account')} ${r.id.slice(0,8)}`:t('sessions.unknown_dimension')}/></details>
        <details open><summary>{t('scope.date_table')}</summary><UsageBreakdownTable rows={display.byTime} identityLabel={t('scope.date_table')} scopeKey={caption}/></details>
      </>}
    </section>}
  </Panel>;
}
