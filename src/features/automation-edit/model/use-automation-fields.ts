import { computed, ref } from 'vue'

import type { ScheduleKind } from '../automation-edit.constants'
import { type AutomationFields, scheduleOfFields } from '../lib/automation-fields'

/** The automation dialog's own fields as refs; `load` writes a set of them, `currentFields` reads them back. */
export function useAutomationFields() {
  const name = ref('')
  const active = ref(true)
  const scheduleKind = ref<ScheduleKind>('monthly')
  const day = ref('1')
  const weekday = ref('0')
  const everyDays = ref('7')
  const startsAt = ref('')
  const varies = ref(false)
  const workdayShift = ref<WorkdayShift>('none')

  function load(fields: AutomationFields) {
    name.value = fields.name
    active.value = fields.active
    scheduleKind.value = fields.scheduleKind
    day.value = fields.day
    weekday.value = fields.weekday
    everyDays.value = fields.everyDays
    startsAt.value = fields.startsAt
    varies.value = fields.varies
    workdayShift.value = fields.workdayShift
  }
  const currentFields = (): AutomationFields => ({
    name: name.value,
    active: active.value,
    scheduleKind: scheduleKind.value,
    day: day.value,
    weekday: weekday.value,
    everyDays: everyDays.value,
    startsAt: startsAt.value,
    varies: varies.value,
    workdayShift: workdayShift.value,
  })
  const schedule = computed<Schedule | null>(() => scheduleOfFields(currentFields()))

  return {
    name,
    active,
    scheduleKind,
    day,
    weekday,
    everyDays,
    startsAt,
    varies,
    workdayShift,
    load,
    currentFields,
    schedule,
  }
}
