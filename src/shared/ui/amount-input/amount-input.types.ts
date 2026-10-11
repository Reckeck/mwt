export interface AmountInputProps {
  /** Most fraction digits accepted; unlimited when omitted (crypto quantities). */
  decimals?: number
  allowNegative?: boolean
  placeholder?: string
  disabled?: boolean
  id?: string
}
