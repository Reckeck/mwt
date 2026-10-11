import type { UiAction } from '@/shared/ui/context-menu'

export interface WidgetFrameProps {
  title: string
  /** Header hidden and position locked. */
  pinned?: boolean
  /** Right-click menu of the widget. */
  actions?: UiAction[]
}
