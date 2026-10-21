/** Commands: inflation and the key rate from the statistics offices. */

import { resolve } from './core'
import type { MockCommands } from './core'
import { mockAppSettings } from './vault-data'

const SOURCES: Record<string, InflationSource> = { rub: 'cbr', usd: 'bls', eur: 'eurostat' }
/** Last month's rate and how it drifts back in time, per currency. */
const SHAPE: Record<string, [number, number]> = { rub: [6.35, 0.12], usd: [3.74, 0.04], eur: [2.2, 0.03] }
const modes: Record<string, InflationMode> = {}

function monthsBack(count: number): string[] {
  const now = new Date()
  return Array.from({ length: count }, (_unused, index) => {
    const date = new Date(now.getFullYear(), now.getMonth() - count + index, 1)
    return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}`
  })
}

function overview(): InflationCurrency[] {
  const manualTable = mockAppSettings.inflation ?? {}
  const codes = [...Object.keys(SOURCES), ...Object.keys(manualTable).filter((code) => !(code in SOURCES))]
  return codes.map((currency) => {
    const source = SOURCES[currency] ?? null
    const manual = manualTable[currency] ?? null
    const mode = modes[currency] ?? (source ? 'auto' : 'manual')
    const months = monthsBack(21)
    const [last, drift] = SHAPE[currency] ?? [0, 0]
    const series: InflationReading[] = source
      ? months.map((month, index) => ({
          currency,
          month,
          source,
          yoy_pct: (last + drift * (months.length - 1 - index) * Math.sin(index / 3)).toFixed(2),
        }))
      : []
    const latest = series.at(-1) ?? null
    return {
      currency,
      mode,
      chosen: currency in modes,
      source,
      latest,
      previous: series.at(-2)?.yoy_pct ?? null,
      manual,
      effective: mode === 'auto' && latest ? latest.yoy_pct : manual,
      series,
    }
  })
}

export const ratesCommands: MockCommands = {
  get_inflation_overview: () => resolve(overview()),
  refresh_rates: () => resolve(overview()),
  set_inflation_mode: ({ args }) => {
    const currency = String(args?.currency ?? '')
    const mode = args?.mode as InflationMode | null
    if (mode) modes[currency] = mode
    else delete modes[currency]
    return resolve(overview())
  },
}
