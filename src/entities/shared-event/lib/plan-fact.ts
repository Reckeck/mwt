import { sign } from './settlement'

/** Where an item stands against the plan. */
export type ItemState = 'cancelled' | 'planned' | 'bought' | 'unplanned'

export interface PlanFact {
  state: ItemState
  /** How much was bought, in the item's unit. */
  bought: string
  planQty: string | null
  planTotal: string | null
  /** What was spent on the item, in the event currency. */
  cost: string
  /** Spent minus planned; only for a planned item that was bought. */
  diffMoney: string | null
  /** Bought minus planned quantity; only for a planned item that was bought. */
  diffQty: string | null
}

/** One item against its plan: the state, and the differences the row shows. */
export function planFact(item: EventItem, view: ItemView): PlanFact {
  const planned = view.plan_total !== null || view.plan_qty !== null
  const state: ItemState = item.cancelled ? 'cancelled' : !view.bought ? 'planned' : planned ? 'bought' : 'unplanned'
  const compared = state === 'bought'
  return {
    state,
    bought: view.qty,
    planQty: view.plan_qty,
    planTotal: view.plan_total,
    cost: view.cost,
    diffMoney: compared ? view.diff_money : null,
    diffQty: compared ? view.diff_qty : null,
  }
}

/** `8 of 6 pcs`, `1 pack`, `4 of 4 kg`: what was bought against what was planned; `ofWord` is the localized "of". */
export function qtyText(
  fact: PlanFact,
  unit: string,
  format: (value: string) => string = (value) => value,
  ofWord = 'of',
): string {
  const bought = format(fact.bought)
  if (fact.planQty === null || fact.state === 'unplanned') return `${bought} ${unit}`
  return `${bought} ${ofWord} ${format(fact.planQty)} ${unit}`
}

/** `+600 ₽`, `−200 ₽`, `0 ₽`: a difference with its sign, the sum itself written by `money`. */
export function diffText(diff: string, money: (absolute: string) => string): string {
  const direction = sign(diff)
  if (direction === 0) return money('0')
  const text = money(diff.startsWith('-') ? diff.slice(1) : diff)
  return `${direction > 0 ? '+' : '−'}${text}`
}

/** Spending above the plan is bad (red), below it is good (green), equal is neutral. */
export function diffTone(diff: string | null): 'over' | 'under' | 'even' {
  const direction = sign(diff)
  return direction > 0 ? 'over' : direction < 0 ? 'under' : 'even'
}

/** Price of one unit: total over quantity, two places; null without a usable quantity or total. */
export function unitPrice(total: string, qty: string): string | null {
  const sum = Number(total)
  const count = Number(qty)
  if (!Number.isFinite(sum) || !Number.isFinite(count) || count <= 0) return null
  return String(Math.round((sum / count) * 100) / 100)
}

/** `5540` -> `-5540`, `-5540` -> `5540`, `0` stays `0`. */
export function negate(value: string): string {
  if (sign(value) === 0) return '0'
  return value.startsWith('-') ? value.slice(1) : `-${value}`
}
