import type { LocationQuery, RouteLocationRaw } from 'vue-router'

import { sectionOfRoute } from './nav-sections'

interface BackRouter {
  back: () => void
  push: (to: RouteLocationRaw) => unknown
}

/** The money group keeps its period filter on the way back; only the report view is dropped. */
function keepFilter(query: LocationQuery): LocationQuery {
  const { view: _view, ...rest } = query
  return rest
}

/**
 * The top-bar Back: with a previous in-app entry (vue-router's `history.state.back`) it steps back;
 * otherwise (a deep link) it opens the root of the current section, the dashboard when there is none.
 */
export function goBackOrToRoot(router: BackRouter, route: { name?: unknown; query: LocationQuery }): void {
  if (window.history.state?.back) {
    router.back()
    return
  }
  const section = typeof route.name === 'string' ? sectionOfRoute(route.name) : undefined
  if (!section) {
    void router.push({ name: 'dashboard' })
    return
  }
  void router.push({ name: section.root, query: section.id === 'budget' ? keepFilter(route.query) : {} })
}
