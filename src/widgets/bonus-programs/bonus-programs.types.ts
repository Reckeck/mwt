/** One programme as a row of the table (Ops-2 · 12). */
export interface ProgramRow {
  store: Store
  /** «Тинькофф · баллы = ₽», «Bybit · 1 купон = 2,50 $». */
  subtitle: string
  /** «1 240 баллов». */
  points: string
  valueBase: string
  /** «120 баллов сгорят 1 нояб.», empty when nothing burns. */
  expiring: string
  /** It burns within a month: the line is in the warning colour. */
  soon: boolean
}

export type PointsMode = 'credit' | 'spend'
