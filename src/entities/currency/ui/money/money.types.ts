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
  /** The operation's moment (RFC 3339): the card shows the base currency at that day's rate. */
  at?: string
}
