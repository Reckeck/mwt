import { defineConfig } from 'vitest/config'

// One config, one project per kind of test (`unit` now; `checks` and `e2e` later).
// `extends` reuses plugins and aliases from vite.config.ts.
export default defineConfig({
  test: {
    projects: [
      {
        extends: './vite.config.ts',
        test: {
          name: 'unit',
          environment: 'happy-dom',
          include: ['src/**/*.test.ts'],
          setupFiles: ['src/test/setup.ts'],
          restoreMocks: true,
        },
      },
    ],
  },
})
