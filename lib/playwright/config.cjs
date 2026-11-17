const path = require('node:path');
const { defineConfig } = require('@playwright/test');
const settings = require('./settings.json');

const url = new URL(settings.url);
if (url.protocol !== 'http:' || url.hostname !== '127.0.0.1') {
  throw new Error('playwrightTest: url must use http://127.0.0.1');
}

const reportDir = process.env.BLOOMERY_PLAYWRIGHT_REPORT_DIR;

module.exports = defineConfig({
  testDir: path.join(__dirname, 'source'),
  outputDir: path.join(reportDir, 'test-results'),
  forbidOnly: true,
  retries: 0,
  workers: 1,
  timeout: settings.testTimeout,
  reporter: [
    ['list'],
    ['json', { outputFile: path.join(reportDir, 'results.json') }],
    ['html', { outputFolder: path.join(reportDir, 'html'), open: 'never' }],
  ],
  use: {
    browserName: 'chromium',
    // Use the full Chromium in the selected bundle, rather than the separately
    // packaged headless shell. The outer Nix sandbox provides build isolation.
    channel: 'chromium',
    headless: true,
    baseURL: settings.url,
    launchOptions: { chromiumSandbox: false },
    screenshot: 'only-on-failure',
    trace: 'retain-on-failure',
  },
  webServer: {
    command: settings.serverCommand,
    url: settings.url,
    env: { ...settings.env, PORT: url.port || '80' },
    timeout: settings.serverTimeout,
    reuseExistingServer: false,
    stdout: 'pipe',
    stderr: 'pipe',
  },
});
