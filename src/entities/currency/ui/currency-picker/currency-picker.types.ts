export type CurrencyKind = 'fiat' | 'crypto'

export interface CurrencyPickerProps {
  /** Label above the control; pass `''` to hide. */
  label?: string
  placeholder?: string
  disabled?: boolean
  /** Which halves of the catalog to offer; both by default. */
  kinds?: CurrencyKind[]
  /** Offer a «none» row that sets the model to `''`. */
  allowEmpty?: boolean
  /** Extra codes that must be selectable even if the catalog lacks them (e.g. the stored value). */
  extra?: string[]
  /** Trigger shows the code only, for a suffix inside another field. */
  compact?: boolean
  /** `id` for the trigger, so an outer `<Label for>` can point at it. */
  id?: string
}

export interface CurrencyOption {
  code: string
  kind: CurrencyKind
  /** Localized fiat name from `Intl.DisplayNames`; empty for crypto. */
  name: string
  favorite: boolean
}
