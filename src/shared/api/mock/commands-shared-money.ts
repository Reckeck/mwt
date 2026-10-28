/** Commands: shared events, settling — money back, set-offs, write-offs, terms. */

import { txId } from '@/shared/lib/transactions'
import { mockDebtTerms } from './catalog'
import { mockPrices, mockTransactions } from './demo'
import { mockTransactionFrom } from './ledger'
import { resolve } from './core'
import type { MockCommands } from './core'
import { eventOrFail, viewOf } from './commands-shared-events'
import { ME, computeSplit, toCents } from './shared-events/engine'
import { fail } from './shared-events/state'

const bad = (message: string) => fail(message.startsWith('participant') ? 'MISSING' : 'INVALID_INPUT', message)

const moment = (raw: string | null | undefined) => {
  const text = raw?.trim()
  if (!text) return new Date().toISOString()
  return /^\d{4}-\d{2}-\d{2}$/.test(text) ? `${text}T12:00:00Z` : text
}

function dropTx(id: string | null) {
  const index = mockTransactions.findIndex((tx) => txId(tx) === id)
  if (index >= 0) mockTransactions.splice(index, 1)
}

export const sharedMoneyCommands: MockCommands = {
  settle_shared: ({ body }) => {
    const input = body as SettleSharedInput
    const event = eventOrFail(input.event_id)
    if (event instanceof Promise) return event
    const row = computeSplit(event).people.find((entry) => entry.id === input.participant_id)
    if (!row || input.participant_id === event.organizer) return bad('the organizer settles with the others')
    const owed = input.direction === 'to_organizer' ? row.saldo : -row.saldo
    if (!(Number(input.amount) > 0) || toCents(input.amount) > owed + 1)
      return bad('this is more than is owed in that direction')
    const settlement: Settlement = {
      id: crypto.randomUUID(),
      kind: 'offset',
      participant_id: input.participant_id,
      direction: input.direction,
      amount: input.amount,
      at: moment(input.at),
      tx_id: null,
      to: [],
    }
    if (input.store_id) {
      const other = event.organizer === ME ? input.participant_id : event.organizer
      const contact = event.participants.find((person) => person.id === other)?.contact_id
      if (!event.i_participate || (event.organizer !== ME && input.participant_id !== ME) || !contact) {
        return bad('settle without an operation: a guest has no contact, and others do not touch your wallets')
      }
      const receive = (event.organizer === ME) === (input.direction === 'to_organizer')
      const wallet = { store: input.store_id }
      const side = { contact: { contact_id: contact, account_id: null } }
      const tx = mockTransactionFrom(
        {
          type: 'transfer',
          common: {
            timestamp: settlement.at,
            comment: event.title,
            categories: [],
            flow_id: null,
            contact_id: null,
            price_usd: null,
            fx_override: null,
            price: null,
            price_currency: null,
          },
          from: receive ? side : wallet,
          to: receive ? wallet : side,
          asset: {
            symbol: event.currency.toUpperCase(),
            name: event.currency.toUpperCase(),
            coingecko_id: null,
            chain: 'fiat' as Chain,
            contract: null,
          },
          amount: input.amount,
          fee: null,
          network: null,
          tx_hash: null,
          debt: receive ? 'collect' : 'repay',
        },
        (symbol) => mockPrices[symbol.toUpperCase()] ?? 0,
      )
      mockTransactions.push(tx)
      settlement.kind = 'transfer'
      settlement.tx_id = txId(tx)
    }
    event.settlements.push(settlement)
    return viewOf(event)
  },
  write_off_shared: ({ body }) => remainder(body as SharedRemainderInput, 'write_off'),
  reassign_shared: ({ body }) => remainder(body as SharedRemainderInput, 'reassign'),
  delete_shared_settlement: ({ body }) => {
    const input = body as SharedSettlementRef
    const event = eventOrFail(input.event_id)
    if (event instanceof Promise) return event
    const settlement = event.settlements.find((entry) => entry.id === input.settlement_id)
    if (!settlement) return bad(`settlement ${input.settlement_id}`)
    dropTx(settlement.tx_id)
    event.settlements = event.settlements.filter((entry) => entry !== settlement)
    return viewOf(event)
  },
  set_shared_due: ({ body }) => {
    const input = body as SetSharedDueInput
    const event = eventOrFail(input.event_id)
    if (event instanceof Promise) return event
    const contact = event.participants.find((person) => person.id === input.participant_id)?.contact_id
    if (!contact) return bad('a term needs a participant who is a contact')
    const asset = event.currency.toUpperCase()
    const kept = mockDebtTerms.filter((entry) => !(entry.contact_id === contact && entry.asset === asset))
    mockDebtTerms.splice(0, mockDebtTerms.length, ...kept)
    if (!input.due_at) return resolve(null)
    const terms: DebtTerms = {
      id: crypto.randomUUID(),
      contact_id: contact,
      asset,
      due_at: input.due_at,
      note: '',
      created_at: new Date().toISOString(),
    }
    mockDebtTerms.push(terms)
    return resolve(terms)
  },
}

/** «Списать остаток» / «Переложить на других». */
function remainder(input: SharedRemainderInput, kind: 'write_off' | 'reassign') {
  const event = eventOrFail(input.event_id)
  if (event instanceof Promise) return event
  if (kind === 'write_off' && (event.organizer !== ME || !event.i_participate)) {
    return bad('only the organizer can write a debt off, and only when they take part')
  }
  const split = computeSplit(event)
  const row = split.people.find((entry) => entry.id === input.participant_id)
  if (!row || input.participant_id === event.organizer) return bad('the organizer settles with the others')
  if (row.saldo <= 0) return bad('nothing is owed here')
  const cents = input.amount ? toCents(input.amount) : row.saldo
  if (cents <= 0 || cents > row.saldo + 1) return bad('the amount must be positive and at most what is owed')
  const everyone = split.people.map((entry) => entry.id).filter((id) => id !== input.participant_id)
  event.settlements.push({
    id: crypto.randomUUID(),
    kind,
    participant_id: input.participant_id,
    direction: 'to_organizer',
    amount: String(Math.min(cents, row.saldo) / 100),
    at: new Date().toISOString(),
    tx_id: null,
    to: kind === 'reassign' ? (input.to.length ? input.to : everyone) : [],
  })
  return viewOf(event)
}
