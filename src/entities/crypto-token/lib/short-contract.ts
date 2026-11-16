/** `0x9a1c…4f02`: a contract where there is no room for all of it. */
export function shortContract(contract: string): string {
  return contract.length <= 12 ? contract : `${contract.slice(0, 6)}…${contract.slice(-4)}`
}
