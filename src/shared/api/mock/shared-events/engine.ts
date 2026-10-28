/**
 * The split of a shared event, in cents: the same rules as
 * `service/shared_events/split.rs` — each purchase converted once at its own
 * rate, shares rounded to the event's step with the leftover to the item's
 * buyer, hand-set totals, rests put on the others, then balances through the
 * organizer. The mock follows the rules only as far as the screens need.
 */

export const ME = 'me'

export const toCents = (raw: string | number): number => Math.round(Number(raw) * 100)

/** `1735`, `1231.25`: cents back to an amount string without needless zeros. */
export function fromCents(value: number): string {
  const text = (value / 100).toFixed(2)
  return text.replace(/\.00$/, '').replace(/(\.\d)0$/, '$1')
}

/** Round half away from zero. */
const roundAway = (value: number): number => Math.sign(value) * Math.round(Math.abs(value))

export interface PersonRow {
  id: string
  share: number
  paid: number
  moved: number
  written_off: number
  saldo: number
}

export interface PurchaseLine {
  item_id: string
  purchase_id: string
  buyer: string
  at: string
  amount: number
  my_part: number
}

export interface ItemRow {
  item_id: string
  cost: number
  qty: number
  plan_total: number | null
  plan_qty: number | null
  shares: Record<string, number>
}

export interface Split {
  people: PersonRow[]
  items: ItemRow[]
  purchases: PurchaseLine[]
  spent: number
  planned: number
}

export function purchaseTotal(purchase: Purchase): number {
  return purchase.price_mode === 'per_unit' ? Number(purchase.qty) * Number(purchase.amount) : Number(purchase.amount)
}

/** What a purchase is worth in the event currency, in cents. */
export function eventAmount(event: SharedEvent, purchase: Purchase): number {
  const rate = purchase.currency === event.currency ? 1 : Number(purchase.rate_to_event)
  return toCents(purchaseTotal(purchase) * rate)
}

export function peopleIds(event: SharedEvent): string[] {
  const ids = event.i_participate || event.organizer === ME ? [ME] : []
  return [...ids, ...event.participants.map((person) => person.id)]
}

function weights(split: ItemSplit, pool: string[]): Array<[string, number]> {
  const everyone = (): Array<[string, number]> => pool.map((id) => [id, 1])
  if ('equal' in split) {
    const picked = pool.filter((id) => split.equal.among.includes(id)).map((id): [string, number] => [id, 1])
    return split.equal.among.length === 0 || picked.length === 0 ? everyone() : picked
  }
  const units = split.by_units.units
  const picked = pool.filter((id) => Number(units[id]) > 0).map((id): [string, number] => [id, Number(units[id])])
  return picked.length === 0 ? everyone() : picked
}

/** Spread `cost` over the weights, each part rounded to `step`; the leftover goes to `taker`. */
function spread(cost: number, parts: Array<[string, number]>, step: number, taker: string): Record<string, number> {
  const out: Record<string, number> = {}
  const total = parts.reduce((sum, [, weight]) => sum + weight, 0)
  if (parts.length === 0 || total <= 0) return out
  let given = 0
  for (const [id, weight] of parts) {
    const part = roundAway((cost * weight) / total / step) * step
    out[id] = part
    given += part
  }
  const receiver = taker in out ? taker : parts[0][0]
  out[receiver] += cost - given
  return out
}

function applyOverrides(
  event: SharedEvent,
  pool: string[],
  shares: Record<string, number>,
  paid: Record<string, number>,
  spent: number,
) {
  const fixed = event.adjustments.filter((fix) => pool.includes(fix.participant_id))
  if (fixed.length === 0) return
  const fixedIds = fixed.map((fix) => fix.participant_id)
  const others = pool.filter((id) => !fixedIds.includes(id))
  const rest = spent - fixed.reduce((sum, fix) => sum + toCents(fix.total), 0)
  for (const fix of fixed) shares[fix.participant_id] = toCents(fix.total)
  if (others.length === 0) return
  const base = others.reduce((sum, id) => sum + (shares[id] ?? 0), 0)
  const parts = others.map((id): [string, number] => [id, base > 0 ? (shares[id] ?? 0) : 1])
  const taker = [...others].sort((left, right) => (paid[right] ?? 0) - (paid[left] ?? 0))[0]
  const spreadRest = spread(rest, parts, toCents(event.rounding_step), taker)
  for (const id of others) shares[id] = spreadRest[id] ?? 0
}

function applyReassignments(event: SharedEvent, shares: Record<string, number>) {
  for (const settlement of event.settlements.filter((entry) => entry.kind === 'reassign')) {
    const targets = settlement.to.filter((id) => id !== settlement.participant_id)
    if (targets.length === 0) continue
    const amount = toCents(settlement.amount)
    shares[settlement.participant_id] = (shares[settlement.participant_id] ?? 0) - amount
    const each = Math.round(amount / targets.length)
    for (const id of targets) shares[id] = (shares[id] ?? 0) + each
    shares[targets[0]] += amount - each * targets.length
  }
}

/** My part of every purchase: item by item, scaled to my final share. */
function distributeMyPart(purchases: PurchaseLine[], items: ItemRow[], myShare: number) {
  if (purchases.length === 0 || myShare === 0) return
  let weight = purchases.map((line) => {
    const item = items.find((row) => row.item_id === line.item_id)
    return item && item.cost > 0 ? ((item.shares[ME] ?? 0) * line.amount) / item.cost : 0
  })
  let total = weight.reduce((sum, value) => sum + value, 0)
  if (total <= 0) {
    weight = purchases.map((line) => line.amount)
    total = weight.reduce((sum, value) => sum + value, 0)
    if (total <= 0) return
  }
  let given = 0
  purchases.forEach((line, index) => {
    line.my_part = index === purchases.length - 1 ? myShare - given : Math.round((myShare * weight[index]) / total)
    given += line.my_part
  })
}

export function computeSplit(event: SharedEvent): Split {
  const pool = peopleIds(event).filter((id) => id !== ME || event.i_participate)
  const step = toCents(event.rounding_step)
  const shares: Record<string, number> = {}
  const paid: Record<string, number> = {}
  const items: ItemRow[] = []
  const purchases: PurchaseLine[] = []
  let spent = 0
  let planned = 0
  for (const item of event.items.filter((entry) => !entry.cancelled)) {
    const lines = item.purchases.map((purchase) => ({ purchase, amount: eventAmount(event, purchase) }))
    const cost = lines.reduce((sum, line) => sum + line.amount, 0)
    const planTotal = item.plan
      ? toCents(item.plan.price_mode === 'per_unit' ? Number(item.plan.qty) * Number(item.plan.price) : item.plan.price)
      : null
    if (planTotal !== null) planned += planTotal
    let buyer = ''
    let biggest = 0
    for (const { purchase, amount } of lines) {
      if (!buyer || amount > biggest) {
        buyer = purchase.buyer
        biggest = amount
      }
      paid[purchase.buyer] = (paid[purchase.buyer] ?? 0) + amount
      purchases.push({
        item_id: item.id,
        purchase_id: purchase.id,
        buyer: purchase.buyer,
        at: purchase.at,
        amount,
        my_part: 0,
      })
    }
    spent += cost
    const parts = spread(cost, weights(item.split, pool), step, buyer)
    for (const [id, part] of Object.entries(parts)) shares[id] = (shares[id] ?? 0) + part
    items.push({
      item_id: item.id,
      cost,
      qty: item.purchases.reduce((sum, purchase) => sum + Number(purchase.qty), 0),
      plan_total: planTotal,
      plan_qty: item.plan ? Number(item.plan.qty) : null,
      shares: parts,
    })
  }
  applyOverrides(event, pool, shares, paid, spent)
  applyReassignments(event, shares)

  const moved: Record<string, number> = {}
  const writtenOff: Record<string, number> = {}
  for (const settlement of event.settlements) {
    const amount = toCents(settlement.amount)
    if (settlement.kind === 'transfer' || settlement.kind === 'offset') {
      moved[settlement.participant_id] =
        (moved[settlement.participant_id] ?? 0) + (settlement.direction === 'to_organizer' ? amount : -amount)
    } else if (settlement.kind === 'write_off') {
      writtenOff[settlement.participant_id] = (writtenOff[settlement.participant_id] ?? 0) + amount
    }
  }
  const people: PersonRow[] = peopleIds(event).map((id) => ({
    id,
    share: shares[id] ?? 0,
    paid: paid[id] ?? 0,
    moved: moved[id] ?? 0,
    written_off: writtenOff[id] ?? 0,
    saldo: (shares[id] ?? 0) - (paid[id] ?? 0) - (moved[id] ?? 0) - (writtenOff[id] ?? 0),
  }))
  const organizer = people.find((row) => row.id === event.organizer)
  if (organizer) {
    organizer.saldo = -people.filter((row) => row.id !== event.organizer).reduce((sum, row) => sum + row.saldo, 0)
  }
  distributeMyPart(purchases, items, people.find((row) => row.id === ME)?.share ?? 0)
  return { people, items, purchases, spent, planned }
}

export function eventStatus(event: SharedEvent, split: Split): SharedEventStatus {
  const bought = event.items.some((item) => !item.cancelled && item.purchases.length > 0)
  if (!bought) return 'planning'
  if (event.settlements.length === 0) return 'in_progress'
  return split.people.every((row) => Math.abs(row.saldo) < 1) ? 'closed' : 'settling'
}
