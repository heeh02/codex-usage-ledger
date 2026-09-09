import { expect,it } from 'vitest';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { validateScope,type ScopeResponse } from '../api/sourceScope';
import { scopedTrendSeries } from './scopedTrend';
import { contiguous } from './time';
import { UsageTrendChart } from '../components/UsageTrendChart';
import { I18nContext } from '../i18n';
import { enMessages } from '../locales/en';

function scope(keys:string[],grain:'day'|'week'|'month'='day'):ScopeResponse {
  const usage={input:80,cached:40,cacheWrite:0,cacheWriteObservedInput:0,cacheWriteCoverage:0,uncached:40,output:20,reasoning:5,total:100};
  const rows=keys.map(id=>({id,events:1,usage}));
  const n=keys.length;
  const total={...usage,input:80*n,cached:40*n,uncached:40*n,output:20*n,reasoning:5*n,total:100*n};
  const aggregate=[{id:'all',events:n,usage:total}];
  return {version:2,status:'available',query:{start:'2026-07-01T00:00:00Z',end:'2026-08-01T00:00:00Z',timezone:'UTC',grain},historyComplete:false,productionPolicyChanged:false,data:{records:keys.length},display:{usage:total,byTime:rows,byModel:aggregate,byAccount:aggregate,byProject:aggregate,byThread:aggregate}};
}
it('sorts observed dates without zero filling and keeps the requested calendar domain',()=>{
  const data=scope(['2026-07-30','2026-07-15']);
  const series=scopedTrendSeries(validateScope(data,data.query),'total');
  expect(series.points.map(p=>p.date)).toEqual(['2026-07-15','2026-07-30']);
  expect(series.points.reduce((n,p)=>n+p.value!,0)).toBe(200);
  expect(contiguous(series.points,'day',()=>true)).toHaveLength(2);
  expect(series.domain).toEqual([Date.parse('2026-07-01T00:00:00Z'),Date.parse('2026-08-01T00:00:00Z')]);
});
it('normalizes month keys, accepts an overlapping partial week and rejects out-of-range data',()=>{
  expect(scopedTrendSeries(scope(['2026-07'],'month'),'total').points[0].date).toBe('2026-07-01');
  expect(scopedTrendSeries(scope(['2026-06-29'],'week'),'total').points).toHaveLength(1);
  expect(()=>scopedTrendSeries(scope(['2026-06-01']),'total')).toThrow();
  expect(()=>scopedTrendSeries(scope(['invalid']),'total')).toThrow();
});
it('does not turn unknown writes into measured zero',()=>{
  const data=scope(['2026-07-01']);
  expect(scopedTrendSeries(data,'cacheWrite').points[0].value).toBeNull();
  data.display!.byTime[0].usage={...data.display!.byTime[0].usage,cacheWriteCoverage:1,cacheWriteObservedInput:80};
  expect(scopedTrendSeries(data,'cacheWrite').points[0].value).toBe(0);
});
it('uses discrete marks for sparse data and the existing line renderer for enough observations',()=>{
  const render=(data:ScopeResponse)=>renderToStaticMarkup(createElement(I18nContext.Provider,{value:{language:'en',setLanguage:()=>{},t:(key)=>enMessages[key]}},createElement(UsageTrendChart,{scope:data,metric:'total'})));
  expect(render(scope(['2026-07-01','2026-07-03']))).not.toContain('<polyline');
  const html=render(scope(Array.from({length:8},(_,i)=>`2026-07-${String(i+1).padStart(2,'0')}`)));
  expect(html).toContain('<polyline');expect(html).not.toContain('NaN');
  expect(html).not.toContain('<th>Previous</th>');
});
