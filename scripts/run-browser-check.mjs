// `mise run check-layout|check-links [-- flags]`: the vitest project `layout` (tests/e2e/layout, tests/e2e/links).
//   --quick (1280 and 390 / 1280), --report (check-layout: print only), --update (check-links: rewrite baseline),
//   --overlays[=id,id] (check-layout: only those layers). They reach the tests as E2E_* variables.
//   --jobs N: N processes with one shared Vite and a Chromium each, the work split by E2E_SHARD=i/N
//   (tests/e2e/harness/shard.ts). Default min(4, cores / 2), at least 1. --jobs 1 is one plain vitest run.
//   A failed process fails the check; each one's output is kept whole in .cache/check-<name>/job-i.log
//   and printed one after another when all are done.
//   --shard K/N: only the K-th of N shares of the units (the CI matrix); --jobs then splits that share.
//   Every run writes .cache/check-<name>/timings.json (unit -> seconds, all processes merged);
//   --update-weights writes tests/e2e/<name>/weights.json from it (the weights the shares are balanced by).
import { spawn, spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { cpus } from 'node:os'
import { join } from 'node:path'

import { startVite } from '../tests/e2e/harness/server.mjs'

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

let jobs = Math.max(1, Math.min(4, Math.floor(cpus().length / 2)))
const jobsAt = flags.findIndex((flag) => flag === '--jobs' || flag.startsWith('--jobs='))
if (jobsAt >= 0) {
  const value = flags[jobsAt].includes('=') ? flags[jobsAt].split('=')[1] : flags[jobsAt + 1]
  jobs = Number(value)
  if (!Number.isInteger(jobs) || jobs < 1) {
    console.error(`run-browser-check: --jobs needs a whole number >= 1, got "${value}"`)
    process.exit(2)
  }
}
const shardAt = flags.findIndex((flag) => flag === '--shard' || flag.startsWith('--shard='))
if (shardAt >= 0) {
  const value = flags[shardAt].includes('=') ? flags[shardAt].split('=')[1] : flags[shardAt + 1]
  const [index, total] = String(value).split('/').map(Number)
  if (!(Number.isInteger(index) && Number.isInteger(total) && index >= 1 && index <= total)) {
    console.error(`run-browser-check: --shard needs K/N with 1 <= K <= N, got "${value}"`)
    process.exit(2)
  }
  env.E2E_SHARD_CI = `${index}/${total}`
}
const updateWeights = flags.includes('--update-weights')
if (env.E2E_UPDATE && (jobs > 1 || env.E2E_SHARD_CI)) {
  // The baseline is written from all the errors of one run; a process knows only its own.
  if (env.E2E_SHARD_CI) {
    console.error('run-browser-check: --update writes the baseline from a whole run, so no --shard')
    process.exit(2)
  }
  if (jobsAt >= 0) console.error('run-browser-check: --update writes the baseline from a whole run, so --jobs 1')
  jobs = 1
}
const partial = Boolean(env.E2E_SHARD_CI || env.E2E_QUICK || env.E2E_OVERLAYS !== undefined)

const root = new URL('..', import.meta.url).pathname
const logs = join(root, '.cache', `check-${name}`)
const timingsDir = join(logs, 'timings')
rmSync(timingsDir, { recursive: true, force: true })
mkdirSync(logs, { recursive: true })
const vitest = ['exec', 'vitest', 'run', '--project', 'layout', files[name], '--reporter=verbose']
const started = Date.now()

/** The timings of every process of this run, merged into timings.json; returns them. */
function mergeTimings() {
  const merged = {}
  if (existsSync(timingsDir)) {
    for (const file of readdirSync(timingsDir)) {
      Object.assign(merged, JSON.parse(readFileSync(join(timingsDir, file), 'utf8')))
    }
  }
  const sorted = Object.fromEntries(Object.entries(merged).sort(([left], [right]) => (left < right ? -1 : 1)))
  writeFileSync(join(logs, 'timings.json'), `${JSON.stringify(sorted, null, 2)}\n`)
  return sorted
}

function finish(code) {
  const timings = mergeTimings()
  const sum = Object.values(timings).reduce((total, seconds) => total + seconds, 0)
  console.log(
    `run-browser-check: ${name}: ${Math.round((Date.now() - started) / 1000)} s wall, ${Math.round(sum)} s of units in ${Object.keys(timings).length} units (${logs}/timings.json)`,
  )
  if (updateWeights && code === 0) {
    const file = join(root, 'tests', 'e2e', name, 'weights.json')
    // A whole run replaces the file; a partial one (--quick, --shard, --overlays) updates the units it ran.
    const kept = partial && existsSync(file) ? JSON.parse(readFileSync(file, 'utf8')) : {}
    const next = { ...kept }
    for (const [unit, seconds] of Object.entries(timings)) next[unit] = Math.max(0.1, Math.round(seconds * 10) / 10)
    const sorted = Object.fromEntries(Object.entries(next).sort(([left], [right]) => (left < right ? -1 : 1)))
    writeFileSync(file, `${JSON.stringify(sorted, null, 2)}\n`)
    console.log(`run-browser-check: weights written: ${file} (${Object.keys(sorted).length} units)`)
  } else if (updateWeights) {
    console.error('run-browser-check: the run failed, weights are not written')
  }
  process.exit(code)
}

if (jobs === 1) {
  const run = spawnSync('pnpm', vitest, { stdio: 'inherit', env: { ...env, E2E_SHARD: '1/1' } })
  finish(run.status ?? 1)
}

console.log(`run-browser-check: ${name} in ${jobs} jobs (logs: ${logs}/job-<i>.log)`)
// One Vite for all the processes: several dev servers on one node_modules/.vite re-optimize dependencies under each
// other and reload pages mid-test ("Failed to fetch dynamically imported module").
const { vite, url } = await startVite()
await fetch(url)
env.E2E_VITE_URL = url
const children = [vite]
const stop = () => {
  for (const child of children) child.kill('SIGTERM')
}
process.on('SIGINT', () => (stop(), process.exit(130)))
process.on('SIGTERM', () => (stop(), process.exit(143)))

const results = await Promise.all(
  Array.from({ length: jobs }, (_unused, at) => {
    const job = at + 1
    return new Promise((done) => {
      const child = spawn('pnpm', vitest, {
        env: { ...env, E2E_SHARD: `${job}/${jobs}` },
        stdio: ['ignore', 'pipe', 'pipe'],
      })
      children.push(child)
      let output = ''
      child.stdout.on('data', (chunk) => (output += chunk))
      child.stderr.on('data', (chunk) => (output += chunk))
      child.on('error', (error) => done({ job, code: 1, output: `${output}\n${error}` }))
      child.on('close', (code, signal) => {
        const result = { job, code: code ?? (signal ? 1 : 0), output }
        writeFileSync(join(logs, `job-${job}.log`), output)
        done(result)
      })
    })
  }),
)

for (const { job, code, output } of results) {
  console.log(`\n===== job ${job}/${jobs}: ${code === 0 ? 'passed' : `FAILED (exit ${code})`} =====`)
  console.log(output.trimEnd())
}
stop()
const failed = results.filter(({ code }) => code !== 0).map(({ job }) => job)
console.log(
  `\nrun-browser-check: ${name}: ${failed.length ? `jobs ${failed.join(', ')} of ${jobs} failed` : `all ${jobs} jobs passed`}`,
)
finish(failed.length ? 1 : 0)
