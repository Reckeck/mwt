import { decimalAdd, decimalSub, parseDecimal } from '@/shared/lib/decimal'

import { sign } from './settlement'

/** The three ways to share an item: everybody, chosen people, by units. */
export type SplitMode = 'all' | 'some' | 'units'

/** An empty circle in `equal` means everybody. */
export function splitMode(split: ItemSplit): SplitMode {
  if ('by_units' in split) return 'units'
  return split.equal.among.length ? 'some' : 'all'
}

/** The request for a mode: `units` hold what each person took (text, zero or empty skipped). */
export function splitOf(mode: SplitMode, among: string[], units: Record<string, string>): ItemSplit {
  if (mode === 'units') {
    const taken: Record<string, string> = {}
    for (const [id, qty] of Object.entries(units)) {
      const value = parseDecimal(qty)
      if (value !== null && sign(value) > 0) taken[id] = value
    }
    return { by_units: { units: taken } }
  }
  return { equal: { among: mode === 'some' ? among : [] } }
}

/** The sum of the units handed out. */
export function unitsTotal(units: Record<string, string>): string {
  let total = '0'
  for (const qty of Object.values(units)) {
    const value = parseDecimal(qty)
    if (value !== null) total = decimalAdd(total, value) ?? total
  }
  return total
}

export type UnitsCheck =
  | { kind: 'ok' }
  | { kind: 'short'; left: string }
  | { kind: 'over'; extra: string }
  | { kind: 'empty' }

/** Units must add up to what was bought: not less (`short`), not more (`over`). */
export function checkUnits(units: Record<string, string>, bought: string): UnitsCheck {
  const total = unitsTotal(units)
  if (sign(total) === 0) return { kind: 'empty' }
  const left = decimalSub(bought, total)
  if (left === null || sign(left) === 0) return { kind: 'ok' }
  return sign(left) > 0 ? { kind: 'short', left } : { kind: 'over', extra: left.slice(1) }
}

export interface SplitCaption {
  mode: SplitMode
  /** Names of the chosen people (`some`), or `name qty` pairs (`units`). */
  parts: string[]
}

/** The line under the item: nothing for everybody, `only Pete, Vasya` or `by units · Pete 3, Vasya 5`. */
export function splitCaption(
  split: ItemSplit,
  nameOf: (id: string) => string,
  format: (qty: string) => string = (qty) => qty,
): SplitCaption {
  if ('by_units' in split) {
    return {
      mode: 'units',
      parts: Object.entries(split.by_units.units)
        .filter(([, qty]) => sign(qty) > 0)
        .map(([id, qty]) => `${nameOf(id)} ${format(qty)}`),
    }
  }
  const among = split.equal.among
  return { mode: among.length ? 'some' : 'all', parts: among.map(nameOf) }
}
