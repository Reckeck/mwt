/**
 * Commands: the operations of the crypto module and the corrections over them (spec E10).
 *
 * The mock keeps the same rules as the Rust service: events are read-only (the list is built
 * from a fixed seed, nothing here adds or removes one), a correction changes classification and
 * attributes only, a page is 100 rows, and the detail names the explorer page (`https` only) while
 * the list does not. The seed holds 10 000 operations, so a screen built against it is tried on the
 * size the spec asks for.
 */

import { resolve } from './core'
import {
  ACCOUNT,
  CONTACTS,
  FIAT,
  type MockEvent,
  type MockFix,
  type MockRule,
  SENDER,
  USDT,
  seed,
} from './crypto-operations-seed'
import type { MockCommands } from './core'

const PAGE = 100

let events: MockEvent[] | null = null
const fixes = new Map<string, MockFix>()
/** A rule by contract: it outlives a refetch and classifies what arrives later, as in the vault. */
const rules = new Map<string, MockRule>()
const fiatFlows = new Map<string, string>()
const all = () => (events ??= seed())

function row(event: MockEvent): CryptoOpRow {
  const fix = fixes.get(event.id)
  const rule = rules.get(contractOf(event))
  const byRule = Boolean(rule && !event.own && (!fix?.kind || !fix.categories.length || !fix.contactId))
  const contactId = fix?.contactId ?? (event.own ? undefined : rule?.contactId)
  const guessed =
    !contactId &&
    event.from === SENDER &&
    event.direction === 'in' &&
    [...fixes.values()].some((other) => other.contactId)
  const contact = contactId ?? (guessed ? 'mock-contact-coinrush' : undefined)
  return {
    id: event.id,
    account_id: ACCOUNT,
    account_name: 'Основной',
    network: event.exchange ? undefined : 'ethereum',
    exchange: event.exchange,
    time: event.time,
    kind: fix?.kind ?? (event.own ? event.kind : (rule?.kind ?? event.kind)),
    source_kind: event.kind,
    legs: [{ direction: event.direction, asset_key: event.assetKey, symbol: event.symbol, amount: event.amount }],
    gas: event.gas ? { asset: { native: { chain: 'ethereum' } }, amount: event.gas } : undefined,
    gas_symbol: event.gas ? 'ETH' : undefined,
    protocol: event.protocol,
    from: event.from,
    to: event.to,
    reference: event.exchange ? event.hash : `${event.hash.slice(0, 6)}…${event.hash.slice(-4)}`,
    value_usd: event.symbol === 'USDT' ? `${event.direction === 'in' ? '' : '-'}${event.amount}` : undefined,
    flow_id: fix?.flowId,
    contact_id: contactId,
    contact_name: contact ? CONTACTS[contact] : undefined,
    contact_guessed: guessed,
    comment: fix?.comment,
    categories: fix?.categories?.length ? fix.categories : event.own ? [] : (rule?.categories ?? []),
    hidden: fix?.hidden ?? false,
    corrected: Boolean(fix),
    // An own entry has no hash, so there is no page to open for it.
    linkable: !event.own,
    own_entry: event.own,
    by_rule: byRule,
  }
}

function matches(filter: CryptoOpFilter, event: MockEvent): boolean {
  const fix = fixes.get(event.id)
  if (fix?.hidden && !filter.show_hidden) return false
  if (filter.chain && (event.exchange ? 'fiat' : 'ethereum') !== filter.chain) return false
  if (filter.kind && (fix?.kind ?? event.kind) !== filter.kind) return false
  if (filter.protocol && event.protocol !== filter.protocol) return false
  if (filter.token && event.assetKey !== filter.token && event.symbol.toLowerCase() !== filter.token.toLowerCase())
    return false
  if (filter.address) {
    const needle = filter.address.trim().toLowerCase()
    if (needle && !event.from.toLowerCase().includes(needle) && !event.to.toLowerCase().includes(needle)) return false
  }
  return true
}

function candidates(event: MockEvent): CryptoLinkCandidate[] {
  if (event.direction !== 'in') return []
  return FIAT.map((fiat, index) => ({
    transaction_id: fiat.id,
    place: fiat.place,
    amount: fiat.amount,
    currency: fiat.currency,
    time: fiat.time,
    comment: fiat.comment,
    reasons:
      index === 0
        ? (['same_day', 'amount_fits'] as CandidateReason[])
        : index === 2
          ? ([{ near_date: { days: 1 } }, 'amount_fits'] as CandidateReason[])
          : (['same_day'] as CandidateReason[]),
    rate: index === 1 ? undefined : '100',
    rate_symbol: event.symbol,
  }))
}

function chainOf(flow: string | undefined, current: string): CryptoChainMember[] {
  if (!flow) return []
  const members: CryptoChainMember[] = []
  for (const [fiatId, id] of fiatFlows) {
    const fiat = FIAT.find((entry) => entry.id === fiatId)
    if (fiat && id === flow)
      members.push({
        side: 'fiat',
        id: fiat.id,
        place: fiat.place,
        amount: fiat.amount,
        currency: fiat.currency,
        time: fiat.time,
        is_this: false,
      })
  }
  for (const [eventId, fix] of fixes) {
    const event = all().find((entry) => entry.id === eventId)
    if (event && fix.flowId === flow) {
      members.push({
        side: 'crypto',
        id: event.id,
        place: 'Основной',
        amount: event.amount,
        currency: event.symbol,
        time: event.time,
        is_this: event.id === current,
      })
    }
  }
  return members
}

const contractOf = (event: MockEvent) => (event.symbol === 'USDT' ? USDT : event.to)

export const cryptoOperationsCommands: MockCommands = {
  crypto_list_operations: (call) => {
    const input = (call.body ?? {}) as unknown as CryptoListOperationsInput
    const filter = input.filter ?? { account_ids: [], show_hidden: false }
    const pool = all().filter((event) => !fixes.get(event.id)?.hidden || filter.show_hidden)
    const matched = all()
      .filter((event) => matches(filter, event))
      .sort((left, right) => (left.time < right.time ? 1 : left.time > right.time ? -1 : 0))
    const limit = Math.min(input.limit ?? PAGE, PAGE)
    const items = matched.slice(input.offset ?? 0, (input.offset ?? 0) + limit).map(row)
    const tokens = new Map<string, string>()
    for (const event of pool) tokens.set(event.assetKey, event.symbol)
    const first = (input.offset ?? 0) === 0
    const open = first
      ? matched
          .slice(0, 40)
          .find(
            (event) =>
              event.direction === 'in' &&
              !fixes.get(event.id)?.flowId &&
              !fixes.get(event.id)?.notExchange &&
              event.symbol === 'USDT',
          )
      : undefined
    const top = open ? candidates(open).find((candidate) => candidate.reasons.includes('amount_fits')) : undefined
    return resolve<CryptoOpsPage>({
      items,
      total: matched.length,
      facets: {
        chains: ['ethereum'],
        protocols: ['uniswap'],
        tokens: [...tokens].map(([key, symbol]) => ({ key, symbol })),
      },
      suggestion:
        open && top ? { event_id: open.id, symbol: open.symbol, amount: open.amount, candidate: top } : undefined,
    })
  },

  crypto_operation_detail: (call) => {
    const input = call.body as unknown as CryptoOperationDetailInput
    const event = all().find((entry) => entry.id === input.id)
    if (!event) return Promise.reject({ code: 'NOT_FOUND', message: `operation ${input.id}` })
    const summary = row(event)
    const chain = chainOf(summary.flow_id, event.id)
    const link = event.exchange ? 'https://www.bybit.com/' : `https://etherscan.io/tx/${event.hash}`
    return resolve<CryptoOperationDetail>({
      row: summary,
      url: link,
      url_note: event.exchange ? event.hash : undefined,
      method: undefined,
      block: 21_000_000n,
      tracking: event.exchange
        ? { exchange_history: { exchange: event.exchange, exec_id: event.hash } }
        : { chain_tx: { chain: 'ethereum', hash: event.hash } },
      chain,
      basis_from_chain:
        chain.filter((member) => member.side === 'fiat').length === 1 &&
        chain.filter((member) => member.side === 'crypto').length === 1,
      same_contract_count: all().filter((other) => contractOf(other) === contractOf(event)).length,
      contract_rule: rules.get(contractOf(event)),
      contract: contractOf(event) || undefined,
      candidates: input.with_candidates ? candidates(event) : [],
    })
  },

  crypto_set_override: (call) => {
    const input = call.body as unknown as CryptoSetOverrideInput
    const event = all().find((entry) => entry.id === input.event_id)
    if (!event) return Promise.reject({ code: 'NOT_FOUND', message: `operation ${input.event_id}` })
    const contract = contractOf(event)
    if (input.remove_contract_rule) rules.delete(contract)
    if (input.same_contract) {
      const rule: MockRule = rules.get(contract) ?? { categories: [] }
      for (const field of input.clear ?? []) {
        if (field === 'kind') rule.kind = undefined
        if (field === 'categories') rule.categories = []
        if (field === 'contact') rule.contactId = undefined
      }
      if (input.kind) rule.kind = input.kind
      if (input.categories) rule.categories = input.categories
      if (input.contact_id) rule.contactId = input.contact_id
      if (rule.kind || rule.categories.length || rule.contactId) rules.set(contract, rule)
      else rules.delete(contract)
    }
    const reaches = all().filter((other) => contractOf(other) === contract).length
    const targets = [event]
    let flow: string | undefined
    for (const target of targets) {
      const own = target.id === event.id
      const fix = fixes.get(target.id) ?? { categories: [], hidden: false }
      for (const field of input.clear ?? []) {
        if (field === 'kind') fix.kind = undefined
        if (field === 'categories') fix.categories = []
        if (field === 'contact') fix.contactId = undefined
        if (own && field === 'comment') fix.comment = undefined
        if (own && field === 'flow') {
          for (const [fiatId, id] of fiatFlows) if (id === fix.flowId) fiatFlows.delete(fiatId)
          fix.flowId = undefined
        }
      }
      if (!input.same_contract) {
        if (input.kind) fix.kind = input.kind
        if (input.categories) fix.categories = input.categories
        if (input.contact_id) fix.contactId = input.contact_id
      }
      if (own) {
        if (input.not_exchange !== undefined) fix.notExchange = input.not_exchange
        if (input.comment !== undefined) fix.comment = input.comment.trim() || undefined
        if (input.hidden !== undefined) fix.hidden = input.hidden
        if (input.link_transaction_id) {
          fix.flowId ??= fiatFlows.get(input.link_transaction_id) ?? `flow-${event.id}`
          fiatFlows.set(input.link_transaction_id, fix.flowId)
        }
        flow = fix.flowId
      }
      fixes.set(target.id, fix)
    }
    return resolve<CryptoSetOverrideResult>({
      flow_id: flow,
      applied: input.same_contract ? reaches : targets.length,
    })
  },
}
