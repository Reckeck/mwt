// `mise run check-<name> [-- --update]`: one static check of tests/checks (vitest project `checks`).
// `--update` (check-ui, check-fsd) rewrites the baseline: it reaches the test as CHECK_UPDATE=1.
import { spawnSync } from 'node:child_process'

const [name, ...flags] = process.argv.slice(2)
const known = ['onboarding', 'routes', 'forms', 'ui', 'fsd']
if (!known.includes(name)) {
  console.error(`run-check: unknown check "${name}" (one of ${known.join(', ')})`)
  process.exit(2)
}
const update = flags.includes('--update')
if (update && !['ui', 'fsd'].includes(name)) {
  console.error(`run-check: check-${name} has no baseline to update`)
  process.exit(2)
}
const run = spawnSync(
  'pnpm',
  ['exec', 'vitest', 'run', '--project', 'checks', `tests/checks/${name}.check.ts`, '--reporter', 'default'],
  { stdio: 'inherit', env: { ...process.env, CHECK_UPDATE: update ? '1' : '0' } },
)
process.exit(run.status ?? 1)
