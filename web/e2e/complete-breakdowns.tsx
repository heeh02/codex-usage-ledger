import { createRoot } from 'react-dom/client';
import { useState } from 'react';
import { I18nProvider } from '../src/I18nProvider';
import { useI18n } from '../src/i18n';
import { BreakdownPanel } from '../src/components/BreakdownPanel';
import type { BreakdownRow, BreakdownsResponse, MetricKey, TokenUsage } from '../src/api/types';
import '../src/styles.css';

const zero: TokenUsage = { input: 0, cached: 0, uncached: 0, cacheWrite: 0, cacheWriteCoverage: 0, cacheWriteObservedInput: 0, output: 0, reasoning: 0, total: 0 };
const rows: BreakdownRow[] = Array.from({ length: 45 }, (_, index) => {
  const input = (45 - index) * 1_000_000;
  const output = (index + 1) * 1_000;
  return { id: `synthetic-${index + 1}`, label: `Synthetic ${index + 1}`, confirmedEvents: 1, shareOfConfirmed: 0,
    usage: { confirmed: { input, cached: input * .8, uncached: input * .1, cacheWrite: input * .1, cacheWriteCoverage: 1,
      cacheWriteObservedInput: input, output, reasoning: 0, total: input + output }, quarantined: zero, unknown: zero } };
});
rows.push({ id: 'no-evidence', label: 'No evidence', confirmedEvents: 0, shareOfConfirmed: 0, usage: { confirmed: zero, quarantined: zero, unknown: zero } });
// A recorded positive write must remain visible after coverage-only reconciliation.
rows[0].usage.confirmed.cacheWriteCoverage = 0;
rows[0].usage.confirmed.cacheWriteObservedInput = 0;

function Fixture() {
  const { language, setLanguage } = useI18n();
  const [metric, setMetric] = useState<MetricKey>('total');
  const [revision, setRevision] = useState(0);
  const [count, setCount] = useState(46);
  const [scope, setScope] = useState('all');
  const [selected, setSelected] = useState('');
  const data: BreakdownsResponse = { generatedAt: `2025-04-15T12:00:${String(revision).padStart(2, '0')}Z`,
    period: { key: 'month', label: 'Synthetic month', start: '2025-04-01T00:00:00Z', end: `2025-04-15T12:00:${String(revision).padStart(2, '0')}Z`,
      timezone: 'UTC', crossesMonth: false, crossesYear: false, windowKind: 'calendar' }, officialAccounts: [],
    project: rows.slice(0, count), model: rows.slice(0, count), account: [] };
  return <main style={{ width: '100%', height: '100vh', overflow: 'auto', padding: 16, boxSizing: 'border-box' }}>
    <p>SYNTHETIC COMPONENT QA · NOT REAL USAGE</p>
    <button onClick={() => setLanguage(language === 'en' ? 'zh-CN' : 'en')}>Language</button>
    <button onClick={() => setMetric(metric === 'total' ? 'output' : 'total')}>Metric</button>
    <button onClick={() => setRevision(revision + 1)}>Refresh</button>
    <button onClick={() => setCount(count === 46 ? 3 : 46)}>Shrink</button>
    <button onClick={() => setScope(scope === 'all' ? 'other' : 'all')}>Scope</button>
    <output data-testid="selection">{selected}</output>
    <BreakdownPanel data={data} metric={metric} scopeKey={scope} dimensions={['project', 'model']} onSelect={(dimension, id) => setSelected(`${dimension}:${id}`)} />
  </main>;
}
createRoot(document.getElementById('root')!).render(<I18nProvider><Fixture /></I18nProvider>);
