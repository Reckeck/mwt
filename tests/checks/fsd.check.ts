// The FSD gate (AGENTS.md rule 9): Steiger (steiger.config.ts) over src/, with the project's own reading
// of the methodology:
// - `shared` has no single public API: its segments are imported by file (`@/shared/api/ipc`,
//   `@/shared/store/settings`), so a "sidestep" into shared is fine;
// - many slices per layer are a choice (excessive-slicing is off in the config).
//
// Debts that predate the gate live in `scripts/check-fsd.baseline.json` as `{ "<rule> <path>": count }`,
// the same way as check-ui: a path may not get worse, `mise run check-fsd -- --update` writes the smaller
// number when it gets better; new paths must be clean.
import { spawnSync } from 'node:child_process'
import { closeSync, existsSync, mkdirSync, openSync, readFileSync, writeFileSync } from 'node:fs'
import { join, relative } from 'node:path'

import { describe, it } from 'vitest'

import { expectNone, ROOT, UPDATE } from './lib/report'

const BASELINE = join(ROOT, 'scripts/check-fsd.baseline.json')

interface Diagnostic {
  ruleName: string
  severity: string
  message: string
  location: { path: string }
}

function steiger(): Diagnostic[] {
  // Into a file, not a pipe: Steiger exits before a pipe drains, and a report over 64 KiB came back
  // cut off. Writes to a file are synchronous, so it arrives whole.
  const dir = join(ROOT, '.cache')
  mkdirSync(dir, { recursive: true })
  const path = join(dir, 'steiger.json')
  const out = openSync(path, 'w')
  const run = spawnSync('pnpm', ['exec', 'steiger', './src', '--reporter', 'json'], {
    cwd: ROOT,
    encoding: 'utf8',
    stdio: ['ignore', out, 'pipe'],
  })
  closeSync(out)
  try {
    return JSON.parse(readFileSync(path, 'utf8'))
  } catch {
    throw new Error(`steiger gave no report\n${run.stderr}`)
  }
}

/** Shared segments are imported by file in this project. */
const intoShared = (item: Diagnostic) =>
  item.ruleName === 'fsd/no-public-api-sidestep' && /importing from "@\/shared\//.test(item.message)

const diagnostics = steiger()
const counts: Record<string, number> = {}
const messages: Record<string, string[]> = {}
for (const item of diagnostics) {
  if (item.severity !== 'error' || intoShared(item)) continue
  const key = `${item.ruleName.replace(/^fsd\//, '')} ${relative(ROOT, item.location.path)}`
  counts[key] = (counts[key] ?? 0) + 1
  messages[key] = [...(messages[key] ?? []), item.message]
}

const first = !existsSync(BASELINE)
const baseline: Record<string, number> = first ? {} : JSON.parse(readFileSync(BASELINE, 'utf8'))

describe('check-fsd', () => {
  if (UPDATE || first) {
    it('writes the baseline', () => {
      const next: Record<string, number> = {}
      for (const [key, count] of Object.entries(counts))
        next[key] = first ? count : Math.min(count, baseline[key] ?? count)
      const sorted = Object.fromEntries(Object.entries(next).sort(([left], [right]) => left.localeCompare(right)))
      writeFileSync(BASELINE, `${JSON.stringify(sorted, null, 2)}\n`)
      console.log(`check-fsd: baseline ${first ? 'written' : 'updated'}, ${Object.keys(sorted).length} entries`)
    })
    return
  }

  it('no layer boundary or public-API problem past the baseline', () => {
    const problems: string[] = []
    for (const [key, count] of Object.entries(counts)) {
      const allowed = baseline[key] ?? 0
      if (count <= allowed) continue
      const was = allowed ? ` (baseline ${allowed})` : ''
      problems.push(`${key}: ${count}${was} — ${messages[key][0]}`)
    }
    expectNone(problems)
  })

  it('reports', () => {
    console.log(`check-fsd: ${diagnostics.filter((item) => item.severity === 'warn').length} warnings`)
  })
})
