export interface MoneyCardProps {
  /** Signed: below zero the card reads «Отдали», above «Получили». */
  amount: number
  /** The sum's currency (lower or upper case). */
  currency: string
  /** The operation's moment, RFC 3339; empty for a balance or a total of now. */
  at: string
}
