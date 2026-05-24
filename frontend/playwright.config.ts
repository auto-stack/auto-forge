import { defineConfig, devices } from '@playwright/test'

/**
 * Playwright E2E configuration for AutoForge.
 *
 * Tests the full application including:
 * - Relay pipeline UI (run list, detail, gates)
 * - Forge chat flows
 * - Specs workspace
 */
export default defineConfig({
  testDir: './e2e',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'list',
  use: {
    baseURL: 'http://localhost:3031',
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },

  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],

  webServer: {
    command: 'cd ../backend && ./target/release/auto-forge.exe',
    url: 'http://localhost:3031/api/forge/relay/runs',
    reuseExistingServer: true,
    timeout: 30_000,
  },
})
