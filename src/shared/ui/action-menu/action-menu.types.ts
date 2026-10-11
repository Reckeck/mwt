import type { UiAction } from '@/shared/ui/context-menu'

export interface ActionMenuProps {
  actions: UiAction[]
  /** Accessible name of the «⋯» button. */
  label: string
}
