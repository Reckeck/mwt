export interface MoneyConversionsProps {
  amount: number
  /** The amount's currency (lower or upper case). */
  currency: string
  /** The operation's moment, RFC 3339: rates of that day. Empty: the latest cached. */
  at?: string
}
