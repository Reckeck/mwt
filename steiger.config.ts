// FSD rules for `mise run check-fsd` (tests/checks/fsd.check.ts keeps the baseline).
import fsd from '@feature-sliced/steiger-plugin'
import { defineConfig } from 'steiger'

export default defineConfig([
  ...fsd.configs.recommended,
  {
    rules: {
      // Many small slices per layer are the project's choice (AGENTS.md rule 9).
      'fsd/excessive-slicing': 'off',
      // A slice with one consumer is a hint to move it into that page, not an error.
      'fsd/insignificant-slice': 'warn',
    },
  },
])
