import { describe, expect, it } from 'vitest'

import { cryptoOpFilterCount, cryptoOpFilterFromQuery, cryptoOpFilterToQuery } from './crypto-op-filter'

describe('the operations filter in the address', () => {
  it('round-trips through the query, leaving empty fields out', () => {
    const filter = cryptoOpFilterFromQuery({
      chain: 'ethereum',
      token: 'USDT',
      kind: 'swap',
      hidden: '1',
      account: ['a1', 'a2'],
    })
    expect(filter).toMatchObject({
      chain: 'ethereum',
      token: 'USDT',
      kind: 'swap',
      show_hidden: true,
      account_ids: ['a1', 'a2'],
    })
    const query = cryptoOpFilterToQuery(filter)
    expect(query).toMatchObject({ chain: 'ethereum', token: 'USDT', kind: 'swap', hidden: '1', account: ['a1', 'a2'] })
    expect(Object.keys(cryptoOpFilterToQuery({ account_ids: [], show_hidden: false }))).toEqual(
      expect.arrayContaining(['chain']),
    )
    expect(cryptoOpFilterToQuery({ account_ids: [], show_hidden: false }).chain).toBeUndefined()
  })

  it('a wallet fixes its own account whatever the address says', () => {
    expect(cryptoOpFilterFromQuery({ account: 'other' }, ['mine']).account_ids).toEqual(['mine'])
    expect(cryptoOpFilterToQuery({ account_ids: ['mine'], show_hidden: false }, true).account).toBeUndefined()
  })

  it('counts the narrowing fields', () => {
    expect(cryptoOpFilterCount({ account_ids: [], show_hidden: false })).toBe(0)
    expect(cryptoOpFilterCount({ account_ids: ['x'], chain: 'base', address: '0x', show_hidden: true })).toBe(3)
  })
})
