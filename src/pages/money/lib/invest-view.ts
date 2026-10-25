/** The sum of an investment figure for `<Money v-bind>`: whole sums drop the cents, as on the canvas. */
export function investMoney(value: string, currency: string, signed = false) {
  const number = Number(value)
  const decimals = Number.isInteger(number) || Math.abs(number) >= 1000 ? 0 : 2
  return { amount: number, currency, decimals, signed }
}
