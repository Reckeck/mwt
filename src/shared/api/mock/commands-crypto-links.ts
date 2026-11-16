/**
 * Commands: pointing a fiat record at a crypto operation, and back (spec E13).
 *
 * Same rules as the Rust service: the pointer is a visible reference only (it joins no chain and changes no
 * sum), both ends must exist when it is made, and an end that is gone later reads as absent, not as an error.
 * The pointer belongs to the operation, as `CryptoOverride::fiat_tx_id` does.
 */

import { txId } from '@/shared/lib/transactions'

import { type MockCommands, resolve } from './core'
import { cryptoOperationsCommands } from './commands-crypto-operations'
import { FIAT } from './crypto-operations-seed'
import { mockTransactions } from './demo'

/** Operation id -> fiat record id. */
const pairs = new Map<string, string>()
const DAY = 86_400_000
const WINDOW_DAYS = 3

const call = (cmd: string, body: Record<string, unknown>) => ({ cmd, args: { input: body }, body })

interface MockFiat {
  id: string
  place: string
  amount: string
  currency: string
  time: string
  comment: string
}

/** A seeded record, or one the user saved in this session. */
function fiatOf(id: string): MockFiat | undefined {
  const seeded = FIAT.find((entry) => entry.id === id)
  if (seeded) return seeded
  for (const tx of mockTransactions) {
    if (txId(tx) !== id) continue
    const body = Object.values(tx)[0] as {
      meta: { timestamp: string; comment: string }
      amount?: string
      asset?: { symbol: string }
    }
    return {
      id,
      place: '',
      amount: body.amount ?? '0',
      currency: body.asset?.symbol ?? '',
      time: body.meta.timestamp,
      comment: body.meta.comment,
    }
  }
  return undefined
}

async function rowOf(eventId: string): Promise<CryptoOpRow | undefined> {
  try {
    const detail = (await cryptoOperationsCommands.crypto_operation_detail(
      call('crypto_operation_detail', { id: eventId, with_candidates: false }),
    )) as CryptoOperationDetail
    return detail.row
  } catch {
    return undefined
  }
}

async function pairOf(eventId: string, fiatId: string): Promise<CryptoPair> {
  const event = await rowOf(eventId)
  const fiat = fiatOf(fiatId)
  return { event_id: eventId, fiat_tx_id: fiatId, event, fiat }
}

export const cryptoLinksCommands: MockCommands = {
  crypto_link_candidates: async ({ body }) => {
    const input = body as unknown as CryptoLinkCandidatesInput
    const page = (await cryptoOperationsCommands.crypto_list_operations(
      call('crypto_list_operations', { filter: { account_ids: [], show_hidden: false }, offset: 0, limit: 100 }),
    )) as CryptoOpsPage
    const at = Date.parse(input.time)
    const options: CryptoLinkOption[] = page.items
      .filter((row) => !row.own_entry)
      .map((row) => ({ row, days: Math.round(Math.abs(Date.parse(row.time) - at) / DAY), fits: false }))
      .filter((option) => option.days <= WINDOW_DAYS)
      .sort((left, right) => left.days - right.days || (left.row.time < right.row.time ? 1 : -1))
      .slice(0, 40)
    return resolve(options)
  },
  crypto_link_operation: async ({ body }) => {
    const input = body as unknown as CryptoLinkInput
    if (!(await rowOf(input.event_id)))
      return Promise.reject({ code: 'NOT_FOUND', message: `operation ${input.event_id}` })
    if (!fiatOf(input.fiat_tx_id))
      return Promise.reject({ code: 'NOT_FOUND', message: `transaction ${input.fiat_tx_id}` })
    pairs.set(input.event_id, input.fiat_tx_id)
    return resolve(undefined)
  },
  crypto_unlink_operation: ({ body }) => {
    pairs.delete((body as unknown as CryptoUnlinkInput).event_id)
    return resolve(undefined)
  },
  crypto_link_pairs: async ({ body }) => {
    const input = body as unknown as CryptoPairsInput
    const wanted = [...pairs].filter(
      ([eventId, fiatId]) =>
        (!input.event_id || input.event_id === eventId) && (!input.fiat_tx_id || input.fiat_tx_id === fiatId),
    )
    return resolve(await Promise.all(wanted.map(([eventId, fiatId]) => pairOf(eventId, fiatId))))
  },
}
