import { test, expect } from '@playwright/test';

test('intentional browser assertion failure', async ({ page }) => {
  await page.goto('/');
  expect(await page.title(), 'runner must propagate assertion failures').toBe('absent title');
});
