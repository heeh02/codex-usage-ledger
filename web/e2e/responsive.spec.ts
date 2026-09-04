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
