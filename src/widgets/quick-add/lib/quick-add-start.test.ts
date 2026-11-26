import { beforeEach, describe, expect, it } from 'vitest'

import { rememberStore, startForm } from './quick-add-start'

const NOW = '2026-11-26T10:00'

describe('startForm', () => {
  beforeEach(() => localStorage.clear())

  it('starts on the first wallet, an expense, now', () => {
    expect(startForm(null, ['a', 'b'], NOW)).toMatchObject({
      kind: 'payment',
      storeId: 'a',
      timestamp: NOW,
      amount: '',
    })
  })

  it('prefers the remembered wallet, then the asked one', () => {
    rememberStore('b')
    expect(startForm(null, ['a', 'b'], NOW).storeId).toBe('b')
    expect(startForm({ storeId: 'a' }, ['a', 'b'], NOW).storeId).toBe('a')
  })

  it('drops a wallet that is gone and a kind the form does not have', () => {
    rememberStore('gone')
    expect(startForm({ kind: 'trade_spot', amount: '100' }, ['a'], NOW)).toMatchObject({
      storeId: 'a',
      kind: 'payment',
      amount: '100',
    })
  })

  it('opens on an exchange when asked', () => {
    expect(startForm({ kind: 'convert', amount: '100' }, ['a'], NOW)).toMatchObject({ kind: 'convert', amount: '100' })
  })

  it('takes the kind, category and destination of a request', () => {
    expect(startForm({ kind: 'transfer', toStoreId: 'b', category: 'food' }, ['a', 'b'], NOW)).toMatchObject({
      kind: 'transfer',
      toStoreId: 'b',
      category: 'food',
    })
  })
})
