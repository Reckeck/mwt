import { describe, expect, it } from 'vitest'

import { fieldsOfAutomation, scheduleOfFields } from './automation-fields'

const stored = (
  schedule: Schedule,
): Pick<AutomationInput, 'name' | 'active' | 'schedule' | 'starts_at' | 'amount_mode' | 'workday_shift'> => ({
  name: 'Salary',
  active: false,
  schedule,
  starts_at: new Date(2026, 10, 5, 12).toISOString(),
  amount_mode: 'varies',
  workday_shift: 'back',
})

describe('automation fields', () => {
  it.each<Schedule>([{ monthly: { day: 15 } }, { weekly: { weekday: 3 } }, { every_n_days: { days: 10 } }])(
    'round-trips the schedule %j',
    (schedule) => {
      expect(scheduleOfFields(fieldsOfAutomation(stored(schedule)))).toEqual(schedule)
    },
  )

  it('keeps name, flags, date and shift', () => {
    expect(fieldsOfAutomation(stored({ monthly: { day: 1 } }))).toEqual({
      name: 'Salary',
      active: false,
      scheduleKind: 'monthly',
      day: '1',
      weekday: '0',
      everyDays: '7',
      startsAt: '2026-11-05',
      varies: true,
      workdayShift: 'back',
    })
  })

  it('a schedule out of range is not valid', () => {
    expect(scheduleOfFields({ ...fieldsOfAutomation(stored({ monthly: { day: 1 } })), day: '40' })).toBeNull()
  })
})
