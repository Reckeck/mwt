export interface PnlTextProps {
  /** The change in money, a decimal string from Rust. */
  amount: string | number | null | undefined
  /** Lowercase code; the base currency when omitted. */
  currency?: string
  decimals?: number
  /** The same change in percent, shown after the amount in brackets. */
  pct?: string | number | null
}
