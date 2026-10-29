/** How many events the sidebar lists before «All events». */
export const SIDEBAR_EVENTS_LIMIT = 5

/** Active: not archived and not closed. */
export const isActiveEvent = (event: SharedEventSummary) => !event.archived && event.status !== 'closed'

/** Nearest start first, undated last, then by title. */
export function activeEvents(events: SharedEventSummary[]): SharedEventSummary[] {
  return events.filter(isActiveEvent).sort((left, right) => {
    if (left.starts_on !== right.starts_on) {
      if (left.starts_on === null) return 1
      if (right.starts_on === null) return -1
      return left.starts_on < right.starts_on ? -1 : 1
    }
    return left.title.localeCompare(right.title)
  })
}

/** My balance in the event: positive they owe me, negative I owe; `null` when even. */
export function eventBalance(event: SharedEventSummary): string | null {
  if (Number(event.owed_to_me) > 0) return event.owed_to_me
  if (Number(event.i_owe) > 0) return `-${event.i_owe}`
  return null
}
