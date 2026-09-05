import { expect, test } from '@playwright/test';

const widths = [560, 700, 900, 1280];

for (const width of widths) {
  test(`dashboard remains usable at ${width}px`, async ({ page }) => {
    const pageErrors: string[] = [];
    page.on('pageerror', (error) => pageErrors.push(error.message));
    await page.setViewportSize({ width, height: 820 });
    await page.goto('/');
    await expect(page.locator('.workspace-heading h1')).toBeVisible();
    await expect(page.locator('.demo-notice')).toBeVisible();

    const overflow = await page.evaluate(() => ({
      document: document.documentElement.scrollWidth - window.innerWidth,
      workspace: (() => {
        const element = document.querySelector<HTMLElement>('.workspace-scroll');
        return element ? element.scrollWidth - element.clientWidth : 0;
      })(),
    }));
    expect(overflow.document).toBeLessThanOrEqual(1);
    expect(overflow.workspace).toBeLessThanOrEqual(1);
    expect(pageErrors).toEqual([]);
  });
}

test('language and project navigation preserve a complete page', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 820 });
  await page.goto('/');
  await page.locator('.language-select select').selectOption('en');
  await expect(page.locator('.workspace-heading h1')).toHaveText('Overview');

  const firstProject = page.locator('button.project-item').first();
  const projectName = (await firstProject.locator('span').textContent())?.trim();
  await firstProject.click();
  await expect(page.locator('.workspace-heading h1')).toHaveText(projectName ?? '');
  await expect(page.locator('.explorer-page')).toBeVisible();

  const canReachFooter = await page.locator('.workspace-scroll').evaluate((element) => {
    element.scrollTop = element.scrollHeight;
    return element.scrollTop + element.clientHeight >= element.scrollHeight - 2;
  });
  expect(canReachFooter).toBe(true);
});

test('account and model selection survive project and conversation navigation', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 820 });
  await page.goto('/');
  await page.getByLabel('账号', { exact: true }).selectOption('acct-personal');
  await expect(page.getByLabel('账号', { exact: true })).toHaveValue('acct-personal');
  await page.locator('button.project-item').filter({ hasText: 'Project Atlas' }).click();
  await expect(page.locator('.workspace-heading h1')).toHaveText('Project Atlas');
  await expect(page.getByLabel('账号', { exact: true })).toHaveValue('acct-personal');
  await page.getByLabel('模型', { exact: true }).selectOption('gpt-5.6-sol');
  await expect(page.getByLabel('模型', { exact: true })).toHaveValue('gpt-5.6-sol');
  await page.locator('.project-view-tabs button').last().click();
  await page.locator('.session-row').first().click();
  await expect(page.locator('.session-detail-page')).toBeVisible();
  await expect(page.getByLabel('账号', { exact: true })).toHaveValue('acct-personal');
  await expect(page.getByLabel('模型', { exact: true })).toHaveValue('gpt-5.6-sol');
});

test('dated chart values stay accessible by keyboard in a narrow window', async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 820 });
  await page.goto('/');
  await page.locator('button.project-item').filter({ hasText: 'Project Atlas' }).click();
  const chart = page.locator('.usage-time-chart').first();
  const svg = chart.locator('svg').first();
  await svg.focus();
  await svg.press('Home');
  const first = await chart.locator('.usage-time-readout > span').first().textContent();
  await svg.press('End');
  await expect(chart.locator('.usage-time-readout > span').first()).not.toHaveText(first ?? '');
  await page.setViewportSize({ width: 560, height: 820 });
  await expect(chart.locator('.usage-time-readout')).toBeVisible();
  await expect(svg).toHaveCSS('height', '260px');
  await expect(svg.locator('.chart-axis-label').first()).toHaveCSS('font-size', '12px');
  await expect(chart).toBeVisible();
});

test('conversation search updates the list without changing the project total', async ({ page }) => {
  await page.goto('/');
  await page.locator('button.project-item').filter({ hasText: 'Project Atlas' }).click();
  const total = await page.locator('.scope-metric.is-primary-scope strong').textContent();
  await page.locator('.project-view-tabs button').last().click();
  await page.getByRole('textbox', { name: '搜索全部聊天' }).fill('release');
  await page.getByRole('button', { name: '搜索', exact: true }).click();
  await expect(page.locator('.session-row')).toHaveCount(1);
  await expect(page.locator('.session-row')).toContainText('Prepare release evidence');
  await expect(page.locator('.scope-metric.is-primary-scope strong')).toHaveText(total ?? '');
});

test('child navigation changes the detail and supports returning to its parent', async ({ page }) => {
  await page.goto('/');
  await page.locator('button.project-item').filter({ hasText: 'Project Atlas' }).click();
  await page.locator('.project-view-tabs button').last().click();
  await page.locator('.session-row').filter({ hasText: 'Audit parser boundaries' }).click();
  await expect(page.locator('.session-detail-page')).toBeVisible();
  await page.getByRole('treeitem', { name: /^runtime review,/ }).click();
  await expect(page.locator('.workspace-heading h1')).toHaveText('runtime review');
  await expect(page.getByRole('treeitem')).toHaveCount(2);
  await expect(page.locator('.usage-time-chart')).toBeVisible();
  await page.getByRole('button', { name: '返回上级聊天', exact: true }).click();
  await expect(page.locator('.workspace-heading h1')).toHaveText('Audit parser boundaries and fixtures');
});

test('narrow conversation list keeps usage and export visible', async ({ page }) => {
  await page.setViewportSize({ width: 560, height: 820 });
  await page.goto('/');
  await page.locator('.mobile-page-select').selectOption('proj-atlas');
  await page.locator('.project-view-tabs button').last().click();
  await expect(page.locator('.session-usage').first()).toBeVisible();
  await expect(page.locator('.export-menu > button')).toBeVisible();
  await page.locator('.session-row').filter({ hasText: 'Audit parser boundaries' }).click();
  await expect(page.locator('.agent-own').first()).toBeVisible();
});
