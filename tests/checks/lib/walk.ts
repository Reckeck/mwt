import { readdirSync } from 'node:fs'
import { join } from 'node:path'

/** Every file under `dir` whose name ends with `ext` (one extension, or a test on the name). */
export function walk(dir: string, match: string | RegExp, out: string[] = []): string[] {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) walk(path, match, out)
    else if (typeof match === 'string' ? entry.name.endsWith(match) : match.test(entry.name)) out.push(path)
  }
  return out
}

export const lineOf = (text: string, index: number) => text.slice(0, index).split('\n').length
