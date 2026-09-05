import { expect, test } from '@playwright/test';

const widths = [560, 700, 900, 1280];

for (const width of [560, 700, 900, 1280, 1440]) {
  test(`English filter controls do not overlap at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    await page.goto('/');
    await page.locator('.language-select select').selectOption('en');
    await expect(page.locator('.workspace-heading h1')).toHaveText('Overview');
    for (const zoom of [0.8, 1, 1.6]) {
      await page.evaluate(value => { document.body.style.zoom = String(value); }, zoom);
      const violations = await page.locator('.filter-bar').evaluate(bar => {
        const bounds = bar.getBoundingClientRect();
        const targets = Array.from(bar.querySelectorAll('.filter-scope-label, .filter-field, .period-option, .refresh-button'));
        const failures: string[] = [];
        for (let index = 0; index < targets.length; index++) {
          const a = targets[index].getBoundingClientRect();
          if (a.left < bounds.left - 1 || a.right > bounds.right + 1) failures.push(`outside-${index}`);
          for (let other = index + 1; other < targets.length; other++) {
            const b = targets[other].getBoundingClientRect();
            if (Math.min(a.right, b.right) - Math.max(a.left, b.left) > 1 &&
                Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) > 1) failures.push(`overlap-${index}-${other}`);
          }
        }
        return failures;
      });
      expect(violations).toEqual([]);
      await expect(page.getByRole('button', { name: 'Refresh local usage' })).toBeVisible();
    }
  });
}

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

test('request evidence traverses three pages and returns to the exact first row', async ({ page }) => {
  await page.goto('/');
  await page.locator('button.project-item').filter({ hasText: 'Project Atlas' }).click();
  await page.locator('.project-view-tabs button').last().click();
  await page.locator('.session-row').filter({ hasText: 'Audit parser boundaries' }).click();
  const panel = page.getByRole('region', { name: '请求证据明细', exact: true });
  await panel.getByRole('button', { name: '请求证据明细', exact: true }).click();
  const rows = panel.getByRole('table').locator('tbody tr');
  await expect(rows).toHaveCount(100);
  const first = await rows.first().innerText();
  await expect(panel.getByRole('button', { name: '上一页', exact: true })).toBeDisabled();
  await panel.getByRole('button', { name: '下一页', exact: true }).click();
  await expect(rows.first()).not.toHaveText(first);
  await expect(rows).toHaveCount(100);
  const second = await rows.first().innerText();
  await panel.getByRole('button', { name: '下一页', exact: true }).click();
  await expect(rows).toHaveCount(5);
  await expect(panel.getByRole('button', { name: '下一页', exact: true })).toBeDisabled();
  await panel.getByRole('button', { name: '上一页', exact: true }).click();
  await expect(rows).toHaveCount(100);
  await expect(rows.first()).toHaveText(second);
  await panel.getByRole('button', { name: '首页', exact: true }).click();
  await expect(rows.first()).toHaveText(first);
  await expect(panel.getByRole('button', { name: '上一页', exact: true })).toBeDisabled();
  await page.setViewportSize({ width: 560, height: 820 });
  const scroller = panel.getByRole('region', { name: '可滚动请求表，使用方向键查看其余列' });
  await scroller.focus();
  await expect(scroller).toBeFocused();
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth)).toBeLessThanOrEqual(1);
  await scroller.press('ArrowRight');
  await expect.poll(() => scroller.evaluate(element => element.scrollLeft)).toBeGreaterThan(0);
});
