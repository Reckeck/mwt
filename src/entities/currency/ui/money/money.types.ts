export interface MoneyProps {
  amount: string | number | null | undefined
  /** Lowercase code; the base currency when omitted. */
  currency?: string
  decimals?: number
  signed?: boolean
  compact?: boolean
  /** Show the conversion tooltip on hover; on by default for amounts in the base currency. */
  convert?: boolean
}
