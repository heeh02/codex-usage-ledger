import type { ScopeResponse } from '../api/sourceScope';
import type { MetricKey } from '../api/types';
import { metricValue } from '../lib';
import { hasCacheWriteAmount } from '../shared/cacheWriteDisplay';
import { civilKey, civilTime, nextBucket, timeDomain, type TrendPoint } from './time';

export function scopedTrendSeries(scope: ScopeResponse, metric: MetricKey) {
  const {grain}=scope.query;
  const start=civilTime(civilKey(scope.query.start,scope.query.timezone));
  const end=civilTime(civilKey(scope.query.end,scope.query.timezone));
  const points:TrendPoint[]=(scope.display?.byTime??[]).map(row=>{
    const key=row.id;
    if(key===null || !(grain==='month'?/^\d{4}-\d{2}$/:/^\d{4}-\d{2}-\d{2}$/).test(key))throw new Error('Invalid scoped calendar key');
    const date=grain==='month'?`${key}-01`:key;
    const time=civilTime(date);
    if(!Number.isFinite(time)||new Date(time).toISOString().slice(0,10)!==date||time>=end||nextBucket(date,grain)<=start)throw new Error('Scoped bucket outside calendar range');
    return {date,value:metric==='cacheWrite'&&!hasCacheWriteAmount(row.usage)?null:metricValue(row.usage,metric,row.events),previous:null,local:null};
  }).sort((a,b)=>civilTime(a.date)-civilTime(b.date));
  return {points,grain,account:false,domain:timeDomain(points.map(p=>p.date),grain,scope.query)};
}
