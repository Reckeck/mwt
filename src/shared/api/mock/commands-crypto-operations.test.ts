import { describe, expect, it } from 'vitest'

import { cryptoOperationsCommands } from './commands-crypto-operations'

const call = (cmd: string, input: unknown) =>
  cryptoOperationsCommands[cmd]({ cmd, args: { input }, body: input as Record<string, unknown> })

describe('the operations mock', () => {
  it('has no command that creates or deletes an operation', () => {
    const names = Object.keys(cryptoOperationsCommands)
    expect(names.sort()).toEqual(['crypto_list_operations', 'crypto_operation_detail', 'crypto_set_override'])
    expect(names.filter((name) => /create|delete|remove|add|insert|import/.test(name))).toEqual([])
  })

  it('pages ten thousand operations by a hundred, newest first', async () => {
    const first = (await call('crypto_list_operations', {
      filter: { account_ids: [], show_hidden: false },
      offset: 0,
    })) as CryptoOpsPage
    expect(first.total).toBe(10_000)
    expect(first.items).toHaveLength(100)
    const dates = first.items.map((row) => row.time)
    expect([...dates].sort().reverse()).toEqual(dates)
    const last = (await call('crypto_list_operations', {
      filter: { account_ids: [], show_hidden: false },
      offset: 9_950,
      limit: 5_000,
    })) as CryptoOpsPage
    expect(last.items).toHaveLength(50)
  })

  it('the list names no explorer page, the card names an https one', async () => {
    const page = (await call('crypto_list_operations', {
      filter: { account_ids: [], show_hidden: false },
      offset: 0,
    })) as CryptoOpsPage
    expect(JSON.stringify(page)).not.toContain('http')
    const detail = (await call('crypto_operation_detail', {
      id: page.items[0].id,
      with_candidates: true,
    })) as CryptoOperationDetail
    expect(detail.url).toMatch(/^https:\/\//)
  })

  it('a correction changes classification, never an amount or the count', async () => {
    const before = (await call('crypto_list_operations', {
      filter: { account_ids: [], show_hidden: false },
      offset: 0,
    })) as CryptoOpsPage
    const target = before.items[1]
    await call('crypto_set_override', { event_id: target.id, kind: 'claim', comment: ' note ' })
    const after = (await call('crypto_list_operations', {
      filter: { account_ids: [], show_hidden: false },
      offset: 0,
    })) as CryptoOpsPage
    expect(after.total).toBe(before.total)
    const changed = after.items.find((row) => row.id === target.id)
    expect(changed?.kind).toBe('claim')
    expect(changed?.source_kind).toBe(target.source_kind)
    expect(changed?.comment).toBe('note')
    expect(changed?.legs).toEqual(target.legs)
  })

  it('a link is only made when asked for, and can be broken', async () => {
    const page = (await call('crypto_list_operations', {
      filter: { account_ids: [], show_hidden: false },
      offset: 0,
    })) as CryptoOpsPage
    const open = page.items.find(
      (row) => row.legs[0].symbol === 'USDT' && row.legs[0].direction === 'in' && !row.flow_id,
    )
    expect(open).toBeDefined()
    const result = (await call('crypto_set_override', {
      event_id: open?.id,
      link_transaction_id: 'mock-fiat-tinkoff',
    })) as CryptoSetOverrideResult
    expect(result.flow_id).toBeTruthy()
    const detail = (await call('crypto_operation_detail', {
      id: open?.id,
      with_candidates: false,
    })) as CryptoOperationDetail
    expect(detail.chain.map((member) => member.side).sort()).toEqual(['crypto', 'fiat'])
    await call('crypto_set_override', { event_id: open?.id, clear: ['flow'] })
    const broken = (await call('crypto_operation_detail', {
      id: open?.id,
      with_candidates: false,
    })) as CryptoOperationDetail
    expect(broken.chain).toEqual([])
  })
})
