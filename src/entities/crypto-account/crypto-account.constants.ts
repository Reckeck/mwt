/** The networks an address can be followed on: the ones the module reads (spec E6). Proper names, not translated. */
const CONNECTABLE_NETWORKS: { chain: Chain; label: string }[] = [
  { chain: 'ethereum', label: 'Ethereum' },
  { chain: 'arbitrum', label: 'Arbitrum' },
  { chain: 'optimism', label: 'Optimism' },
  { chain: 'base', label: 'Base' },
  { chain: 'polygon', label: 'Polygon' },
  { chain: 'bsc', label: 'BNB Smart Chain' },
]

export function networkLabel(chain: Chain): string {
  return CONNECTABLE_NETWORKS.find((entry) => entry.chain === chain)?.label ?? chain
}

/** The source of a plain address: the network's public explorer. */
export const DEFAULT_SOURCE_LABEL = 'Blockscout'
