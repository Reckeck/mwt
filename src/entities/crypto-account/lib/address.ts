/** `0x7a3F…9bC1`: an address where there is no room for all of it. */
export function shortAddress(address: string): string {
  return address.length <= 12 ? address : `${address.slice(0, 6)}…${address.slice(-4)}`
}

/** `0x` and 40 hex digits: the shape of an EVM address. */
export function isEvmAddress(address: string): boolean {
  return /^0x[0-9a-fA-F]{40}$/.test(address.trim())
}

/** `YYYY-MM-DD` that is a real day since the first Bitcoin block year. */
export function isStartDate(value: string): boolean {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value)
  if (!match) return false
  const [, year, month, day] = match.map(Number)
  const date = new Date(Date.UTC(year, month - 1, day))
  return year >= 2009 && date.getUTCMonth() === month - 1 && date.getUTCDate() === day
}
