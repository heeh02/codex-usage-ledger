import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  fullyParallel: false,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? 'github' : 'list',
  use: {
    baseURL: 'http://127.0.0.1:47128',
    browserName: 'chromium',
    channel: 'chrome',
    colorScheme: 'light',
    locale: 'zh-CN',
    trace: 'retain-on-failure',
  },
  webServer: {
    command: 'npm run dev -- --port 47128',
    url: 'http://127.0.0.1:47128',
    reuseExistingServer: !process.env.CI,
    env: {
      ...process.env,
      VITE_LEDGER_DATA_MODE: 'mock',
    },
  },
});
