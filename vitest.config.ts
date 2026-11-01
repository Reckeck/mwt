import { defineConfig } from 'vitest/config'

// One config, one project per kind of test (`unit`, `e2e`; `checks` and `layout` later).
// `extends` reuses plugins and aliases from vite.config.ts.
export default defineConfig({
  test: {
    projects: [
      {
        extends: './vite.config.ts',
        test: {
          name: 'unit',
          environment: 'happy-dom',
          include: ['src/**/*.test.ts', 'tests/e2e/harness/**/*.test.ts'],
          setupFiles: ['src/test/setup.ts'],
          restoreMocks: true,
        },
      },
      {
        // Static rules over the sources (onboarding, routes, forms, ui, fsd): a rule is a describe.
        extends: './vite.config.ts',
        test: {
          name: 'checks',
          environment: 'node',
          include: ['tests/checks/**/*.check.ts'],
        },
      },
      {
        // The app on the mock in headless Chromium: Vite and the browser start once (globalSetup).
        extends: './vite.config.ts',
        test: {
          name: 'e2e',
          environment: 'node',
          include: ['tests/e2e/flows/**/*.e2e.ts'],
          globalSetup: ['tests/e2e/harness/global-setup.ts'],
          fileParallelism: false,
          testTimeout: 60_000,
          hookTimeout: 60_000,
        },
      },
      {
        // check-layout and check-links: the same harness, long sweeps (own timeouts per test).
        extends: './vite.config.ts',
        test: {
          name: 'layout',
          environment: 'node',
          include: ['tests/e2e/layout/**/*.e2e.ts', 'tests/e2e/links/**/*.e2e.ts'],
          globalSetup: ['tests/e2e/harness/global-setup.ts'],
          fileParallelism: false,
          testTimeout: 900_000,
          hookTimeout: 60_000,
        },
      },
    ],
  },
})
