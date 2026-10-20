/** Tailwind max-width class applied to the dialog panel per size. */
export const dialogWidths = {
  sm: 'max-w-md',
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
}
