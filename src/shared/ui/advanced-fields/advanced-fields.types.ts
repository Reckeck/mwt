export interface AdvancedFieldsProps {
  /** The toggle's text; `More options` by default. */
  label?: string
  /** The toggle's text while open (`Hide: date, fee…`); `label` when unset. */
  openLabel?: string
  /** `primary`: a link-coloured row with a chevron down, what the short operation form uses. */
  tone?: 'muted' | 'primary'
  /** Open from the start, e.g. when one of the hidden fields already has a value or an error. */
  defaultOpen?: boolean
}
