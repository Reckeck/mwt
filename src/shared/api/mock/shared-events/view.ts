/** What the screens read of an event: the view, the list, the contact blocks, the chat text. */

import { txId } from '@/shared/lib/transactions'
import { mockContactBalances } from '../ledger'
import { mockDebtTerms } from '../catalog'
import { ME, type Split, computeSplit, eventStatus, fromCents } from './engine'
import { mockSharedEvents } from './state'

const nameOf = (event: SharedEvent, id: string) =>
  id === ME ? '' : (event.participants.find((person) => person.id === id)?.name ?? '')

function termOf(event: SharedEvent, person: Participant | undefined): string | null {
  if (!person?.contact_id) return null
  const asset = event.currency.toUpperCase()
  return mockDebtTerms.find((terms) => terms.contact_id === person.contact_id && terms.asset === asset)?.due_at ?? null
}

function minePosition(event: SharedEvent, split: Split): MyPosition | null {
  const mine = split.people.find((row) => row.id === ME)
  if (!event.i_participate || !mine) return null
  let owed = 0
  let owe = 0
  let returned = 0
  let off = 0
  if (event.organizer === ME) {
    for (const row of split.people.filter((entry) => entry.id !== ME)) {
      if (row.saldo > 0) owed += row.saldo
      else owe -= row.saldo
      returned += row.moved
      off += row.written_off
    }
  } else {
    if (mine.saldo > 0) owe = mine.saldo
    else owed = -mine.saldo
    returned = -mine.moved
  }
  return {
    paid: fromCents(mine.paid),
    share: fromCents(mine.share),
    owed_to_me: fromCents(owed),
    returned: fromCents(returned),
    i_owe: fromCents(owe),
    written_off: fromCents(off),
    over_share: fromCents(owed - owe + off),
    pay_to: event.organizer !== ME && mine.saldo > 0 ? event.organizer : null,
  }
}

/** Who pays whom through the organizer. */
function paymentPlan(event: SharedEvent, split: Split): PlannedPayment[] {
  return split.people
    .filter((row) => row.id !== event.organizer && Math.abs(row.saldo) >= 1)
    .map((row) =>
      row.saldo > 0
        ? { from: row.id, to: event.organizer, amount: fromCents(row.saldo) }
        : { from: event.organizer, to: row.id, amount: fromCents(-row.saldo) },
    )
}

export function eventView(event: SharedEvent, txExists: (id: string) => boolean): SharedEventView {
  const split = computeSplit(event)
  const people: PersonView[] = split.people.map((row) => {
    const person = event.participants.find((entry) => entry.id === row.id)
    return {
      id: row.id,
      name: nameOf(event, row.id),
      kind: !person ? 'me' : person.contact_id ? 'contact' : 'guest',
      contact_id: person?.contact_id ?? null,
      has_details: Boolean(
        person?.details && (person.details.phone || person.details.messenger || person.details.note),
      ),
      is_organizer: row.id === event.organizer,
      share: fromCents(row.share),
      paid: fromCents(row.paid),
      moved: fromCents(row.moved),
      written_off: fromCents(row.written_off),
      saldo: fromCents(row.saldo),
      due_at: termOf(event, person),
    }
  })
  const items: ItemView[] = split.items.map((row) => {
    const bought = row.cost > 0 || row.qty > 0
    return {
      item_id: row.item_id,
      cost: fromCents(row.cost),
      qty: String(row.qty),
      plan_total: row.plan_total === null ? null : fromCents(row.plan_total),
      plan_qty: row.plan_qty === null ? null : String(row.plan_qty),
      diff_money: fromCents(row.plan_total !== null && bought ? row.cost - row.plan_total : 0),
      diff_qty: String(row.plan_qty !== null && bought ? row.qty - row.plan_qty : 0),
      bought,
      shares: Object.entries(row.shares).map(([participant_id, amount]) => ({
        participant_id,
        amount: fromCents(amount),
      })),
    }
  })
  const txOf = (purchaseId: string) =>
    event.items.flatMap((item) => item.purchases).find((purchase) => purchase.id === purchaseId)?.tx_id
  return {
    event: structuredClone(event),
    status: eventStatus(event, split),
    totals: {
      planned: fromCents(split.planned),
      spent: fromCents(split.spent),
      diff: fromCents(split.spent - split.planned),
    },
    people,
    items,
    purchases: split.purchases.map((line) => {
      const tx = txOf(line.purchase_id)
      return {
        item_id: line.item_id,
        purchase_id: line.purchase_id,
        event_amount: fromCents(line.amount),
        my_part: fromCents(line.my_part),
        payment_missing: Boolean(tx) && !txExists(tx as string),
      }
    }),
    settlements: event.settlements.map((settlement) => ({
      settlement_id: settlement.id,
      payment_missing: Boolean(settlement.tx_id) && !txExists(settlement.tx_id as string),
    })),
    plan: paymentPlan(event, split),
    mine: minePosition(event, split),
  }
}

export function eventSummary(event: SharedEvent): SharedEventSummary {
  const split = computeSplit(event)
  const mine = minePosition(event, split)
  return {
    id: event.id,
    title: event.title,
    starts_on: event.starts_on,
    ends_on: event.ends_on,
    currency: event.currency,
    status: eventStatus(event, split),
    archived: event.archived,
    i_participate: event.i_participate,
    people_count: split.people.length,
    organizer_name: nameOf(event, event.organizer),
    organizer_is_me: event.organizer === ME,
    planned: fromCents(split.planned),
    spent: fromCents(split.spent),
    owed_to_me: mine?.owed_to_me ?? '0',
    i_owe: mine?.i_owe ?? '0',
  }
}

const sortKey = (event: SharedEvent) => event.starts_on ?? event.created_at.slice(0, 10)

export function listSummaries(includeArchived: boolean): SharedEventSummary[] {
  return mockSharedEvents
    .filter((event) => includeArchived || !event.archived)
    .sort(
      (left, right) => sortKey(right).localeCompare(sortKey(left)) || right.created_at.localeCompare(left.created_at),
    )
    .map(eventSummary)
}

/** Debts with contacts that involve me: positive means the contact owes me (cents). */
function eventDebts(event: SharedEvent): Array<[string, number]> {
  if (!event.i_participate) return []
  const split = computeSplit(event)
  const out: Array<[string, number]> = []
  if (event.organizer === ME) {
    for (const row of split.people) {
      const contact = event.participants.find((person) => person.id === row.id)?.contact_id
      if (contact) out.push([contact, row.saldo])
    }
  } else {
    const contact = event.participants.find((person) => person.id === event.organizer)?.contact_id
    const mine = split.people.find((row) => row.id === ME)
    if (contact && mine) out.push([contact, -mine.saldo])
  }
  return out.filter(([, cents]) => Math.abs(cents) >= 1)
}

/** The event balances to add to `get_contact_balances`. */
export function sharedBalances(): ContactBalance[] {
  const rows: ContactBalance[] = []
  for (const event of mockSharedEvents) {
    const code = event.currency.toUpperCase()
    for (const [contact, cents] of eventDebts(event)) {
      const asset: AssetRef = {
        symbol: code,
        name: code,
        coingecko_id: null,
        chain: 'fiat',
        contract: null,
        quote: code,
      }
      let row = rows.find((entry) => entry.contact_id === contact && entry.asset.symbol === code)
      if (!row) {
        row = { contact_id: contact, asset, receivable: '0', payable: '0' }
        rows.push(row)
      }
      const key = cents > 0 ? 'receivable' : 'payable'
      row[key] = fromCents(toNumber(row[key]) + Math.abs(cents))
    }
  }
  return rows
}

const toNumber = (raw: string) => Math.round(Number(raw) * 100)

export function contactEvents(contactId: string): ContactEventBalance[] {
  return mockSharedEvents.flatMap((event) =>
    eventDebts(event)
      .filter(([contact]) => contact === contactId)
      .map(([, cents]) => ({
        event_id: event.id,
        title: event.title,
        starts_on: event.starts_on,
        ends_on: event.ends_on,
        currency: event.currency,
        status: eventStatus(event, computeSplit(event)),
        amount: fromCents(cents),
        due_at: termOf(
          event,
          event.participants.find((person) => person.contact_id === contactId),
        ),
      })),
  )
}

export function txLinks(): SharedTxLink[] {
  return mockSharedEvents.flatMap((event) => [
    ...event.items.flatMap((item) =>
      item.purchases
        .filter((purchase) => purchase.tx_id)
        .map((purchase) => ({
          tx_id: purchase.tx_id as string,
          event_title: event.title,
          link: { event_id: event.id, purchase_id: purchase.id, settlement_id: null },
        })),
    ),
    ...event.settlements
      .filter((settlement) => settlement.tx_id)
      .map((settlement) => ({
        tx_id: settlement.tx_id as string,
        event_title: event.title,
        link: { event_id: event.id, purchase_id: null, settlement_id: settlement.id },
      })),
  ])
}

const NBSP = ' '
const money = (cents: number) => `${fromCents(cents).replace(/\B(?=(\d{3})+(?!\d))/g, NBSP)}${NBSP}₽`

/** A short chat text in the shape of the core one (ru only here). */
export function shareText(event: SharedEvent): string {
  const split = computeSplit(event)
  const label = (id: string, object = false) => (id === ME ? (object ? 'мне' : 'Я') : nameOf(event, id))
  const lines = [event.title, `Потрачено ${money(split.spent)} на ${split.people.length} человек`]
  lines.push(`Доли: ${split.people.map((row) => `${label(row.id)} ${money(row.share)}`).join(' · ')}`)
  const plan = paymentPlan(event, split)
  if (plan.length === 0) lines.push('Все рассчитались')
  else {
    lines.push('Кто кому:')
    for (const line of plan)
      lines.push(`• ${label(line.from)} → ${label(line.to, true)}: ${money(toNumber(line.amount))}`)
  }
  return lines.join('\n')
}

/** `get_contact_balances` with the open events added; the transfers an event made are already in it. */
export function contactBalancesWithEvents(txs: Transaction[]): ContactBalance[] {
  const linked = new Set(txLinks().map((link) => link.tx_id))
  const rows = mockContactBalances(txs.filter((tx) => !linked.has(txId(tx))))
  for (const extra of sharedBalances()) {
    const row = rows.find((entry) => entry.contact_id === extra.contact_id && entry.asset.symbol === extra.asset.symbol)
    if (!row) rows.push(extra)
    else {
      row.receivable = fromCents(toNumber(row.receivable) + toNumber(extra.receivable))
      row.payable = fromCents(toNumber(row.payable) + toNumber(extra.payable))
    }
  }
  return rows
}
