import { dateInputToIso, isoToDateInput } from '@/shared/lib/planning'

import type { ScheduleKind } from '../automation-edit.constants'

/**
 * The automation form's own fields as plain values. The operation template is a form of its
 * own (`TransactionForm`) and the wallet picker only chooses where it opens, so neither is here.
 */
export interface AutomationFields {
  name: string
  active: boolean
  scheduleKind: ScheduleKind
  day: string
  weekday: string
  everyDays: string
  startsAt: string
  varies: boolean
  workdayShift: WorkdayShift
}

/** The fields a stored automation (or an input) fills; `startsAt` falls back to `fallbackStart`. */
export function fieldsOfAutomation(
  automation: Pick<AutomationInput, 'name' | 'active' | 'schedule' | 'starts_at' | 'amount_mode' | 'workday_shift'>,
  fallbackStart = '',
): AutomationFields {
  const schedule = automation.schedule
  const fields: AutomationFields = {
    name: automation.name,
    active: automation.active,
    scheduleKind: 'monthly',
    day: '1',
    weekday: '0',
    everyDays: '7',
    startsAt: isoToDateInput(automation.starts_at) || fallbackStart,
    varies: automation.amount_mode === 'varies',
    workdayShift: automation.workday_shift,
  }
  if ('monthly' in schedule) {
    fields.day = String(schedule.monthly.day)
  } else if ('weekly' in schedule) {
    fields.scheduleKind = 'weekly'
    fields.weekday = String(schedule.weekly.weekday)
  } else {
    fields.scheduleKind = 'every_n_days'
    fields.everyDays = String(schedule.every_n_days.days)
  }
  return fields
}

/** The schedule the fields name; `null` while it is not valid. */
export function scheduleOfFields(fields: AutomationFields): Schedule | null {
  if (fields.scheduleKind === 'monthly') {
    const value = Number(fields.day)
    return value >= 1 && value <= 31 ? { monthly: { day: value } } : null
  }
  if (fields.scheduleKind === 'weekly') return { weekly: { weekday: Number(fields.weekday) } }
  const value = Number(fields.everyDays)
  return Number.isInteger(value) && value >= 1 ? { every_n_days: { days: value } } : null
}

/** What the form does not edit itself: it is carried over from the automation being edited. */
export interface AutomationRest {
  preset: AutomationPreset | null
  time: string | null
  asks: boolean
  template: TransactionInput
}

export function automationInputOfFields(
  fields: AutomationFields,
  schedule: Schedule,
  rest: AutomationRest,
): AutomationInput {
  return {
    name: fields.name.trim(),
    active: fields.active,
    preset: rest.preset,
    schedule,
    starts_at: dateInputToIso(fields.startsAt),
    template: rest.template,
    amount_mode: fields.varies ? 'varies' : 'fixed',
    workday_shift: fields.workdayShift,
    time: rest.time,
    asks: rest.asks,
  }
}

/**
 * The edit form's base: the stored automation written the way a save writes it, so a field the
 * user did not touch is never counted as changed (dates to the day, the template through the
 * form's own mapping, handed in as `template`). `null` when the stored schedule or template cannot be read.
 */
export function automationBaseOf(automation: Automation, template: TransactionInput | null): AutomationInput | null {
  const fields = fieldsOfAutomation(automation)
  const schedule = scheduleOfFields(fields)
  if (!schedule || !template) return null
  return automationInputOfFields(fields, schedule, {
    preset: automation.preset,
    time: automation.time,
    asks: automation.asks,
    template,
  })
}
