export interface MoneyProps {
  amount: string | number | null | undefined
  /** Lowercase code; the base currency when omitted. */
  currency?: string
  decimals?: number
  signed?: boolean
  /** Green up, red down, neutral for zero; implies `signed`, so colour is never the only cue. */
  colored?: boolean
  compact?: boolean
  /** Show the conversion tooltip on hover; on by default for the base currency, fiat and favourites. */
  convert?: boolean
  /** Only the number, no currency mark: the first of a pair «36 из 50 $». The card and tooltip stay. */
  bare?: boolean
  /** The operation's moment (RFC 3339) or a month `YYYY-MM` (its first day): the card shows the base currency at that day's rate. */
  at?: string
  /** A quick glance the screen points at: the dotted underline is always there, not only on hover. */
  glance?: boolean
}
