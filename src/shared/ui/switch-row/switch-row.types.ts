export interface SwitchRowProps {
  /** The switch's id; the label points at it. */
  id: string
  label: string
  /** What turning it on changes, under the label. */
  hint?: string
  disabled?: boolean
}
