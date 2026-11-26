import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

/**
 * The slices that show the crypto module (spec E10), read as text: what they may call, and what they may never
 * do. The module reads the chain; the screens never create, delete or fetch by themselves.
 */
const SLICES = [
  'src/entities/crypto-tx',
  'src/entities/crypto-account',
  'src/entities/crypto-token',
  'src/entities/crypto-position',
  'src/pages/crypto-positions',
  'src/widgets/price-chart',
]
const ALLOWED = new Set([
  'crypto_list_accounts',
  'crypto_sync_status',
  'crypto_list_tokens',
  'crypto_list_positions',
  // The user's verdict on a quarantined token: it decides whether the token counts, it adds or drops no operation.
  'crypto_confirm_token',
  'crypto_reject_token',
  'crypto_clear_token_verdict',
  // Market data, read only.
  'get_candles',
])

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    return statSync(path).isDirectory()
      ? files(path)
      : /\.(vue|ts)$/.test(name) && !/\.test\.ts$/.test(name)
        ? [path]
        : []
  })
}
const sources = SLICES.flatMap((slice) => files(join(process.cwd(), slice))).map(
  (path) => [path, readFileSync(path, 'utf8')] as const,
)

describe('the crypto slices', () => {
  it('call only reads, token verdicts and market data', () => {
    const called = new Set(
      sources.flatMap(([, text]) => [...text.matchAll(/\bcall(?:<[^>]*>)?\(\s*'([a-z_]+)'/g)].map((match) => match[1])),
    )
    expect([...called].filter((name) => !ALLOWED.has(name))).toEqual([])
    expect(called.has('crypto_list_positions')).toBe(true)
  })

  it('reach no network by themselves: no request, link, image or frame', () => {
    for (const [path, text] of sources) {
      expect(text, path).not.toMatch(
        /\bfetch\(|XMLHttpRequest|window\.open|<a\s|<img|<iframe|<link\s|new Image|sendBeacon|:href=|\bhref=/,
      )
    }
  })

  it('have no create or delete action: no such command, no such label', () => {
    for (const [path, text] of sources) {
      expect(text, path).not.toMatch(/crypto_(create|delete|remove|add|connect|wipe)_?/)
    }
  })
})
