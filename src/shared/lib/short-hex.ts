/** `0x9a1c…4f02`: a hex address or contract where there is no room for all of it. */
export function shortHex(value: string): string {
  return value.length <= 12 ? value : `${value.slice(0, 6)}…${value.slice(-4)}`
}
