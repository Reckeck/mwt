import type { Component } from 'vue'
import type { RouteLocationRaw } from 'vue-router'

export interface DashboardWidgetDef {
  id: string
  titleKey: string
  descriptionKey: string
  component: Component
  /** Props the component needs on the dashboard. */
  props?: Record<string, unknown>
  /** Default size in grid cells. */
  w: number
  h: number
  minW: number
  minH: number
  /**
   * Only offered when the vault has an exchange; the catalog asks which one,
   * and the widget gets it as `storeId`.
   */
  requires?: 'exchange'
  /** Where the frame's title leads: the full screen behind the widget. */
  link?: RouteLocationRaw
}
