import type { SumPart } from '@/shared/lib/money-parts'

/** One programme as a row of the table (Ops-2 · 12). */
export interface ProgramRow {
  store: Store
  /** `Tinkoff · points = ₽`, `Bybit · 1 coupon = 2.50 $`. */
  subtitle: SumPart[]
  /** `1,240 points`. */
  points: string
  valueBase: string
  /** `120 points expire Nov 1`, empty when nothing burns. */
  expiring: string
  /** It burns within a month: the line is in the warning colour. */
  soon: boolean
}

export type PointsMode = 'credit' | 'spend'
