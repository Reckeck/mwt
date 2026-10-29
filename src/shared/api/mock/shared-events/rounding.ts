/** Spreading an item to the cent, hand-set totals and rounding to the event's step (`split.rs`). */

import { CENT, SCALE, dec, divRound, roundTo, sum } from './dec'

/** Spread `cost` to the cent; the leftover cents go to `taker` when they share, else the first sharer. */
export function spread(cost: bigint, parts: Array<[string, bigint]>, taker: string): Record<string, bigint> {
  const out: Record<string, bigint> = {}
  const total = sum(parts.map(([, weight]) => weight))
  if (parts.length === 0 || total <= 0n) return out
  let given = 0n
  for (const [id, weight] of parts) {
    const part = divRound(cost * weight, total * CENT) * CENT
    out[id] = part
    given += part
  }
  out[taker in out ? taker : parts[0][0]] += cost - given
  return out
}

/** Whether the hand-set totals fit what was bought. */
export function overridesFit(fixed: Array<[string, bigint]>, pool: string[], spent: bigint): boolean {
  const inPool = fixed.filter(([id]) => pool.includes(id))
  if (inPool.length === 0) return true
  const total = sum(inPool.map(([, value]) => value))
  if (total > spent) return false
  const everyone = pool.every((id) => inPool.some(([fixedId]) => fixedId === id))
  return !everyone || total === spent
}

/** Organizer first when open, then the biggest payer (the first on a tie), then the biggest share. */
function leftoverOrder(
  event: SharedEvent,
  pool: string[],
  shares: Record<string, bigint>,
  paid: Record<string, bigint>,
  locked: string[],
): string[] {
  const open = pool.filter((id) => !locked.includes(id))
  const order = open.includes(event.organizer) ? [event.organizer] : []
  const pay = (id: string) => paid[id] ?? 0n
  const part = (id: string) => shares[id] ?? 0n
  const rest = open
    .filter((id) => id !== event.organizer)
    .sort((left, right) => {
      const byPay = pay(right) - pay(left)
      if (byPay !== 0n) return byPay < 0n ? -1 : 1
      const byPart = part(right) - part(left)
      return byPart === 0n ? 0 : byPart < 0n ? -1 : 1
    })
  return [...order, ...rest]
}

export function applyOverrides(
  event: SharedEvent,
  pool: string[],
  shares: Record<string, bigint>,
  paid: Record<string, bigint>,
  spent: bigint,
): { locked: string[]; invalid: boolean } {
  const fixed = event.adjustments
    .filter((fix) => pool.includes(fix.participant_id))
    .map((fix): [string, bigint] => [fix.participant_id, dec(fix.total)])
  if (fixed.length === 0) return { locked: [], invalid: false }
  if (!overridesFit(fixed, pool, spent)) return { locked: [], invalid: true }
  const fixedIds = fixed.map(([id]) => id)
  const others = pool.filter((id) => !fixedIds.includes(id))
  if (others.length > 0) {
    let parts = others.map((id): [string, bigint] => [id, shares[id] ?? 0n])
    if (sum(parts.map(([, weight]) => weight)) <= 0n) parts = others.map((id): [string, bigint] => [id, SCALE])
    let taker = others[0]
    for (const id of others) if ((paid[id] ?? 0n) > (paid[taker] ?? 0n)) taker = id
    const rest = spread(spent - sum(fixed.map(([, value]) => value)), parts, taker)
    for (const id of others) shares[id] = rest[id] ?? 0n
  }
  for (const [id, value] of fixed) shares[id] = value
  return { locked: fixedIds, invalid: false }
}

export function roundTotals(
  event: SharedEvent,
  pool: string[],
  shares: Record<string, bigint>,
  paid: Record<string, bigint>,
  locked: string[],
  spent: bigint,
) {
  const step = dec(event.rounding_step)
  if (step <= 0n || step === CENT) return
  for (const id of pool.filter((entry) => !locked.includes(entry))) {
    if (id in shares) shares[id] = roundTo(shares[id], step)
  }
  let leftover = spent - sum(pool.map((id) => shares[id] ?? 0n))
  if (leftover === 0n) return
  const order = leftoverOrder(event, pool, shares, paid, locked)
  if (leftover > 0n) {
    if (order.length > 0) shares[order[0]] = (shares[order[0]] ?? 0n) + leftover
    return
  }
  for (const id of order) {
    const part = shares[id] ?? 0n
    const take = -leftover < part ? -leftover : part
    shares[id] = part - take
    leftover += take
    if (leftover === 0n) return
  }
}
