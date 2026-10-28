export interface ReadonlyFieldProps {
  label: string
  /** The text shown; an empty one reads as «—». */
  value?: string
  /** A copy button at the end of the value. */
  copyable?: boolean
  /** Names what is copied in the «Скопировано» toast. */
  copyKey?: string
}
