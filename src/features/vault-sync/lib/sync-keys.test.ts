import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

import { ALL_COLLECTIONS, KEYS_OF, keysOfBatch } from './sync-keys'

const registry = readFileSync(resolve(process.cwd(), 'crates/core/src/sync/registry.rs'), 'utf8')
const names = new Set(
  [...registry.matchAll(/(?:records|single)\("([a-z_]+)"|name:\s*"([a-z_]+)"/g)].map((match) => match[1] ?? match[2]),
)

const summaryOf = (...collections: string[]) =>
  ({
    total: collections.length,
    collections: collections.map((collection) => ({ collection, count: 1 })),
  }) as unknown as SyncSummary

describe('sync keys', () => {
  it('reads the registry', () => {
    expect(names.size).toBeGreaterThan(30)
    expect(names.has('stores')).toBe(true)
  })

  it('every synced collection invalidates queries', () => {
    const missing = [...names].filter((name) => name !== 'settings' && !(name in KEYS_OF) && !ALL_COLLECTIONS.has(name))
    expect(missing).toEqual([])
  })

  it('a settings batch invalidates what depends on settings', () => {
    const keys = keysOfBatch(summaryOf('settings'))
    expect(keys).not.toBe('all')
    expect(keys).toEqual(expect.arrayContaining([['dashboard-today'], ['month-summary'], ['goals']]))
  })

  it('a ledger batch invalidates everything', () => {
    expect(keysOfBatch(summaryOf('contacts', 'transactions'))).toBe('all')
  })
})
