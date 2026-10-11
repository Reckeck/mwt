import type { RouteLocationRaw } from 'vue-router'

/**
 * Where a notice leads. The backend names a route (`planning-reminders`) or a
 * route with the id it is about (`wallet:<id>`).
 */
export function noticeRoute(link: string | null | undefined): RouteLocationRaw | null {
  if (!link) return null
  const [name, id] = link.split(':')
  return id ? { name, params: { id } } : { name }
}

/** The reminder a notice is about, from its key `reminder:<id>:<when>`. */
export function noticeReminderId(key: string): string | null {
  return key.startsWith('reminder:') ? (key.split(':')[1] ?? null) : null
}
