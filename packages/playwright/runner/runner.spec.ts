import { test, expect } from '@playwright/test';

test('configured server is ready and receives its environment', {
  tag: ['@bloomery:NIXLIB-PLAYWRIGHT-RUNNER-001'],
}, async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('heading')).toHaveText('server environment reached the browser');
});

test('runner launches its pinned headless Chromium', {
  tag: ['@bloomery:NIXLIB-PLAYWRIGHT-RUNNER-002'],
}, async ({ browser, browserName }, testInfo) => {
  expect(browserName).toBe('chromium');
  expect(browser.version()).toMatch(/^\d+\./);
  expect(process.env.PLAYWRIGHT_BROWSERS_PATH).toMatch(/^\/nix\/store\//);
  expect(process.env.PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD).toBe('1');
  expect(testInfo.project.use.headless).toBe(true);
});
