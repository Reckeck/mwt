/** Card payment systems told apart by the first digits; the name is the same in every language but «Мир». */
export type CardBrand = 'mir' | 'visa' | 'mastercard' | 'unionpay' | 'amex' | 'jcb'

/** The payment system of a card number, or `null` when the digits say nothing. */
export function cardBrand(value: string): CardBrand | null {
  const digits = value.replace(/\D/g, '')
  if (/^220[0-4]/.test(digits)) return 'mir'
  if (/^4/.test(digits)) return 'visa'
  if (/^(5[1-5]|2[2-7])/.test(digits)) return 'mastercard'
  if (/^62/.test(digits)) return 'unionpay'
  if (/^3[47]/.test(digits)) return 'amex'
  if (/^35/.test(digits)) return 'jcb'
  return null
}

/**
 * A requisite as the list shows it: enough to recognise, not the whole number
 * («5536 9137 •• 4417», «+7 916 •• 12-34», «TQ7xkN3v…F2s9fKp»). Copy takes the
 * full value.
 */
export function maskDetail(kind: DetailKind, value: string): string {
  const compact = value.replace(/[\s-]/g, '')
  if (kind === 'card' && compact.length >= 12) {
    return `${compact.slice(0, 4)} ${compact.slice(4, 8)} •• ${compact.slice(-4)}`
  }
  if (kind === 'phone' && compact.length >= 10) {
    const digits = compact.replace(/\D/g, '')
    const country = digits.slice(0, digits.length - 10)
    return `+${country} ${digits.slice(-10, -7)} •• ${digits.slice(-4, -2)}-${digits.slice(-2)}`
  }
  if ((kind === 'iban' || kind === 'account') && compact.length > 12) {
    return `${compact.slice(0, 4)} •• ${compact.slice(-4)}`
  }
  if (kind === 'crypto_address' && value.length > 18) return `${value.slice(0, 8)}…${value.slice(-7)}`
  return value
}

/** How many requisites are in use and how many are kept only for old operations. */
export function accountCounts(accounts: ContactAccount[]): { active: number; inactive: number } {
  const active = accounts.filter((account) => account.active).length
  return { active, inactive: accounts.length - active }
}
