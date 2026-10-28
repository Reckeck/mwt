import { fileURLToPath } from 'node:url'

export const ROOT = fileURLToPath(new URL('../../..', import.meta.url))

/** `CHECK_UPDATE=1` (set by `scripts/run-check.mjs --update`): rewrite the baseline instead of judging. */
export const UPDATE = process.env.CHECK_UPDATE === '1'

/** Fail with one `file:line: text` per line, the way the old `check-*.mjs` scripts printed them. */
export function expectNone(lines: string[]) {
  if (lines.length) throw new Error(`\n${lines.map((line) => `error: ${line}`).join('\n')}`)
}
