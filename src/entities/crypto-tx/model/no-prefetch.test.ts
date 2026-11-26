import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

/** The slices of spec E10, read as text: what they may call, and what they may never do. */
const SLICES = ['src/entities/crypto-tx', 'src/features/crypto-tx-override']
const ALLOWED = new Set([
  'crypto_list_operations',
  'crypto_operation_detail',
  'crypto_set_override',
  'open_explorer',
  // Spec E13: a read of the pointers between an operation and a fiat record; it writes nothing.
  'crypto_link_pairs',
])

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    return statSync(path).isDirectory()
      ? files(path)
      : /\.(vue|ts)$/.test(name) && !name.endsWith('.test.ts')
        ? [path]
        : []
  })
}
const sources = SLICES.flatMap((slice) => files(join(process.cwd(), slice))).map(
  (path) => [path, readFileSync(path, 'utf8')] as const,
)

describe('the operations slices', () => {
  it('call only the three commands that read operations or correct them, and open_explorer', () => {
    const called = new Set(
      sources.flatMap(([, text]) => [...text.matchAll(/\bcall(?:<[^>]*>)?\(\s*'([a-z_]+)'/g)].map((match) => match[1])),
    )
    expect([...called].filter((name) => !ALLOWED.has(name))).toEqual([])
    expect(called.has('crypto_list_operations')).toBe(true)
  })

  it('reach the explorer only through open_explorer, in one file, never by a request, a link or an image', () => {
    const openers = sources.filter(([, text]) => text.includes('open_explorer')).map(([path]) => path)
    expect(openers).toHaveLength(1)
    expect(openers[0]).toMatch(/open-in-browser\.ts$/)
    for (const [path, text] of sources) {
      expect(text, path).not.toMatch(
        /\bfetch\(|XMLHttpRequest|window\.open|<a\s|<img|<iframe|<link\s|new Image|sendBeacon|:href=|\bhref=/,
      )
    }
  })

  it('has no create or delete action: no such command, no such label', () => {
    for (const [path, text] of sources) {
      expect(text, path).not.toMatch(/crypto_(create|delete|remove|add)_/)
    }
    const names = sources.flatMap(([, text]) => [...text.matchAll(/id: '([a-z-]+)'/g)].map((match) => match[1]))
    expect(names.filter((name) => /create|delete|remove|add/.test(name))).toEqual([])
  })
})
