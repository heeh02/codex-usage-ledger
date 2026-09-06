#!/usr/bin/env node
// Read-only, streaming fingerprints for migration receipts. No auth/catalog
// tables, source log bodies, prompts or per-account values are printed.
// Input MUST be a consistent, quiescent snapshot, not a live writer's database:
// independent sqlite3 processes do not share a cross-table read transaction.
import { spawn, execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { statSync } from 'node:fs';

const database = process.argv[2];
if (!database || process.argv.length !== 3) throw new Error('Usage: node scripts/audit-usage-facts.mjs <existing-sqlite-file>');
const initial = statSync(database, { bigint: true });
if (!initial.isFile()) throw new Error('Audit input must be an existing snapshot file');
const tables = [
  'usage_events', 'daily_usage_rollups', 'hourly_usage_rollups',
  'reconstruction_usage_events', 'reconstruction_daily_rollups', 'reconstruction_hourly_rollups',
  'compacted_event_keys', 'official_daily_usage', 'official_thread_usage',
];
const query = sql => execFileSync('sqlite3', ['-readonly', '-json', database, sql], { encoding: 'utf8', maxBuffer: 1024 * 1024 });
const quote = value => `"${value.replaceAll('"', '""')}"`;
const output = { formatVersion: 1, readOnly: true, requiresQuiescentSnapshot: true, scope: 'allowlisted_usage_rows_not_schema_or_complete_ledger', schemaVersion: JSON.parse(query('PRAGMA user_version;'))[0].user_version, tables: [] };
for (const table of tables) {
  const columns = JSON.parse(query(`PRAGMA table_info(${quote(table)});`));
  if (!columns.length) throw new Error(`Missing required usage table: ${table}`);
  const primary = columns.filter(column => column.pk > 0).sort((a, b) => a.pk - b.pk);
  const order = primary.length ? primary.map(column => quote(column.name)).join(',') : 'rowid';
  const count = JSON.parse(query(`SELECT COUNT(*) AS count FROM ${quote(table)};`))[0].count;
  const hash = createHash('sha256');
  let bytes = 0;
  // Quote mode is typed SQL literal serialization. Stable column/primary-key
  // order makes before/after row comparison independent of physical pages.
  const child = spawn('sqlite3', ['-readonly', '-quote', database, `SELECT * FROM ${quote(table)} ORDER BY ${order};`], { stdio: ['ignore', 'pipe', 'pipe'] });
  child.stdout.on('data', chunk => { bytes += chunk.length; hash.update(chunk); });
  let failure = false;
  child.stderr.on('data', () => { failure = true; });
  await new Promise((resolve, reject) => {
    child.on('error', reject);
    child.on('close', code => code === 0 && !failure ? resolve() : reject(new Error(`Read failed for ${table}`)));
  });
  output.tables.push({ table, rows: count, serializedBytes: bytes, sha256: hash.digest('hex') });
}
const final = statSync(database, { bigint: true });
if (initial.size !== final.size || initial.mtimeNs !== final.mtimeNs || initial.ino !== final.ino) throw new Error('Snapshot changed during audit; discard this run');
console.log(JSON.stringify(output, null, 2));
