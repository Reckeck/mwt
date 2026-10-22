// The FSD gate (AGENTS.md rule 9): Steiger (steiger.config.ts) over src/, with the
// project's own reading of the methodology:
// - `shared` has no single public API: its segments are imported by file
//   (`@/shared/api/ipc`, `@/shared/store/settings`), so a "sidestep" into shared is fine;
// - many slices per layer are a choice (excessive-slicing is off in the config).
//
// Debts that predate the gate live in `scripts/check-fsd.baseline.json` as
// `{ "<rule> <path>": count }`, the same way as check-ui: a path may not get worse,
// `--update` writes the smaller number when it gets better; new paths must be clean.
//
//   node scripts/check-fsd.mjs            # gate
//   node scripts/check-fsd.mjs --update   # shrink the baseline (never grows it, but on first run)
import { spawnSync } from 'node:child_process'
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, writeFileSync } from 'node:fs'
import { join, relative } from 'node:path'

const root = new URL('..', import.meta.url).pathname
const baselinePath = join(root, 'scripts/check-fsd.baseline.json')
const update = process.argv.includes('--update')

// Into a file, not a pipe: Steiger exits before a pipe drains, and a report over
// 64 KiB came back cut off. Writes to a file are synchronous, so it arrives whole.
const reportDir = join(root, '.cache')
mkdirSync(reportDir, { recursive: true })
const reportPath = join(reportDir, 'steiger.json')
const out = openSync(reportPath, 'w')
const run = spawnSync('pnpm', ['exec', 'steiger', './src', '--reporter', 'json'], {
  cwd: root,
  encoding: 'utf8',
  stdio: ['ignore', out, 'pipe'],
})
closeSync(out)
let diagnostics
try {
  diagnostics = JSON.parse(readFileSync(reportPath, 'utf8'))
} catch {
  console.error(`check-fsd: steiger gave no report\n${run.stderr}`)
  process.exit(1)
}

/** Shared segments are imported by file in this project. */
const intoShared = (item) =>
  item.ruleName === 'fsd/no-public-api-sidestep' && /importing from "@\/shared\//.test(item.message)

const counts = {}
const messages = {}
for (const item of diagnostics) {
  if (item.severity !== 'error' || intoShared(item)) continue
  const key = `${item.ruleName.replace(/^fsd\//, '')} ${relative(root, item.location.path)}`
  counts[key] = (counts[key] ?? 0) + 1
  ;(messages[key] ??= []).push(item.message)
}

const first = !existsSync(baselinePath)
const baseline = first ? {} : JSON.parse(readFileSync(baselinePath, 'utf8'))
const problems = []
for (const [key, count] of Object.entries(counts)) {
  const allowed = baseline[key] ?? 0
  if (count <= allowed) continue
  const was = allowed ? ` (baseline ${allowed})` : ''
  problems.push(`${key}: ${count}${was} — ${messages[key][0]}`)
}

if (update || first) {
  const next = {}
  for (const [key, count] of Object.entries(counts)) {
    next[key] = first ? count : Math.min(count, baseline[key] ?? count)
  }
  const sorted = Object.fromEntries(Object.entries(next).sort(([left], [right]) => left.localeCompare(right)))
  writeFileSync(baselinePath, `${JSON.stringify(sorted, null, 2)}\n`)
  console.log(`check-fsd: baseline ${first ? 'written' : 'updated'}, ${Object.keys(sorted).length} entries`)
  if (first) process.exit(0)
}

const warnings = diagnostics.filter((item) => item.severity === 'warn').length
for (const problem of problems) console.error(`error: ${problem}`)
console.log(`check-fsd: ${problems.length} new problems, ${warnings} warnings`)
process.exit(problems.length ? 1 : 0)
