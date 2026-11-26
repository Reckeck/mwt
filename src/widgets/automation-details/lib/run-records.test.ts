import { describe, expect, it } from 'vitest'

import { recordsById, runNeedsAction } from './run-records'

const record = (patch: Partial<AutomationRun>): AutomationRun => ({
  id: 'run',
  automation_id: 'auto',
  planned_at: '',
  processed_at: '',
  outcome: { created: { tx_id: 'tx' } },
  estimated: false,
  amount: '1',
  symbol: 'rub',
  ...patch,
})

describe('run records', () => {
  it('asks for action on a refusal and on an estimated amount only', () => {
    expect(runNeedsAction(record({ outcome: { rejected: { reason: 'insufficient_funds' } as never } }))).toBe(true)
    expect(runNeedsAction(record({ estimated: true }))).toBe(true)
    expect(runNeedsAction(record({}))).toBe(false)
    expect(runNeedsAction(record({ outcome: 'skipped', estimated: true }))).toBe(false)
    expect(runNeedsAction(undefined)).toBe(false)
  })

  it('finds a record by its run id', () => {
    expect(recordsById([record({ id: 'a' }), record({ id: 'b' })]).get('b')?.id).toBe('b')
    expect(recordsById(undefined).size).toBe(0)
  })
})
