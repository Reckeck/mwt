/** One field's last two readings, as numbers in text. */
export interface ReadingSample {
  previous: string
  current: string
}

const before = (reading: string, consumption: string) => String(Number(reading) - Number(consumption))

/**
 * The meter's latest month with a consumption, field by field in the order of
 * `zones` (Wallets-3 · 16: «(209 − 205) × 3,5 × 10»); `null` when there is none
 * yet or the month's zones are not the ones being set up.
 */
export function lastReadings(trend: UtilityTrend | undefined, zones: string[]): ReadingSample[] | null {
  const months = [...(trend?.months ?? [])].reverse()
  if (!zones.length) {
    const month = months.find((entry) => entry.reading !== null && entry.consumption !== null)
    if (!month?.reading || !month.consumption) return null
    return [{ previous: before(month.reading, month.consumption), current: month.reading }]
  }
  for (const month of months) {
    const parts = zones.map((zone) => month.zones.find((entry) => entry.zone === zone))
    if (parts.every((part) => part?.consumption != null))
      return parts.map((part) => ({ previous: before(part!.reading, part!.consumption!), current: part!.reading }))
  }
  return null
}
