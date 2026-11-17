import { test, expect } from '@playwright/test';

// The docs intentionally reference optional remote fonts and a GitHub badge.
// Keep browser behavior independent of those services, including outside Nix.
test.beforeEach(async ({ context }) => {
  await context.route('**/*', route => {
    const url = new URL(route.request().url());
    return url.hostname === '127.0.0.1' ? route.continue() : route.abort();
  });
});

test('sidebar opens the quickstart guide', {
  tag: ['@bloomery:DOCS-UI-BROWSER-001'],
}, async ({ page }) => {
  await page.goto('/');
  await page.locator('.sidebar-nav').getByRole('link', { name: 'Quickstart', exact: true }).click();
  await expect(page).toHaveURL(/\/docs\/quickstart$/);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Quickstart Guide');
  await expect(page.locator('.sidebar-link.active')).toHaveText('Quickstart');
});

test('search filters guides and opens a result', {
  tag: ['@bloomery:DOCS-UI-BROWSER-002'],
}, async ({ page }) => {
  await page.goto('/');
  await page.locator('.quick-search-box').click();
  await page.getByPlaceholder('Search documentation, options, architecture...').fill('profiles');
  const results = page.locator('.search-result-item:visible');
  await expect(results).toHaveCount(1);
  await results.click();
  await expect(page).toHaveURL(/\/profiles$/);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Strongly-Typed Profiles');
});

test('Escape closes keyboard-opened search', {
  tag: ['@bloomery:DOCS-UI-BROWSER-003'],
}, async ({ page }) => {
  await page.goto('/');
  await page.keyboard.press('ControlOrMeta+k');
  await expect(page.locator('#docs-search-input')).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(page.locator('#search-modal')).toBeHidden();
});

test('native stylesheet and logo load in the browser', {
  tag: ['@bloomery:DOCS-UI-BROWSER-004'],
}, async ({ page }) => {
  await page.goto('/docs/quickstart');
  const stylesheet = page.locator('link[rel="stylesheet"][href^="/_topcoat/assets/"]');
  await expect(stylesheet).toHaveCount(1);
  expect(await stylesheet.evaluate((element: HTMLLinkElement) => element.sheet !== null)).toBe(true);
  const logo = page.getByRole('img', { name: 'Bloomery logo' });
  await expect(logo).toBeVisible();
  expect(await logo.evaluate((element: HTMLImageElement) => element.complete && element.naturalWidth > 0)).toBe(true);
});
