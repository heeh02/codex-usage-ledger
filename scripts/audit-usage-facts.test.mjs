import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, existsSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

test('snapshot receipt is read-only, opaque and sensitive to row changes', () => {
  const directory = mkdtempSync(join(tmpdir(), 'usage-fact-test-'));
  const path = join(directory, 'fixture.sqlite3');
  const script = fileURLToPath(new URL('./audit-usage-facts.mjs', import.meta.url));
  const names = ['usage_events','daily_usage_rollups','hourly_usage_rollups','reconstruction_usage_events','reconstruction_daily_rollups','reconstruction_hourly_rollups','compacted_event_keys','official_daily_usage','official_thread_usage'];
  try {
    const missing = spawnSync(process.execPath, [script, path]);
    assert.notEqual(missing.status, 0);
    assert.equal(existsSync(path), false);
    execFileSync('sqlite3', [path, names.map(name => `CREATE TABLE ${name}(id TEXT PRIMARY KEY,value INTEGER); INSERT INTO ${name} VALUES('private-fixture-marker',12);`).join('\n')]);
    const before = readFileSync(path);
    const run = () => execFileSync(process.execPath,[script,path],{encoding:'utf8'});
    const first = run();
    assert.equal(first.includes('private-fixture-marker'),false);
    assert.deepEqual(readFileSync(path), before);
    assert.equal(run(), first);
    const receipt = JSON.parse(first);
    assert.equal(receipt.tables.length,9);
    assert.ok(receipt.tables.every(table => table.rows===1 && /^[a-f0-9]{64}$/.test(table.sha256)));
    execFileSync('sqlite3',[path,'UPDATE daily_usage_rollups SET value=13;']);
    const after = JSON.parse(run());
    assert.deepEqual(after.tables.filter((table,i) => table.sha256!==receipt.tables[i].sha256).map(table => table.table), ['daily_usage_rollups']);
  } finally { rmSync(directory,{recursive:true}); }
});
