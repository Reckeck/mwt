import { beforeEach, describe, expect, it, vi } from 'vitest'

import { call } from '@/shared/api/ipc'

import { meterRow } from './bill-row.fixture'
import { priceRow, rowConsumption, typedReadings } from './price-row'

describe('price row', () => {
  beforeEach(() => {
    vi.mocked(call).mockClear()
  })

  it('needs a typed reading that is not below the previous one', () => {
    expect(typedReadings(meterRow())).toBeNull()
    expect(typedReadings(meterRow({}, { reading: '99' }))).toBeNull()
    expect(typedReadings(meterRow({}, { reading: '100' }))).toEqual([{ zone: null, previous: '100', current: '100' }])
  })

  it('adds the consumption of every zone', () => {
    const row = meterRow(
      {
        zones: [
          { zone: 'day', previous_reading: '10', reading: null },
          { zone: 'night', previous_reading: '5', reading: null },
        ] as never,
      },
      { zoneReadings: { day: '15', night: '8' } },
      { zones: ['day', 'night'] },
    )
    expect(rowConsumption(row)).toBe(8)
  })

  it('takes the core amount and includes the line', async () => {
    vi.mocked(call).mockResolvedValueOnce({ amount: '812.5' })
    const row = meterRow({}, { reading: '125' })
    await priceRow(row)
    expect(row.amount).toBe('812.50')
    expect(row.include).toBe(true)
  })

  it('keeps the typed amount when pricing fails', async () => {
    vi.mocked(call).mockRejectedValueOnce(new Error('no'))
    const row = meterRow({}, { reading: '125', amount: '7' })
    await priceRow(row)
    expect(row.amount).toBe('7')
    expect(row.include).toBe(false)
  })
})
