import { expect, test } from '@playwright/test';

for (const width of [560, 700, 900, 1280]) {
  test(`account viewing scope is independent of login and accessible at ${width}px`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width, height: 800 });
    await page.goto('/');
    await page.locator('.language-select select').selectOption('en');
    const trigger = page.locator('.account-switcher-trigger:visible');
    await expect(trigger).toHaveAttribute('data-account', 'all');
    await trigger.click();
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();
    await expect(dialog.getByText(/Codex login is unchanged/)).toBeVisible();
    await expect(dialog.getByRole('combobox')).toHaveCount(0);
    const login = await dialog.locator('.account-switcher-option').filter({has:page.locator('small')}).getAttribute('data-account');
    const ids = await dialog.locator('.account-switcher-option').evaluateAll(options => options.map(option => option.getAttribute('data-account')!));
    const account = ids.filter(id => id !== 'all').at(-1)!;
    expect(account).toBeTruthy();
    await dialog.locator(`[data-account="${account}"]`).click();
    await expect(dialog).not.toBeVisible();
    await expect(trigger).toHaveAttribute('data-account', account);
    expect(await page.locator('.workspace-topbar').evaluate(node => node.getBoundingClientRect().height)).toBeLessThan(200);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    await expect(trigger).toBeFocused();
    await trigger.click();
    if (login) await expect(dialog.locator('.account-switcher-option').filter({has:page.locator('small')})).toHaveAttribute('data-account',login);
    await expect(dialog.locator(`[data-account="${account}"]`)).toHaveAttribute('aria-pressed','true');
    await dialog.locator('[data-account="all"]').hover();
    expect(await dialog.locator('[data-account="all"]').evaluate(node=>getComputedStyle(node).outlineStyle)).toBe('solid');
    await page.keyboard.press('Escape');
    await expect(dialog).not.toBeVisible();
    await expect(trigger).toBeFocused();
    await trigger.click();
    await dialog.locator('.account-switcher-option').last().scrollIntoViewIfNeeded();
    const box = await dialog.boundingBox();
    expect(box!.x).toBeGreaterThanOrEqual(0);
    expect(box!.x + box!.width).toBeLessThanOrEqual(width);
    expect(box!.y + box!.height).toBeLessThanOrEqual(800);
    if (width === 560 || width === 1280) await page.screenshot({ path: testInfo.outputPath('account-menu.png') });
    await dialog.locator('[data-account="all"]').click();
    await expect(trigger).toHaveAttribute('data-account', 'all');
  });
}
