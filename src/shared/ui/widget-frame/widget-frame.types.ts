import type { RouteLocationRaw } from 'vue-router'

import type { UiAction } from '@/shared/ui/context-menu'

export interface WidgetFrameProps {
  title: string
  /** Header hidden and position locked. */
  pinned?: boolean
  /** The widget's menu: right click and the «⋯» button show the same list. */
  actions?: UiAction[]
  /** Makes the title a link to the screen behind the widget. */
  to?: RouteLocationRaw
}
