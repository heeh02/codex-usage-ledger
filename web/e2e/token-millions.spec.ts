import { expect, test } from '@playwright/test';

test('overview, charts and sidebar use M while request metrics remain counts', async ({ page }, testInfo) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto('/');
  await page.locator('.language-select select').selectOption('en');
  const metric = page.getByRole('combobox', { name: 'Metric', exact: true });
  await expect(page.locator('.explorer-pulse > article').first().locator('strong')).toContainText('M');
  await expect(page.locator('.project-item small').first()).toContainText('M');
  const axis = page.locator('.usage-time-chart .chart-axis-label').first();
  await expect(axis).toContainText('M');
  await metric.selectOption('requests');
  await expect(axis).not.toContainText('M');
  await expect(page.locator('.explorer-pulse > article').first().locator('strong')).not.toContainText('M');
  await expect(page.locator('.explorer-pulse > article').nth(1).locator('strong')).toContainText('M');
  await metric.selectOption('total');
  await expect(axis).toContainText('M');
  await page.setViewportSize({ width: 560, height: 800 });
  const clipped = await page.locator('.chart-axis-label').evaluateAll(labels => labels.some(label => {
    const box = (label as SVGGraphicsElement).getBBox();
    return box.x < 0;
  }));
  expect(clipped).toBe(false);
  await page.locator('.usage-time-chart').first().scrollIntoViewIfNeeded();
  await page.screenshot({ path: testInfo.outputPath('million-trend.png') });
});

test('million-token model table keeps units, exact values and scroll access in both languages', async ({ page }) => {
  await page.setViewportSize({ width: 900, height: 700 });
  await page.goto('/e2e/session-distributions.html');
  const modelRow = page.locator('table').first().locator('tbody tr').first();
  await expect(modelRow.locator('td').nth(1)).toHaveText('12 M');
  await expect(modelRow.locator('td').nth(1)).toHaveAttribute('title', '12,000,000');
  await expect(modelRow.locator('td').nth(3)).toHaveText('7 M');
  await expect(modelRow.locator('td').last()).toHaveText('1');
  await page.getByRole('button', { name: 'Language', exact: true }).click();
  await expect(modelRow.locator('td').nth(1)).toHaveText('12 M');
  await page.getByRole('button', { name: 'Own / Tree', exact: true }).click();
  await expect(page.locator('table').first().locator('tbody tr')).toHaveCount(2);
  await page.getByRole('button', { name: 'Narrow / Wide', exact: true }).click();
  await page.setViewportSize({ width: 560, height: 700 });
  const scroller = page.locator('.usage-breakdown-scroll').first();
  await scroller.focus();
  for (let step = 0; step < 12; step++) await page.keyboard.press('ArrowRight');
  await expect.poll(() => scroller.evaluate(node => node.scrollLeft)).toBeGreaterThan(0);
  const finalCell = modelRow.locator('td').last();
  await finalCell.scrollIntoViewIfNeeded();
  await expect(finalCell).toBeInViewport();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
});
