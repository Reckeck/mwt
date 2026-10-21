/** A figure against its base (plan H1): the change in money and in percent of the base. */
export interface ComparisonDelta {
  delta: number | null
  pct: number | null
}

/** Mirrors core `Comparison::delta` / `delta_pct`: no percent on a missing or zero base. */
export function comparisonDelta(cmp: Comparison | null | undefined): ComparisonDelta {
  if (!cmp || cmp.base === null) return { delta: null, pct: null }
  const value = Number(cmp.value)
  const base = Number(cmp.base)
  return { delta: value - base, pct: base === 0 ? null : ((value - base) / Math.abs(base)) * 100 }
}
