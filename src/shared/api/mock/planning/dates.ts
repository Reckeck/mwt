/** `preview_automation_dates`: the next three dates under «Когда?» of the wizard (Auto-1 · 8). */

import { now, plannedDates } from './state'

/** The next three dates after now, not before the start, at the repeat's time. */
export function previewAutomationDates(input: AutomationDatesInput): string[] {
  const [hours, minutes] = (input.time ?? '09:00').split(':').map(Number)
  const start = new Date(Math.max(new Date(input.starts_at).getTime(), now().getTime()))
  const until = new Date(start.getTime() + 4 * 366 * 86_400_000)
  return plannedDates(
    input.schedule,
    new Date(start.getFullYear(), start.getMonth(), start.getDate()),
    until,
    input.workday_shift,
  )
    .map((date) => {
      date.setHours(hours ?? 9, minutes ?? 0, 0, 0)
      return date
    })
    .filter((date) => date > now())
    .slice(0, 3)
    .map((date) => date.toISOString())
}
