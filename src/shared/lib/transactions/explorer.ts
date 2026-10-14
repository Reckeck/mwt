/** The on-chain side: chain and hash, and the explorer page for them. */

import { body, txKind } from './access'

/** Chain and hash for the explorer link, when the variant has them. */
export function txChainAndHash(tx: Transaction): { chain: Chain | null; hash: string | null } {
  switch (txKind(tx)) {
    case 'topup': {
      const { details } = body(tx, 'topup')
      return { chain: details.network, hash: details.tx_hash }
    }
    case 'payment': {
      const { details } = body(tx, 'payment')
      return { chain: details.network, hash: details.tx_hash }
    }
    case 'transfer': {
      const record = body(tx, 'transfer')
      return { chain: record.network, hash: record.tx_hash }
    }
    case 'defi': {
      const record = body(tx, 'defi')
      return { chain: record.network, hash: record.tx_hash }
    }
    default:
      return { chain: null, hash: null }
  }
}

const EXPLORERS: Partial<Record<Chain, (hash: string) => string>> = {
  bitcoin: (hash) => `https://mempool.space/tx/${hash}`,
  ethereum: (hash) => `https://etherscan.io/tx/${hash}`,
  bsc: (hash) => `https://bscscan.com/tx/${hash}`,
  polygon: (hash) => `https://polygonscan.com/tx/${hash}`,
  arbitrum: (hash) => `https://arbiscan.io/tx/${hash}`,
  optimism: (hash) => `https://optimistic.etherscan.io/tx/${hash}`,
  base: (hash) => `https://basescan.org/tx/${hash}`,
  avalanche: (hash) => `https://snowtrace.io/tx/${hash}`,
  solana: (hash) => `https://solscan.io/tx/${hash}`,
  tron: (hash) => `https://tronscan.org/#/transaction/${hash}`,
}

/** Explorer page of the on-chain hash; mirrors `Transaction::explorer_url`. */
export function txExplorerUrl(tx: Transaction): string | null {
  const { chain, hash } = txChainAndHash(tx)
  if (!chain || !hash) return null
  return EXPLORERS[chain]?.(hash) ?? null
}
