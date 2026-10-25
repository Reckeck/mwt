import type { LocationQuery, RouteLocationRaw, Router } from 'vue-router'

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

/**
 * The target if the router knows it, else the home screen: the log can hold
 * notices with links of older versions (a renamed route, a removed path), and
 * `router.push` throws on a name with no route.
 */
export function reachable(router: Router, target: RouteLocationRaw): RouteLocationRaw {
  if (typeof target === 'object' && 'name' in target && target.name) {
    return router.hasRoute(target.name) ? target : '/'
  }
  return router.resolve(target).matched.length > 0 ? target : '/'
}

/** The reminder a notice is about, from its key `reminder:<id>:<when>`. */
export function noticeReminderId(key: string): string | null {
  return key.startsWith('reminder:') ? (key.split(':')[1] ?? null) : null
}
