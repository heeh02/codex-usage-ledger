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
    await expect(dialog.getByText(/Changes the reporting scope only/)).toBeVisible();
    const select = dialog.getByRole('combobox');
    await expect(select).toBeEnabled();
    const login = await dialog.locator('.account-switcher-login > span').innerText();
    const ids = await select.locator('option').evaluateAll(options => options.map(option => (option as HTMLOptionElement).value));
    const account = ids.filter(id => id !== 'all').at(-1)!;
    expect(account).toBeTruthy();
    await select.selectOption(account);
    await expect(dialog).not.toBeVisible();
    await expect(trigger).toHaveAttribute('data-account', account);
    expect(await page.locator('.workspace-topbar').evaluate(node => node.getBoundingClientRect().height)).toBeLessThan(200);
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    await expect(trigger).toBeFocused();
    await trigger.click();
    await expect(dialog.locator('.account-switcher-login > span')).toHaveText(login);
    await page.keyboard.press('Escape');
    await expect(dialog).not.toBeVisible();
    await expect(trigger).toBeFocused();
    await trigger.click();
    await dialog.getByRole('button', { name: 'Accounts & quota', exact: true }).scrollIntoViewIfNeeded();
    const box = await dialog.boundingBox();
    expect(box!.x).toBeGreaterThanOrEqual(0);
    expect(box!.x + box!.width).toBeLessThanOrEqual(width);
    expect(box!.y + box!.height).toBeLessThanOrEqual(800);
    if (width === 560 || width === 1280) await page.screenshot({ path: testInfo.outputPath('account-menu.png') });
    await select.selectOption('all');
    await expect(trigger).toHaveAttribute('data-account', 'all');
  });
}
