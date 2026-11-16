import { shortAddress } from './address'

/** How the backend compares wallet names among siblings: trimmed, case-insensitive. */
const nameKey = (name: string) => name.trim().toLowerCase()

/** `base`, or `base (2)`, `base (3)`… when the name is already taken (`STORE_NAME_TAKEN` is refused by the backend). */
export function uniqueWalletName(base: string, taken: readonly string[]): string {
  const used = new Set(taken.map(nameKey))
  if (!used.has(nameKey(base))) return base
  let suffix = 2
  while (used.has(nameKey(`${base} (${suffix})`))) suffix += 1
  return `${base} (${suffix})`
}

/** `Ethereum · 0x7a3F…9bC1`: the name an address gets until the user types their own; the network alone while there is no address yet. */
export function defaultWalletName(networkName: string, address: string, taken: readonly string[]): string {
  const trimmed = address.trim()
  const base = trimmed ? `${networkName} · ${shortAddress(trimmed)}` : networkName
  return uniqueWalletName(base, taken)
}
