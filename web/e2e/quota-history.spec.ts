import { expect, test } from '@playwright/test';
import { mockQuotaHistory } from '../src/api/quotaHistoryMock';
import { mockQuotaIntervalUsage } from '../src/api/quotaIntervalUsage';

test('interval usage is loaded on demand and unsafe source overlap hides totals', async ({ page }) => {
  const history = mockQuotaHistory({ account: 'all' });
  let reads = 0; let review = false;
  await page.route('**/v1/quota-history?**', route => route.fulfill({ contentType: 'application/json', body: JSON.stringify(history) }));
  await page.route('**/v1/quota-interval-usage?**', route => {
    reads++;
    const result = mockQuotaIntervalUsage(history.intervals[0], history.selections![0]);
    return route.fulfill({ contentType: 'application/json', body: JSON.stringify(review ? { ...result, status: 'source_overlap_review', usage: null, events: null, models: [], projects: [] } : result) });
  });
  await page.goto('/e2e/quota-history-http.html');
  const row = page.locator('.quota-history-item').first();
  await expect(page.locator('.quota-history-item')).toHaveCount(20); expect(reads).toBe(0);
  await row.getByRole('button', { name: '查看区间 Token 明细', exact: true }).click();
  const detail = row.getByRole('region', { name: '查看区间 Token 明细', exact: true });
  await expect(detail.locator('.local-composition')).toContainText('12 M');
  await expect(detail.locator('table')).toHaveCount(2); expect(reads).toBe(1);
  review = true; await detail.getByRole('button', { name: '重新查询区间用量', exact: true }).click();
  await expect(detail).toContainText('等待来源合并修复');
  await expect(detail.locator('.local-composition')).toHaveCount(0);
});

test('HTTP failure and index waiting retain the accepted history without a mock fallback', async ({ page }) => {
  let mode: 'good' | 'failure' | 'pending' = 'good';
  await page.route('**/v1/quota-history?**', route => {
    if (mode === 'failure') return route.fulfill({ status: 503, contentType: 'application/json', body: JSON.stringify({ code: 'request_failed' }) });
    const response = mockQuotaHistory({ account: 'all' });
    return route.fulfill({ contentType: 'application/json', body: JSON.stringify(mode === 'pending' ? { ...response, indexReady: false, intervals: [], next: null, selections: [] } : response) });
  });
  await page.goto('/e2e/quota-history-http.html');
  const section = page.getByRole('region', { name: '已保存的额度历史', exact: true });
  const rows = section.locator('.quota-history-item');
  await expect(rows).toHaveCount(20);
  const first = await rows.first().getAttribute('data-history-id');
  mode = 'failure'; await section.getByRole('button', { name: '下一页', exact: true }).click();
  await expect(section.getByRole('alert')).toBeVisible();
  await expect(rows.first()).toHaveAttribute('data-history-id', first!);
  await expect(section).not.toContainText('演示历史');
  mode = 'pending'; await section.getByRole('button', { name: '刷新历史视图', exact: true }).click();
  await expect(section).toContainText('暂时保留已加载的查看版本');
  await expect(rows).toHaveCount(20);
  await expect(section.getByRole('button', { name: '下一页', exact: true })).toBeDisabled();
});

for (const width of [560, 1280]) {
  test(`full retained history traverses three pages and retains its view at ${width}px`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height: 800 });
    await page.goto('/');
    await page.locator('.language-select select').selectOption('en');
    if (width <= 900) await page.locator('.mobile-page-select').selectOption('accounts');
    else await page.locator('.sidebar-page-links').getByRole('button', { name: /Accounts & quota/ }).click();
    await page.getByRole('tab', { name: 'Quota history', exact: true }).click();
    await expect(page.locator('.filter-bar')).toHaveCount(0);
    await expect(page.locator('.data-status-strip')).toHaveCount(0);
    const section = page.getByRole('region', { name: 'Retained quota history', exact: true });
    await section.scrollIntoViewIfNeeded();
    const rows = section.locator('.quota-history-item');
    await expect(rows).toHaveCount(20);
    await expect(section.getByText(/Independent of the calendar range/)).toBeVisible();
    const first = await rows.first().getAttribute('data-history-id');
    await section.getByRole('button', { name: 'Next page', exact: true }).click();
    await expect(rows.first()).not.toHaveAttribute('data-history-id', first!);
    await expect(rows).toHaveCount(20);
    await expect(rows.first()).toBeFocused();
    await section.getByRole('button', { name: 'Next page', exact: true }).click();
    await expect(rows).toHaveCount(5);
    await expect(section.getByText(/End of this frozen view/)).toBeVisible();
    await expect(section.getByRole('button', { name: 'Next page', exact: true })).toBeDisabled();
    await section.getByRole('button', { name: 'Previous page', exact: true }).click();
    await expect(rows).toHaveCount(20);
    await section.getByRole('button', { name: 'Previous page', exact: true }).click();
    await expect(rows.first()).toHaveAttribute('data-history-id', first!);
    await rows.first().locator('summary').click();
    await expect(rows.first()).toContainText('Token sample interval');
    await rows.first().getByRole('button', { name: 'Inspect interval Token usage', exact: true }).click();
    await expect(rows.first().locator('.local-composition')).toContainText('12 M');
    await expect(rows.first().locator('.quota-interval-usage')).toContainText('not complete cycle usage');
    await rows.first().locator('.quota-interval-usage').scrollIntoViewIfNeeded();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: testInfo.outputPath('quota-history.png') });
    await page.locator('.language-select select').selectOption('zh-CN');
    await expect(section).toHaveCount(0);
    const chinese = page.getByRole('region', { name: '已保存的额度历史', exact: true });
    await expect(chinese.locator('.quota-history-item').first()).toHaveAttribute('data-history-id', first!);
    await expect(chinese).toContainText('独立于“使用情况”');
    await page.locator('.account-switcher-trigger:visible').click();
    await page.getByRole('dialog').getByRole('combobox').selectOption('acct-personal');
    await expect(chinese.locator('.quota-history-item')).toHaveCount(15);
    expect(await chinese.locator('.quota-history-item').evaluateAll(items => items.every(item => item.getAttribute('data-account-id') === 'acct-personal'))).toBe(true);
    await expect(chinese.getByRole('button', { name: '上一页', exact: true })).toBeDisabled();
    await page.getByRole('tab', { name: '额度历史', exact: true }).press('ArrowLeft');
    await expect(page.locator('.filter-bar')).toHaveCount(1);
    await page.getByRole('tab', { name: '使用情况', exact: true }).press('ArrowRight');
    await expect(page.locator('.filter-bar')).toHaveCount(0);
    await page.reload();
    await expect(page.getByRole('tab', { name: '额度历史', exact: true })).toHaveAttribute('aria-selected', 'true');
    await expect(chinese.locator('.quota-history-item')).toHaveCount(15);
    const persisted = await page.evaluate(() => Object.values(sessionStorage).join('\n'));
    expect(persisted).not.toContain('signature');
    expect(persisted).not.toContain('synthetic-044');
  });
}
