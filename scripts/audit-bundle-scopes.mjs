#!/usr/bin/env node
// Run only against an isolated, quiescent ledger server. GET-only; output omits
// account/model/project IDs and values. Equality does not prove source accuracy.
const base = new URL(process.argv[2] ?? 'http://127.0.0.1:47134');
if (base.protocol !== 'http:' || base.hostname !== '127.0.0.1' || base.username || base.password || base.pathname !== '/') throw new Error('Explicit loopback server origin required');
const fields = ['input','cached','cacheWrite','cacheWriteObservedInput','uncached','output','reasoning','total'];
const fetchBundle = async filters => {
  const url = new URL('/v1/bundle', base);
  url.search = new URLSearchParams({ timezone: 'Asia/Shanghai', ...filters });
  const response = await fetch(url, { signal: AbortSignal.timeout(30000), redirect: 'error' });
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  return response.json();
};
const inspect = bundle => {
  const errors = [];
  const expected = bundle.summary.usage.confirmed;
  const sources = { curve: bundle.timeseries.points.map(point=>point.confirmed) };
  const confirmedState = bundle.quality.states.find(state=>state.state==='confirmed');
  if (!confirmedState) errors.push('quality.missing_confirmed');
  else {
    sources.quality = [confirmedState.usage];
    if (confirmedState.eventCount !== bundle.summary.confirmedEvents) errors.push('quality.confirmed_count');
  }
  for (const dimension of ['account','project','model']) sources[dimension] = bundle.breakdowns[dimension].map(row=>row.usage.confirmed);
  for (const [source, rows] of Object.entries(sources)) for (const field of fields) {
    const total = rows.reduce((sum,row)=>sum+BigInt(row[field]),0n);
    if (total !== BigInt(expected[field])) errors.push(`${source}.${field}`);
  }
  if (expected.input+expected.output !== expected.total || expected.uncached+expected.cached+expected.cacheWrite !== expected.input) errors.push('summary.conservation');
  return errors;
};
const reports=[];
let catalog;
for (const period of ['today','week','month','rolling7','rolling30','year','lifetime']) {
  const start=performance.now();
  try {
    const bundle=await fetchBundle({period});
    if(period==='lifetime') catalog=bundle.summary.filters;
    reports.push({scope:period,milliseconds:Math.round(performance.now()-start),errors:inspect(bundle)});
  } catch(error) { reports.push({scope:period,error:String(error)}); }
}
if(catalog) for(const dimension of ['account','project','model']) {
  const options=catalog[`${dimension}s`].filter(option=>option.id!=='all').slice(0,2);
  for(const [index,option] of options.entries()) {
    const start=performance.now();
    try { const bundle=await fetchBundle({period:'lifetime',[dimension]:option.id});
      reports.push({scope:`${dimension}-selection-${index+1}`,milliseconds:Math.round(performance.now()-start),errors:inspect(bundle)});
    } catch(error) {reports.push({scope:`${dimension}-selection-${index+1}`,error:String(error)});}
  }
}
console.log(JSON.stringify({reports,allPassed:reports.every(report=>!report.error&&!report.errors.length),sourceAccuracyProven:false},null,2));
if(reports.some(report=>report.error||report.errors.length)) process.exitCode=1;
