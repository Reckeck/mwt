import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

import en from '@/shared/config/i18n/locales/en.json'
import ru from '@/shared/config/i18n/locales/ru.json'

import { SYNC_COLLECTIONS, collectionKey, sortedChanges } from './collections'

/** Names of the collections in the Rust registry, read from its source. */
function registryNames(): string[] {
  const source = readFileSync(resolve(__dirname, '../../../../crates/core/src/sync/registry.rs'), 'utf8')
  const found = [...source.matchAll(/(?:records|single|log)\(\s*"([a-z_]+)"|\bname:\s*"([a-z_]+)"/g)]
  return found.map((match) => match[1] ?? match[2])
}

describe('collectionKey', () => {
  it('maps every collection of the registry to a named group', () => {
    const names = registryNames()
    expect(names.length).toBeGreaterThan(40)
    for (const name of names) {
      expect(SYNC_COLLECTIONS as readonly string[], `${name} is not in SYNC_COLLECTIONS`).toContain(name)
      expect(collectionKey(name)).toBe(`sync.coll.${name}`)
    }
  })

  it('has a Russian and an English name for each', () => {
    for (const name of SYNC_COLLECTIONS) {
      expect((en as Record<string, string>)[`sync.coll.${name}`], `en ${name}`).toBeTruthy()
      expect((ru as Record<string, string>)[`sync.coll.${name}`], `ru ${name}`).toBeTruthy()
    }
  })

  it('shows Other only for an unknown name', () => {
    expect(collectionKey('from_the_future')).toBe('sync.coll.other')
  })
})

describe('sortedChanges', () => {
  it('puts money first', () => {
    const group = (collection: string) => ({ collection, added: 1, updated: 0, deleted: 0, items: [] })
    const summary = {
      total: 3,
      collections: [group('settings'), group('stores'), group('transactions')],
      overwritten: [],
    }
    expect(sortedChanges(summary).map((entry) => entry.collection)).toEqual(['transactions', 'stores', 'settings'])
  })
})
