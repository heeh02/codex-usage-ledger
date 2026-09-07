import { expect, test } from '@playwright/test';

for (const width of [560, 1280]) {
  test(`quota preview identifies its evidence bounds and remains scrollable at ${width}px`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height: 800 });
    await page.goto('/');
    await page.locator('.language-select select').selectOption('en');
    if (width <= 900) await page.locator('.mobile-page-select').selectOption('accounts');
    else await page.locator('.sidebar-page-links').getByRole('button', { name: /Accounts & quota/ }).click();
    const section = page.locator('.quota-cycle-section');
    await section.scrollIntoViewIfNeeded();
    await expect(section.getByText('Quota observation interval preview', { exact: true })).toBeVisible();
    await expect(section.getByText(/not a complete quota archive/)).toBeVisible();
    const rows = section.locator('.quota-cycle-row');
    await expect(rows.first()).toBeVisible();
    await expect(rows.first()).toContainText('Local Token sample interval');
    await expect(rows.first()).toContainText('Last observed usage');
    await expect(rows.first()).toContainText('First retained observation');
    const unreadable = await rows.first().locator('small').evaluateAll(labels => labels.some(label => {
      const style = getComputedStyle(label);
      return parseFloat(style.fontSize) < 12 || style.textOverflow === 'ellipsis';
    }));
    expect(unreadable).toBe(false);
    await rows.last().scrollIntoViewIfNeeded();
    await expect(rows.last()).toBeInViewport();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.screenshot({ path: testInfo.outputPath('quota-preview.png') });
    await page.locator('.language-select select').selectOption('zh-CN');
    await expect(section).toContainText('额度观测区间预览');
    await expect(rows.first()).toContainText('最后观测已用');
  });
}
