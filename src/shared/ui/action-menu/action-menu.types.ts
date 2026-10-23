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
  /** What the actions act on: «Маме до зарплаты». */
  title?: string
  /** A line under it: «20 окт · Тинькофф → Мама». */
  description?: string
  /** The figure on the right: «−5 000 ₽». */
  value?: string
}
