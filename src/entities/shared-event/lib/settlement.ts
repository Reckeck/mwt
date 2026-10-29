import { decimalAdd, decimalSub, parseDecimal } from '@/shared/lib/decimal'

/** The id of the owner in an event's people (`PersonView.id`, `SharedEvent.organizer`). */
export const ME = 'me'

export type PersonStatus = 'organizer' | 'waiting' | 'partial' | 'returned' | 'closed' | 'even'

/** What the row's main button does: money comes to me, goes from me, or pays my share to the organizer. */
export type RowAction = 'receive' | 'give' | 'pay'

/** -1, 0 or 1; a value that is not a number counts as zero. Display logic only: the backend owns the rules. */
export function sign(value: string | null | undefined): -1 | 0 | 1 {
  const text = parseDecimal(value ?? '')
  if (text === null || /^-?0*(\.0*)?$/.test(text)) return 0
  return text.startsWith('-') ? -1 : 1
}

/** `-685` -> `685`. */
export function absolute(value: string): string {
  return value.startsWith('-') ? value.slice(1) : value
}

/** `amount` is more than `max` (both plain decimals). */
export function exceeds(amount: string, max: string): boolean {
  const difference = decimalSub(amount, max)
  return difference !== null && sign(difference) > 0
}

/** Which way money has to go to settle the person: positive saldo pays the organizer, negative is paid by them. */
export function settleDirection(saldo: string): SettleDirection | null {
  const direction = sign(saldo)
  if (direction === 0) return null
  return direction > 0 ? 'to_organizer' : 'from_organizer'
}

export function personStatus(person: PersonView): PersonStatus {
  if (person.is_organizer) return 'organizer'
  const touched = sign(person.moved) !== 0 || sign(person.written_off) > 0
  if (sign(person.saldo) === 0) {
    if (sign(person.written_off) > 0) return 'closed'
    return sign(person.moved) !== 0 ? 'returned' : 'even'
  }
  return touched ? 'partial' : 'waiting'
}

/** What the person owed (or was owed) before anything came back or was written off. */
export function originalDebt(person: PersonView): string {
  const settled = decimalAdd(person.moved, person.written_off) ?? '0'
  return decimalAdd(person.saldo, settled) ?? person.saldo
}

/** The person on the other end of the person's debt: the organizer, or the person when the organizer is me. */
function counterpart(view: SharedEventView, person: PersonView): PersonView | undefined {
  return view.event.organizer === ME ? person : view.people.find((entry) => entry.is_organizer)
}

/** A settlement can move money of mine only with someone who has a contact card; a guest is set-off only. */
export function canUseWallet(view: SharedEventView, person: PersonView): boolean {
  if (!view.event.i_participate) return false
  if (view.event.organizer !== ME && person.id !== ME) return false
  return Boolean(counterpart(view, person)?.contact_id)
}

/** The row's main button, or `null` when money of mine is not involved or there is nothing to settle. */
export function rowAction(view: SharedEventView, person: PersonView): RowAction | null {
  if (!view.event.i_participate || person.is_organizer) return null
  const direction = sign(person.saldo)
  if (direction === 0) return null
  if (view.event.organizer === ME) return person.id === ME ? null : direction > 0 ? 'receive' : 'give'
  if (person.id !== ME) return null
  return direction > 0 ? 'pay' : 'receive'
}

/** Only I can write a debt off, and only as the organizer who takes part; only a debt to the organizer. */
export function canWriteOff(view: SharedEventView, person: PersonView): boolean {
  return view.event.organizer === ME && view.event.i_participate && !person.is_organizer && sign(person.saldo) > 0
}

/** The people who would share the rest of the person's debt: everybody else, me included. */
export function reassignTargets(view: SharedEventView, person: PersonView): PersonView[] {
  return view.people.filter((entry) => entry.id !== person.id)
}

/** The settlement the last mutation added (the backend appends), for `Undo`. */
export function lastSettlementId(view: SharedEventView): string | null {
  return view.event.settlements.at(-1)?.id ?? null
}

const isWhole = (value: string) => /^-?\d+(\.0+)?$/.test(value)

/**
 * Fraction digits to show an event's sums with: none while the shares are rounded to whole
 * units and nobody paid a fraction (the usual event), two otherwise.
 */
export function sumDecimals(view: SharedEventView): number {
  const whole = isWhole(view.event.rounding_step) && Number(view.event.rounding_step) >= 1
  return whole && view.people.every((person) => isWhole(person.paid) && isWhole(person.saldo)) ? 0 : 2
}
