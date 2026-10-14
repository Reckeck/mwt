import { call } from '@/shared/api/ipc'

import type { BillRow } from '../utility-bill.types'

/** The readings a meter line has typed: one per zone, or the one reading. */
export function typedReadings(row: BillRow): FormulaZoneReading[] | null {
  const kind = row.line.item.kind
  if (!('metered' in kind)) return null
  const parts = kind.metered.zones.length
    ? row.line.zones.map((zone) => ({
        zone: zone.zone,
        previous: zone.previous_reading,
        current: row.zoneReadings[zone.zone] ?? '',
      }))
    : [{ zone: null, previous: row.line.previous_reading, current: row.reading }]
  if (parts.some((part) => !part.current || part.previous === null || Number(part.current) < Number(part.previous)))
    return null
  return parts.map((part) => ({ zone: part.zone, previous: part.previous ?? '0', current: part.current }))
}

/** What the typed readings used, all zones together. */
export function rowConsumption(row: BillRow): number | null {
  const readings = typedReadings(row)
  return readings ? readings.reduce((sum, part) => sum + Number(part.current) - Number(part.previous), 0) : null
}

/** A typed reading re-prices its meter line the way the core bills it (tariff or formula). */
export async function priceRow(row: BillRow) {
  const kind = row.line.item.kind
  const readings = typedReadings(row)
  if (!('metered' in kind) || !readings) return
  const key = JSON.stringify(readings)
  row.pricing = key
  const { tariff, formula, zones } = kind.metered
  try {
    const preview = await call<UtilityFormulaPreview>('preview_utility_formula', {
      input: { tariff, formula, zones, readings, days: null } satisfies UtilityFormulaInput,
    })
    // A later keystroke already asked again.
    if (row.pricing !== key || !preview.amount) return
    row.amount = preview.amount
    row.include = true
  } catch {
    // The amount stays as typed; the user can still enter it by hand.
  }
}
