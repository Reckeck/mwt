/** Tailwind max-width class applied to the dialog panel per size. */
export const dialogWidths = {
  sm: 'max-w-md',
  /** 500px: a question with two lists under it (Currency-2 · 9). */
  confirm: 'max-w-[31.25rem]',
  /** 560px: the deal calculator (Money 4 · 23). */
  form: 'max-w-[35rem]',
  /** 620px: the automation wizard (Auto-1 · 6–8). */
  wizard: 'max-w-[38.75rem]',
  /** 640px: the month's utility bill and the item wizard (Wallets-2 · 13, Wallets-3 · 14–17). */
  bill: 'max-w-[40rem]',
  md: 'max-w-2xl',
  lg: 'max-w-3xl',
  xl: 'max-w-4xl',
} as const

export type DialogSize = keyof typeof dialogWidths

export interface DialogProps {
  /** Controlled open state; omit for an uncontrolled dialog. */
  open?: boolean
  title: string
  description?: string
  /** Tailwind max-width class for the panel. */
  size?: DialogSize
  /** Name of the layer for `data-overlay` (docs/ui-overlays.md; check-layout opens it by that). */
  overlay?: string
}
