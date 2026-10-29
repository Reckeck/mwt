import type { UiAction } from '@/shared/ui/context-menu'

export interface ActionMenuProps {
  actions: UiAction[]
  /** Accessible name of the «⋯» button. */
  label: string
  /** On a phone: the head of the sheet (what the actions act on). */
  sheet?: Omit<ActionSheetProps, 'actions'>
}

export interface ActionSheetProps {
  actions: UiAction[]
  /** What the actions act on: `To Mom before payday`. */
  title?: string
  /** A line under it: `Oct 20 · Tinkoff → Mom`. */
  description?: string
  /** The figure on the right: «−5 000 ₽». */
  value?: string
}
