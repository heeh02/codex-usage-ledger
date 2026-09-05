import { useEffect, useState } from 'react';
import type { MetricKey, TimeseriesResponse } from '../api/types';
import { metricLabel, metricValue } from '../lib';
import { useI18n } from '../i18n';

export function ActivityCalendar({ data, metric, onSelectDay }: { data: TimeseriesResponse; metric: MetricKey; onSelectDay: (date: string) => void }) {
  const { t } = useI18n();
  const lastYear = Number(data.period.end.slice(0, 4)) || new Date().getFullYear();
  const firstYear = Math.min(Number(data.period.start.slice(0, 4)) || lastYear, lastYear);
  const [year, setYear] = useState(lastYear);
  useEffect(() => setYear(lastYear), [firstYear, lastYear]);
  const dates: string[] = [];
  for (let date = new Date(Date.UTC(year, 0, 1)); date.getUTCFullYear() === year; date.setUTCDate(date.getUTCDate() + 1)) dates.push(date.toISOString().slice(0, 10));
  const values = new Map((data.dailyPoints ?? []).map(point => [point.date, metricValue(point.confirmed, metric, point.confirmedEvents)]));
  const max = Math.max(1, ...dates.map(date => values.get(date) ?? 0));
  const offset = (new Date(Date.UTC(year, 0, 1)).getUTCDay() + 6) % 7;
  return <section className="panel activity-calendar">
    <header className="panel-heading"><h2>{t('calendar.title')}</h2><select aria-label={t('calendar.year')} value={year} onChange={event => setYear(Number(event.target.value))}>
      {Array.from({ length: lastYear - firstYear + 1 }, (_, index) => firstYear + index).map(value => <option value={value} key={value}>{value}</option>)}
    </select></header>
    <p>{t('calendar.scope')}</p>
    <div className="activity-calendar-grid">
      {Array.from({ length: offset }, (_, index) => <span key={`blank-${index}`} />)}
      {dates.map(date => {
        const value = values.get(date);
        const label = `${date} · ${metricLabel(metric)} ${value === undefined ? '—' : value.toLocaleString()}`;
        return <button key={date} type="button" disabled={value === undefined} title={label} aria-label={label} data-level={value === undefined ? 'unknown' : value === 0 ? '0' : String(Math.ceil(value / max * 4))} onClick={() => onSelectDay(date)} />;
      })}
    </div>
  </section>;
}
