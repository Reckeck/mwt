// check-layout / check-links in pieces (scripts/run-browser-check.mjs): the work is a list of units (a chunk of
// views, a chunk of overlays, a page), each with a weight in seconds from tests/e2e/<check>/weights.json.
//
//   E2E_SHARD_CI=K/N  (--shard K/N)  the K-th of N shares of the whole list: the CI matrix
//   E2E_SHARD=i/J     (--jobs J)     the i-th of J shares of that share: the processes of one machine
//
// Both splits are the same greedy one (heaviest unit first onto the lightest share, ties by name), so every
// process computes the same plan alone and every unit lands in exactly one (K, i). A unit missing from the
// weights file weighs the median of the known ones; a key without a unit is ignored.
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

export type Unit = [name: string, weight: number]
export type Share = [index: number, total: number]

const ROOT = new URL('../../..', import.meta.url).pathname

/** "K/N" with 1 <= K <= N. */
export function parseShare(raw: string | undefined, label: string): Share {
  const [index, total] = (raw ?? '1/1').split('/').map(Number)
  if (!(Number.isInteger(index) && Number.isInteger(total) && index >= 1 && total >= 1 && index <= total)) {
    throw new Error(`${label} must be K/N with 1 <= K <= N, got "${raw}"`)
  }
  return [index, total]
}

const byName = (left: string, right: string) => (left < right ? -1 : left > right ? 1 : 0)

/** Greedy split: unit name -> share number (0-based). Throws on a name listed twice. */
export function assign(units: Unit[], parts: number): Map<string, number> {
  const load = new Array<number>(parts).fill(0)
  const owner = new Map<string, number>()
  const order = [...units].sort(([nameA, weightA], [nameB, weightB]) => weightB - weightA || byName(nameA, nameB))
  for (const [name, weight] of order) {
    if (owner.has(name)) throw new Error(`shard: unit "${name}" is listed twice`)
    const lightest = load.indexOf(Math.min(...load))
    owner.set(name, lightest)
    load[lightest] += weight
  }
  return owner
}

/** The units of the share `index` (1-based) out of `total`, in their original order. */
export function takeShare(units: Unit[], [index, total]: Share): Unit[] {
  const owner = assign(units, total)
  return units.filter(([name]) => owner.get(name) === index - 1)
}

/** The CI share first, then the jobs share of what is left: the names this process runs, in list order. */
export function plan(units: Unit[], ci: Share, jobs: Share): string[] {
  return takeShare(takeShare(units, ci), jobs).map(([name]) => name)
}

export function median(values: number[]) {
  if (!values.length) return 1
  const sorted = [...values].sort((left, right) => left - right)
  const middle = sorted.length >> 1
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2
}

/** Names to units: the weight of a name is in `weights`, or the median of the file. */
export function weigh(names: string[], weights: Record<string, number>): Unit[] {
  const fallback = median(Object.values(weights))
  return names.map((name): Unit => [name, weights[name] ?? fallback])
}

export function loadWeights(check: string): Record<string, number> {
  const file = join(ROOT, 'tests/e2e', check, 'weights.json')
  return existsSync(file) ? JSON.parse(readFileSync(file, 'utf8')) : {}
}

/**
 * "Does this process run that unit" for the whole list of a check. Shares come from the environment
 * (see the top); the list is planned once.
 */
export function shardOwner(check: string, names: string[]) {
  const ci = parseShare(process.env.E2E_SHARD_CI, 'E2E_SHARD_CI')
  const jobs = parseShare(process.env.E2E_SHARD, 'E2E_SHARD')
  const mine = new Set(plan(weigh(names, loadWeights(check)), ci, jobs))
  return (name: string) => {
    if (!names.includes(name)) throw new Error(`shard: unit "${name}" is not in the plan`)
    return mine.has(name)
  }
}

/** True when this process sees only a part of the work. */
export const isPartial = () => {
  const [, ci] = parseShare(process.env.E2E_SHARD_CI, 'E2E_SHARD_CI')
  const [, jobs] = parseShare(process.env.E2E_SHARD, 'E2E_SHARD')
  return ci > 1 || jobs > 1
}

// Timings: seconds per unit, written by the process at the end, merged by the runner into
// .cache/check-<check>/timings.json (and from there into weights.json by --update-weights).
const seen = new Map<string, number>()

/** Runs `work` and remembers how long it took under `name`. */
export async function timed<T>(name: string, work: () => Promise<T>): Promise<T> {
  const began = Date.now()
  try {
    return await work()
  } finally {
    seen.set(name, (seen.get(name) ?? 0) + (Date.now() - began) / 1000)
  }
}

export function noteTiming(name: string, seconds: number) {
  seen.set(name, (seen.get(name) ?? 0) + seconds)
}

/** One file per process: .cache/check-<check>/timings/<K>-<i>.json. */
export function flushTimings(check: string) {
  const dir = join(ROOT, '.cache', `check-${check}`, 'timings')
  mkdirSync(dir, { recursive: true })
  const tag = `${(process.env.E2E_SHARD_CI ?? '1/1').split('/')[0]}-${(process.env.E2E_SHARD ?? '1/1').split('/')[0]}`
  writeFileSync(join(dir, `${tag}.json`), `${JSON.stringify(Object.fromEntries(seen), null, 2)}\n`)
}
