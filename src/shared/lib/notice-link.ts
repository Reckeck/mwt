import type { LocationQuery, RouteLocationRaw } from 'vue-router'

import { entityLocation, parseEntityKey } from './entity-link'

const OPEN_PREFIX = 'open:'

/**
 * Where a notice leads. The backend names a route (`planning-reminders`), a
 * route with the id it is about (`wallet:<id>`), a path (`/?settings=sync`)
 * or the card of one thing over the current screen (`open:goal:<id>`).
 */
export function noticeRoute(link: string | null | undefined, query: LocationQuery = {}): RouteLocationRaw | null {
  if (!link) return null
  if (link.startsWith('/')) return link
  if (link.startsWith(OPEN_PREFIX)) {
    const target = parseEntityKey(link.slice(OPEN_PREFIX.length))
    return target ? entityLocation(target, query) : null
  }
  const [name, id] = link.split(':')
  return id ? { name, params: { id } } : { name }
}

/** The reminder a notice is about, from its key `reminder:<id>:<when>`. */
export function noticeReminderId(key: string): string | null {
  return key.startsWith('reminder:') ? (key.split(':')[1] ?? null) : null
}
