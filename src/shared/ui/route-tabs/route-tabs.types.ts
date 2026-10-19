import type { LocationQuery } from 'vue-router'

export interface RouteTab {
  /** Route name the tab opens. */
  name: string
  label: string
}

export interface RouteTabsProps {
  tabs: RouteTab[]
  /**
   * What the next tab's address keeps of the current query: nothing by
   * default, all of it with `true`, or what the function returns.
   */
  keepQuery?: boolean | ((query: LocationQuery) => LocationQuery)
}
