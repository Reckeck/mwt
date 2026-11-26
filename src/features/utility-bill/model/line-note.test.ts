import { describe, expect, it } from 'vitest'

import type { SumPart } from '@/shared/lib/money-parts'

import { housingRow, meterRow } from './bill-row.fixture'
import { automated, lineCalc, lineNote, readInCard } from './line-note'

const t = (key: string) => key
const text = (parts: SumPart[]) => parts.map((part) => ('text' in part ? part.text : '')).join('')

describe('line note', () => {
  it('leaves out rent an automation already pays', () => {
    expect(automated(housingRow('a1'))).toBe(true)
    expect(automated(housingRow(null))).toBe(false)
    expect(text(lineNote(housingRow('a1'), t, null))).toBe('utilityBill.automated')
    expect(text(lineNote(housingRow('a1'), t, 'Rent'))).toBe('utilityBill.automatedNamed')
  })

  it('says «as last time» for rent that is not automated', () => {
    expect(text(lineNote(housingRow(null), t, null))).toBe('utilityBill.asLastTime')
  })

  it('tells a stored reading from a missing one', () => {
    expect(readInCard(meterRow())).toBe(false)
    expect(readInCard(meterRow({ reading: '120' }))).toBe(true)
    expect(readInCard(housingRow(null))).toBe(false)
  })

  it('shows the previous reading and the tariff while nothing is typed', () => {
    expect(text(lineNote(meterRow(), t, null))).toBe('utilityBill.was · utilityTab.tariff')
  })

  it('names the formula instead of the tariff', () => {
    const row = meterRow({}, {}, { formula: 'расход * тариф' })
    expect(text(lineNote(row, t, null))).toContain('utilityBill.formula')
  })

  it('prices only readings that are not smaller than the previous one', () => {
    expect(lineCalc(meterRow(), t)).toBeNull()
    expect(lineCalc(meterRow({}, { reading: '90' }), t)).toBeNull()
    expect(lineCalc(meterRow({}, { reading: '125' }), t)).toBe('utilityBill.used')
  })
})
