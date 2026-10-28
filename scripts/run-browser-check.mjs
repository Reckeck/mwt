// `mise run check-layout|check-links [-- flags]`: the vitest project `layout` (tests/e2e/layout, tests/e2e/links).
//   --quick (1280 and 390 / 1280), --report (check-layout: print only), --update (check-links: rewrite baseline),
//   --overlays[=id,id] (check-layout: only those layers). They reach the tests as E2E_* variables.
import { spawnSync } from 'node:child_process'

const [name, ...flags] = process.argv.slice(2)
const files = { layout: 'tests/e2e/layout/layout.e2e.ts', links: 'tests/e2e/links/links.e2e.ts' }
if (!files[name]) {
  console.error(`run-browser-check: unknown check "${name}" (layout, links)`)
  process.exit(2)
}
const env = { ...process.env }
if (flags.includes('--quick')) env.E2E_QUICK = '1'
if (flags.includes('--report')) env.E2E_REPORT = '1'
if (flags.includes('--update')) env.E2E_UPDATE = '1'
const overlays = flags.find((flag) => flag.startsWith('--overlays'))
if (overlays) env.E2E_OVERLAYS = overlays.split('=')[1] ?? ''
const run = spawnSync('pnpm', ['exec', 'vitest', 'run', '--project', 'layout', files[name]], { stdio: 'inherit', env })
process.exit(run.status ?? 1)
