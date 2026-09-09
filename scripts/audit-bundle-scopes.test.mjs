import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

test('HTTP scope audit detects drift without printing dimension identifiers', async () => {
  const script=fileURLToPath(new URL('./audit-bundle-scopes.mjs',import.meta.url));
  const usage={input:100,cached:40,cacheWrite:10,cacheWriteObservedInput:100,uncached:50,output:20,reasoning:5,total:120};
  let corrupt=false;
  let corruptQuality=false;
  const server=createServer((request,response)=>{
    assert.equal(request.method,'GET');
    assert.ok(request.url.startsWith('/v1/bundle?'));
    response.setHeader('Content-Type','application/json');
    response.end(JSON.stringify({summary:{confirmedEvents:1,usage:{confirmed:usage},filters:{accounts:[{id:'private-marker'}],projects:[],models:[]}},
      quality:{states:[{state:'confirmed',eventCount:1,usage:{...usage,output:corruptQuality?21:20}}]},
      timeseries:{points:[{confirmed:{...usage,total:corrupt?121:120}}]},
      breakdowns:Object.fromEntries(['account','project','model'].map(key=>[key,[{usage:{confirmed:usage}}]]))}));
  });
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  const run=()=>new Promise((resolve,reject)=>{
    const child=spawn(process.execPath,[script,`http://127.0.0.1:${server.address().port}`]);
    let stdout='',stderr='';
    child.stdout.on('data',chunk=>stdout+=chunk);
    child.stderr.on('data',chunk=>stderr+=chunk);
    child.on('error',reject);
    child.on('close',code=>resolve({code,stdout,stderr}));
  });
  try {
    const valid=await run();
    assert.equal(valid.code,0,valid.stderr);
    assert.equal(valid.stdout.includes('private-marker'),false);
    const report=JSON.parse(valid.stdout);
    assert.equal(report.reports.length,8);
    assert.equal(report.allPassed,true);
    assert.equal(report.sourceAccuracyProven,false);
    corrupt=true;
    const invalid=await run();
    assert.equal(invalid.code,1);
    const failed=JSON.parse(invalid.stdout);
    assert.equal(failed.allPassed,false);
    assert.ok(failed.reports.every(row=>row.errors.includes('curve.total')));
    corrupt=false; corruptQuality=true;
    const badQuality=await run();
    assert.equal(badQuality.code,1);
    assert.ok(JSON.parse(badQuality.stdout).reports.every(row=>row.errors.includes('quality.output')));
  } finally { await new Promise(resolve=>server.close(resolve)); }
});
