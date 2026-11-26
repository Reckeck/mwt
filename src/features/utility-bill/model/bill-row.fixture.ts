import type { BillRow } from '../utility-bill.types'

/** A bill row for tests: only the fields the model reads are filled. */
export function meterRow(
  patch: Partial<BillRow['line']> = {},
  row: Partial<BillRow> = {},
  metered: object = {},
): BillRow {
  const line = {
    item: {
      currency: 'RUB',
      kind: { metered: { unit: 'kWh', tariff: '6.5', formula: null, zones: [], ...metered } },
    },
    paid: '0',
    reading: null,
    previous_reading: '100',
    consumption: null,
    zones: [],
    includes_deposit: false,
    ...patch,
  } as unknown as BillRow['line']
  return { line, include: false, reading: '', zoneReadings: {}, amount: '0', ...row }
}

export function housingRow(automationId: string | null): BillRow {
  const line = {
    item: { currency: 'RUB', kind: { housing: { due_day: 5, automation_id: automationId } } },
    paid: '0',
    zones: [],
    includes_deposit: false,
  } as unknown as BillRow['line']
  return { line, include: false, reading: '', zoneReadings: {}, amount: '0' }
}
