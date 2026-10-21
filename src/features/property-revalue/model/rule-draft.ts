import { decimal, type FieldError, positiveAmount, required } from '@/shared/lib/form'
import { i18n } from '@/shared/config/i18n'
import { parseDecimal } from '@/shared/lib/decimal'

export type RuleMode = 'off' | 'inflation' | 'fixed'
export type FloorKind = RevalueFloor['kind']
export type RuleField = 'pct' | 'floor'

export interface RuleDraft {
  mode: RuleMode
  pct: string
  floorKind: FloorKind
  floor: string
}

export function draftOf(rule: RevalueRule | null | undefined): RuleDraft {
  const floor = rule?.floor
  return {
    mode: !rule ? 'off' : rule.pct_per_year === null ? 'inflation' : 'fixed',
    pct: rule?.pct_per_year ?? '',
    floorKind: floor?.kind ?? 'pct_of_purchase',
    floor: floor ? (floor.kind === 'amount' ? floor.amount : floor.pct) : '',
  }
}

const numberOf = (value: string) => Number(parseDecimal(value) ?? 'NaN')
const clean = (value: string) => value.trim().replace(',', '.')
const text = (key: string) => i18n.global.t(`revalueRule.${key}`)

/** Mirrors `check_rule` in `service/valuation.rs`: what the core would refuse. */
export function ruleErrors(draft: RuleDraft): Partial<Record<RuleField, FieldError>> {
  if (draft.mode !== 'fixed') return {}
  const pct = required(draft.pct) ?? decimal(draft.pct) ?? (numberOf(draft.pct) <= -100 ? text('pctRange') : null)
  if (pct || numberOf(draft.pct) >= 0) return { pct }
  const floor =
    positiveAmount(draft.floor) ??
    (draft.floorKind === 'pct_of_purchase' && numberOf(draft.floor) > 100 ? text('floorPctRange') : null)
  return { floor }
}

/** The rule to send; a floor only when the value can fall. */
export function ruleOf(draft: RuleDraft): RevalueRule | null {
  if (draft.mode === 'off') return null
  const pct = draft.mode === 'fixed' ? clean(draft.pct) : null
  const falls = pct !== null && numberOf(pct) < 0
  const value = clean(draft.floor)
  const floor: RevalueFloor | null =
    !falls && !value
      ? null
      : draft.floorKind === 'amount'
        ? { kind: 'amount', amount: value }
        : { kind: 'pct_of_purchase', pct: value }
  return { pct_per_year: pct, floor: value ? floor : null }
}

export const needsFloor = (draft: RuleDraft) => draft.mode === 'fixed' && numberOf(draft.pct) < 0
