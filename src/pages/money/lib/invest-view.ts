import { formatMoney } from '@/shared/lib/format'

/** «5 784 $», «22,50 $», «+2 293 $»: whole sums drop the cents, as on the canvas. */
export function investMoney(value: string, currency: string, signed = false): string {
  const number = Number(value)
  const decimals = Number.isInteger(number) || Math.abs(number) >= 1000 ? 0 : 2
  return `${signed && number > 0 ? '+' : ''}${formatMoney(number, { decimals, currency })}`
}
