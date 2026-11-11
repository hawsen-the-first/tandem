import { defineConfig, devices } from '@playwright/test';

// The stack (database, migrations, api serving the built client, worker) is started by
// dev/scripts/e2e.sh, which sets E2E_BASE_URL. The cookie is Secure; Chromium and Firefox accept it
// over http://localhost (Safari does not, so it is not part of this suite).
const baseURL = process.env['E2E_BASE_URL'] ?? 'http://localhost:8090';

export default defineConfig({
  testDir: 'e2e',
  testMatch: '**/*.spec.ts',
  fullyParallel: false,
  workers: 1,
  retries: process.env['CI'] === undefined ? 0 : 1,
  timeout: 60_000,
  expect: { timeout: 10_000 },
  reporter: process.env['CI'] === undefined ? 'list' : [['list'], ['html', { open: 'never' }]],
  outputDir: 'test-results',
  use: {
    baseURL,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [
    // First-run setup happens exactly once on the fresh database and leaves an admin session
    // behind for every other project.
    { name: 'setup', testMatch: 'setup.spec.ts', use: { ...devices['Desktop Chrome'] } },
    {
      name: 'chromium',
      testIgnore: 'setup.spec.ts',
      dependencies: ['setup'],
      use: { ...devices['Desktop Chrome'], storageState: 'e2e/.auth/admin.json' },
    },
    {
      name: 'firefox',
      testIgnore: 'setup.spec.ts',
      dependencies: ['setup'],
      use: { ...devices['Desktop Firefox'], storageState: 'e2e/.auth/admin.json' },
    },
  ],
});
