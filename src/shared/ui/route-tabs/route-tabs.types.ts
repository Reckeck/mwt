import type { LocationQuery } from 'vue-router'

export interface RouteTab {
  /** Route name the tab opens. */
  name: string
  label: string
  /** A line under the label in the «More» menu: what the tab is for. */
  hint?: string
}

export interface RouteTabsProps {
  tabs: RouteTab[]
  /**
   * Tabs behind «More ▾» at the end of the strip. While one of them is open, the
   * button shows its label, so the strip still says where you are.
   */
  more?: RouteTab[]
  /** The «More» button's label when none of `more` is open. */
  moreLabel?: string
  /** The title of the phone sheet with `more` («Ещё в «Деньгах»»); `moreLabel` when unset. */
  moreTitle?: string
  /**
   * What the next tab's address keeps of the current query: nothing by
   * default, all of it with `true`, or what the function returns.
   */
  keepQuery?: boolean | ((query: LocationQuery) => LocationQuery)
}
