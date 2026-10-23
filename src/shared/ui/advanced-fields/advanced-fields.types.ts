export interface AdvancedFieldsProps {
  /** The toggle's text; "Дополнительно" by default. */
  label?: string
  /** The toggle's text while open («Скрыть: дата, комиссия…»); `label` when unset. */
  openLabel?: string
  /** `primary`: a link-coloured row with a chevron down, what the short operation form uses. */
  tone?: 'muted' | 'primary'
  /** Open from the start, e.g. when one of the hidden fields already has a value or an error. */
  defaultOpen?: boolean
}
