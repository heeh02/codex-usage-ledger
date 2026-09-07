import { expect, test } from '@playwright/test';

test('complete ranking and model pages preserve global ranks, shares, refresh and scope', async ({ page }) => {
  await page.goto('/e2e/complete-breakdowns.html');
  const projects = page.locator('.breakdown-project');
  const models = page.locator('.breakdown-model');
  await expect(projects.locator('.breakdown-row')).toHaveCount(20);
  await expect(models.locator('tbody tr').first().locator('td').nth(4)).toHaveAttribute('title', '4,500,000');
  await expect(models.locator('tbody tr').first().locator('td').nth(4)).toContainText('4.5 M');
  const share = await projects.locator('.breakdown-values span').first().textContent();
  // First row is about 4.3% of all 45 confirmed rows, not the first 20.
  expect(parseFloat(share!)).toBeLessThan(5);
  await projects.locator('.breakdown-pagination button').last().click();
  await expect(projects.locator('.breakdown-rank').first()).toHaveText('21');
  await projects.locator('.breakdown-pagination button').last().click();
  await expect(projects.locator('.breakdown-row')).toHaveCount(6);
  await expect(projects.locator('.breakdown-rank').last()).toHaveText('46');
  await expect(projects.locator('.breakdown-values strong').last()).toHaveText('—');
  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(projects.locator('.breakdown-rank').first()).toHaveText('41');
  await models.locator('.breakdown-pagination button').last().click();
  await models.locator('.breakdown-pagination button').last().click();
  await expect(models.locator('tbody tr')).toHaveCount(6);
  await expect(models.locator('tbody tr').last().locator('td').nth(1)).toHaveText('—');
  await models.getByRole('button', { name: 'Synthetic 45', exact: true }).click();
  await expect(page.getByTestId('selection')).toHaveText('model:synthetic-45');
  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect(models.locator('tbody tr').first()).toContainText('Synthetic 41');
  await page.getByRole('button', { name: 'Metric', exact: true }).click();
  await expect(models.locator('tbody tr').first()).toContainText('Synthetic 45');
  await models.locator('.breakdown-pagination button').last().click();
  await page.getByRole('button', { name: 'Scope', exact: true }).click();
  await expect(models.locator('tbody tr').first()).toContainText('Synthetic 45');
  await models.locator('.breakdown-pagination button').last().click();
  await page.getByRole('button', { name: 'Shrink', exact: true }).click();
  await expect(models.locator('tbody tr')).toHaveCount(3);
  await expect(models.locator('.breakdown-pagination')).not.toContainText('21');
});

test('model table is bilingual, keyboard accessible and container-scrollable across widths and zoom', async ({ page }, testInfo) => {
  await page.goto('/e2e/complete-breakdowns.html');
  const models = page.locator('.breakdown-model');
  for (const width of [560, 700, 900, 1280]) {
    await page.setViewportSize({ width, height: 800 });
    for (const zoom of [.8, 1, 1.6]) {
      await page.locator('main').evaluate((node, value) => { node.style.zoom = String(value); node.style.height = `${100 / value}vh`; }, zoom);
      const scroller = models.locator('.usage-breakdown-scroll');
      await scroller.scrollIntoViewIfNeeded();
      await scroller.focus();
      for (let i = 0; i < 12; i++) await page.keyboard.press('ArrowRight');
      await models.locator('tbody tr').first().locator('td').last().scrollIntoViewIfNeeded();
      await expect(models.locator('tbody tr').first().locator('td').last()).toBeInViewport();
      expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    }
  }
  await page.getByRole('button', { name: 'Language', exact: true }).click();
  await expect(models.locator('thead')).toContainText('Cache read');
  await expect(models.locator('thead')).toContainText('Output');
  await models.locator('.breakdown-pagination button').last().focus();
  await page.keyboard.press('Enter');
  await expect(models.locator('tbody tr').first()).toContainText('Synthetic 21');
  await page.setViewportSize({ width: 900, height: 800 });
  await page.locator('main').evaluate(node => { node.style.zoom = '1'; node.style.height = '100vh'; });
  await models.scrollIntoViewIfNeeded();
  await page.screenshot({ path: testInfo.outputPath('complete-models.png') });
});

test('real overview and project routes both use the shared model breakdown', async ({ page }) => {
  await page.goto('/');
  await page.locator('.language-select select').selectOption('en');
  await page.locator('.overview-tabs nav').getByRole('button', { name: 'Models', exact: true }).click();
  await expect(page.locator('.breakdown-model table')).toBeVisible();
  await expect(page.locator('.breakdown-model thead')).toContainText('Cache read');
  await page.locator('.project-item').first().click();
  await expect(page.locator('.breakdown-model table')).toBeVisible();
  await expect(page.locator('.breakdown-model thead')).toContainText('Output');
});
