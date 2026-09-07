import { expect, test } from '@playwright/test';

for (const mode of ['denied-getter', 'quota-write', 'corrupt-settings'] as const) {
  test(`dashboard survives ${mode} presentation storage`, async ({ page }) => {
    const errors: string[] = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.addInitScript(mode => {
      if (mode === 'denied-getter') {
        Object.defineProperty(window, 'sessionStorage', { configurable: true, get() { throw new DOMException('Denied', 'SecurityError'); } });
      } else if (mode === 'quota-write') {
        const session = window.sessionStorage;
        Object.defineProperty(window, 'sessionStorage', { configurable: true, value: {
          getItem: session.getItem.bind(session),
          setItem() { throw new DOMException('Full', 'QuotaExceededError'); },
        } });
      } else {
        sessionStorage.setItem('ledger.filters', JSON.stringify({ period: 'custom', startDate: 'bad-date', endDate: [], account: null, metric: {} }));
        sessionStorage.setItem('ledger.primaryPage', JSON.stringify({ value: 'missing-page' }));
        sessionStorage.setItem('ledger.sessionView', JSON.stringify({ search: {}, scope: 'invalid' }));
      }
    }, mode);
    await page.goto('/');
    await page.locator('.filter-bar').waitFor();
    await page.locator('.language-select select').selectOption('en');
    await expect(page.locator('.workspace-heading h1')).toHaveText('Overview');
    await page.locator('button.project-item').filter({ hasText: 'Project Atlas' }).click();
    await expect(page.locator('.workspace-heading h1')).toHaveText('Project Atlas');
    await page.getByRole('button', { name: 'Sessions · 2', exact: true }).click();
    await page.locator('.session-row').filter({ hasText: 'Audit parser boundaries' }).click();
    await expect(page.locator('.workspace-heading h1')).toHaveText('Audit parser boundaries and fixtures');
    expect(errors).toEqual([]);
  });
}
