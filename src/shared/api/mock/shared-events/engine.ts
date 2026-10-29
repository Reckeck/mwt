/**
 * The split of a shared event: the same rules, in the same order, as
 * `service/shared_events/split.rs`. Every sum is an exact integer (`dec.ts`).
 *
 * 1. each purchase is converted once at its own rate and rounded to the cent;
 * 2. an item's cost is spread to the cent, the cents left over go to the
 *    item's buyer (the first biggest purchase);
 * 3. hand-set totals fix their people and the rest is spread over the others
 *    (ignored, and flagged, when they no longer fit what was bought);
 * 4. each person's total is rounded once to the event's step; the leftover
 *    goes to the organizer if they take part, else the biggest payer, and a
 *    share never drops below zero;
 * 5. a rest put on the others moves between shares;
 * 6. balances go through the organizer.
 */

import { CENT, SCALE, abs, dec, divRound, mulRound2, round2, sum } from './dec'
import { applyOverrides, overridesFit, roundTotals, spread } from './rounding'

export { overridesFit }

export const ME = 'me'

/** Half a cent: how far a balance may be from zero and still be settled. */
export const SETTLE_TOLERANCE = 500_000n

export interface PersonRow {
  id: string
  share: bigint
  paid: bigint
  moved: bigint
  written_off: bigint
  saldo: bigint
}

export interface PurchaseLine {
  item_id: string
  purchase_id: string
  buyer: string
  at: string
  amount: bigint
  my_part: bigint
}

export interface ItemRow {
  item_id: string
  cost: bigint
  qty: bigint
  plan_total: bigint | null
  plan_qty: bigint | null
  shares: Record<string, bigint>
}

export interface Split {
  people: PersonRow[]
  items: ItemRow[]
  purchases: PurchaseLine[]
  spent: bigint
  planned: bigint
  overrides_invalid: boolean
}

const sameCode = (left: string, right: string) => left.toLowerCase() === right.toLowerCase()

/** What a purchase is worth in the event currency. */
export function eventAmount(event: SharedEvent, purchase: Purchase): bigint {
  const rate = sameCode(purchase.currency, event.currency) ? SCALE : dec(purchase.rate_to_event)
  return purchase.price_mode === 'per_unit'
    ? mulRound2(dec(purchase.qty), dec(purchase.amount), rate)
    : mulRound2(dec(purchase.amount), rate)
}

export function peopleIds(event: SharedEvent): string[] {
  const ids = event.i_participate || event.organizer === ME ? [ME] : []
  return [...ids, ...event.participants.map((person) => person.id)]
}

/** Who can share an item: everybody, but the owner only when they take part. */
export const sharerPool = (event: SharedEvent): string[] =>
  peopleIds(event).filter((id) => id !== ME || event.i_participate)

function weights(split: ItemSplit, pool: string[]): Array<[string, bigint]> {
  const everyone = (): Array<[string, bigint]> => pool.map((id) => [id, SCALE])
  if ('equal' in split) {
    const picked = pool.filter((id) => split.equal.among.includes(id)).map((id): [string, bigint] => [id, SCALE])
    return split.equal.among.length === 0 || picked.length === 0 ? everyone() : picked
  }
  const units = split.by_units.units
  const picked = pool.filter((id) => dec(units[id]) > 0n).map((id): [string, bigint] => [id, dec(units[id])])
  return picked.length === 0 ? everyone() : picked
}

/** Whether `participant` is the only person an item is split between. */
export function onlySharer(event: SharedEvent, item: EventItem, participant: string): boolean {
  const pool = sharerPool(event)
  const named =
    'equal' in item.split
      ? item.split.equal.among.filter((id) => pool.includes(id))
      : Object.entries(item.split.by_units.units)
          .filter(([id, qty]) => dec(qty) > 0n && pool.includes(id))
          .map(([id]) => id)
  if (named.length === 0) return pool.length === 1 && pool[0] === participant
  return named.every((id) => id === participant)
}

/** A rest put on the others: the debtor's share shrinks, theirs grow. */
function applyReassignments(event: SharedEvent, shares: Record<string, bigint>) {
  for (const settlement of event.settlements.filter((entry) => entry.kind === 'reassign')) {
    const targets = settlement.to.filter((id) => id !== settlement.participant_id)
    if (targets.length === 0) continue
    const amount = dec(settlement.amount)
    shares[settlement.participant_id] = (shares[settlement.participant_id] ?? 0n) - amount
    const each = divRound(amount, BigInt(targets.length) * CENT) * CENT
    for (const id of targets) shares[id] = (shares[id] ?? 0n) + each
    shares[targets[0]] += amount - each * BigInt(targets.length)
  }
}

/** My part of every purchase: item by item, scaled to my final share. */
function distributeMyPart(purchases: PurchaseLine[], items: ItemRow[], myShare: bigint) {
  if (purchases.length === 0 || myShare === 0n) return
  let weight = purchases.map((line) => {
    const item = items.find((row) => row.item_id === line.item_id)
    return item && item.cost > 0n ? divRound((item.shares[ME] ?? 0n) * line.amount, item.cost) : 0n
  })
  let total = sum(weight)
  if (total <= 0n) {
    weight = purchases.map((line) => line.amount)
    total = sum(weight)
    if (total <= 0n) return
  }
  let given = 0n
  purchases.forEach((line, index) => {
    line.my_part =
      index === purchases.length - 1 ? myShare - given : divRound(myShare * weight[index], total * CENT) * CENT
    given += line.my_part
  })
}

export function computeSplit(event: SharedEvent): Split {
  const pool = sharerPool(event)
  const shares: Record<string, bigint> = {}
  const paid: Record<string, bigint> = {}
  const items: ItemRow[] = []
  const purchases: PurchaseLine[] = []
  let spent = 0n
  let planned = 0n
  for (const item of event.items.filter((entry) => !entry.cancelled)) {
    const lines = item.purchases.map((purchase) => ({ purchase, amount: eventAmount(event, purchase) }))
    const cost = sum(lines.map((line) => line.amount))
    const planTotal = item.plan
      ? item.plan.price_mode === 'per_unit'
        ? mulRound2(dec(item.plan.qty), dec(item.plan.price))
        : round2(dec(item.plan.price))
      : null
    if (planTotal !== null) planned += planTotal
    let buyer = ''
    let biggest = 0n
    for (const { purchase, amount } of lines) {
      if (!buyer || amount > biggest) {
        buyer = purchase.buyer
        biggest = amount
      }
      paid[purchase.buyer] = (paid[purchase.buyer] ?? 0n) + amount
      purchases.push({
        item_id: item.id,
        purchase_id: purchase.id,
        buyer: purchase.buyer,
        at: purchase.at,
        amount,
        my_part: 0n,
      })
    }
    spent += cost
    const parts = spread(cost, weights(item.split, pool), buyer)
    for (const [id, part] of Object.entries(parts)) shares[id] = (shares[id] ?? 0n) + part
    items.push({
      item_id: item.id,
      cost,
      qty: sum(item.purchases.map((purchase) => dec(purchase.qty))),
      plan_total: planTotal,
      plan_qty: item.plan ? dec(item.plan.qty) : null,
      shares: parts,
    })
  }
  const { locked, invalid } = applyOverrides(event, pool, shares, paid, spent)
  roundTotals(event, pool, shares, paid, locked, spent)
  applyReassignments(event, shares)

  const moved: Record<string, bigint> = {}
  const writtenOff: Record<string, bigint> = {}
  for (const settlement of event.settlements) {
    const amount = dec(settlement.amount)
    if (settlement.kind === 'transfer' || settlement.kind === 'offset') {
      moved[settlement.participant_id] =
        (moved[settlement.participant_id] ?? 0n) + (settlement.direction === 'to_organizer' ? amount : -amount)
    } else if (settlement.kind === 'write_off') {
      writtenOff[settlement.participant_id] = (writtenOff[settlement.participant_id] ?? 0n) + amount
    }
  }
  const people: PersonRow[] = peopleIds(event).map((id) => ({
    id,
    share: shares[id] ?? 0n,
    paid: paid[id] ?? 0n,
    moved: moved[id] ?? 0n,
    written_off: writtenOff[id] ?? 0n,
    saldo: (shares[id] ?? 0n) - (paid[id] ?? 0n) - (moved[id] ?? 0n) - (writtenOff[id] ?? 0n),
  }))
  const organizer = people.find((row) => row.id === event.organizer)
  if (organizer) {
    organizer.saldo = -sum(people.filter((row) => row.id !== event.organizer).map((row) => row.saldo))
  }
  distributeMyPart(purchases, items, people.find((row) => row.id === ME)?.share ?? 0n)
  return { people, items, purchases, spent, planned, overrides_invalid: invalid }
}

export const isSettled = (saldo: bigint) => abs(saldo) < SETTLE_TOLERANCE

export function eventStatus(event: SharedEvent, split: Split): SharedEventStatus {
  const bought = event.items.some((item) => !item.cancelled && item.purchases.length > 0)
  if (!bought) return 'planning'
  if (split.people.every((row) => isSettled(row.saldo))) return 'closed'
  return event.settlements.length === 0 ? 'in_progress' : 'settling'
}
