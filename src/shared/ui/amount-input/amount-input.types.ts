export interface AmountInputProps {
  /** Most fraction digits accepted; unlimited when omitted (crypto quantities). */
  decimals?: number
  allowNegative?: boolean
  placeholder?: string
  disabled?: boolean
  id?: string
  /** `lg` — the one big sum of a form (limit, goal): taller, bigger digits. */
  size?: 'default' | 'lg'
}
