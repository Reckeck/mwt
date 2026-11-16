import { describe, expect, it } from 'vitest'
import { createMemoryHistory, createRouter } from 'vue-router'

import { cryptoOpFilterFromQuery } from '@/entities/crypto-tx'

import { relationLocation, relationSections, relationsEmpty } from './relation-sections'

const ACCOUNT = 'acc-1'
const base: CryptoOpFilter = { account_ids: [ACCOUNT], show_hidden: false }

/** A group as the backend sends it: the items it shows and how many there are in all. */
function group<Item>(items: Item[], total = items.length) {
  return { items, total }
}

const data: CryptoRelations = {
  own_addresses: group(
    [
      {
        key: '0xaa',
        label: 'Savings',
        address: '0xaa',
        count: 2,
        last_time: '2026-11-12T10:00:00Z',
        filter: { ...base, address: '0xaa' },
      },
    ],
    3,
  ),
  exchanges: group([
    {
      key: '0xbb',
      label: 'Exchanger',
      address: '0xbb',
      count: 1,
      last_time: '2026-11-11T10:00:00Z',
      filter: { ...base, address: '0xbb' },
    },
  ]),
  protocols: group([
    {
      key: 'uniswap',
      label: 'uniswap',
      count: 3,
      last_time: '2026-11-10T10:00:00Z',
      filter: { ...base, protocol: 'uniswap' },
    },
  ]),
  fiat: group([]),
}

const router = createRouter({
  history: createMemoryHistory(),
  routes: [{ path: '/transactions/crypto', name: 'crypto-operations', component: { render: () => null } }],
})

describe('relation rows open the operations with their filter', () => {
  it('every row of every group leads to the screen with the filter it carries', () => {
    const sections = relationSections(data)
    expect(sections.map((section) => section.group)).toEqual(['own_addresses', 'exchanges', 'protocols'])
    for (const section of sections) {
      const source = data[section.group].items
      section.rows.forEach((row, index) => {
        const filter = source[index]?.filter
        const resolved = router.resolve(row.to)
        expect(resolved.name).toBe('crypto-operations')
        expect(cryptoOpFilterFromQuery(resolved.query)).toEqual({
          account_ids: filter?.account_ids,
          chain: undefined,
          token: undefined,
          protocol: filter?.protocol,
          kind: undefined,
          address: filter?.address,
          show_hidden: false,
        })
        expect(resolved.query.op).toBeUndefined()
      })
    }
  })

  it('a fiat record opens the card of its operation inside the account', () => {
    const resolved = router.resolve(relationLocation(base, 'ev-9'))
    expect(resolved.query.op).toBe('ev-9')
    expect(resolved.query.account).toEqual([ACCOUNT])
  })

  it('an answer with nothing in it is empty, and one with only fiat records is not', () => {
    const none: CryptoRelations = {
      own_addresses: group([]),
      exchanges: group([]),
      protocols: group([]),
      fiat: group([]),
    }
    expect(relationsEmpty(none)).toBe(true)
    expect(relationSections(none)).toEqual([])
    expect(relationsEmpty(undefined)).toBe(false)
    const fiatOnly: CryptoRelations = {
      ...none,
      fiat: group([
        {
          event_id: 'ev-9',
          filter: base,
          fiat: { id: 'tx', place: 'Bank', amount: '-5', currency: 'USD', time: '2026-11-11T09:00:00Z', comment: '' },
        },
      ]),
    }
    expect(relationsEmpty(fiatOnly)).toBe(false)
  })

  it('a group that was cut carries how many are left over', () => {
    const [own] = relationSections(data)
    expect(own?.rows).toHaveLength(1)
    expect(own?.total).toBe(3)
    expect((own?.total ?? 0) - (own?.rows.length ?? 0)).toBe(2)
  })
})
