import { expect,it } from 'vitest';
import { validateScope,type ScopeQuery,type ScopeResponse } from './sourceScope';
import { ledgerResponseError } from './errors';

const query:ScopeQuery={start:'2026-01-01T00:00:00Z',end:'2026-02-01T00:00:00Z',timezone:'UTC',grain:'day',thread:'root'};
function response():ScopeResponse {
  const usage={input:100,cached:80,cacheWrite:0,cacheWriteObservedInput:0,cacheWriteCoverage:0,uncached:20,output:20,reasoning:5,total:120};
  const rows=[{id:'value',events:1,usage}];
  return {version:2,status:'available',query,historyComplete:false,productionPolicyChanged:false,data:{records:1},display:{usage,byTime:rows,byModel:rows,byAccount:rows,byProject:rows,byThread:rows}};
}
it('accepts conserved scoped facts while preserving unknown write coverage',()=>{
  expect(validateScope(response(),query).display!.usage.cacheWriteCoverage).toBe(0);
});
it('rejects scope substitution, inconsistent dimensions and invented pending totals',()=>{
  expect(()=>validateScope(response(),{...query,thread:'other'})).toThrow();
  expect(()=>validateScope(response(),{...query,includeDescendants:true})).toThrow();
  const changed=response();changed.display!.byModel=[];
  expect(()=>validateScope(changed,query)).toThrow();
  expect(()=>validateScope({...response(),status:'pending'},query)).toThrow();
  expect(validateScope({...response(),status:'unresolved',data:null,display:null},query).display).toBeNull();
});
it('recognizes only the explicit unavailable-snapshot service response',async()=>{
  expect((await ledgerResponseError({status:503,json:async()=>({code:'snapshot_unavailable'})})).code).toBe('snapshot_unavailable');
  expect((await ledgerResponseError({status:500,json:async()=>({code:'snapshot_unavailable'})})).code).toBe('request_failed');
});
