/**
 * What a connected address holds in the browser mock (spec E16). The mock derives positions from ledger
 * transactions, which an address has none of (the engine's operations are not ledger records), so a wallet led
 * by the module gets these instead. They follow the hand-made head of `crypto-operations-seed.ts`
 * (the ten thousand rows behind it are size filler, not a story): ETH 0.35 opened, -0.004 reconciled, -0.042 sent,
 * +0.25 received, -0.5 swapped, minus 0.0066 of gas = 0.0474; USDT +500 received. Quotes come from `prices.ts`.
 */

/** A holding of one connected address: the asset, how many, and the average cost in USD. */
export interface EngineHolding {
  asset: AssetRef
  quantity: string
  avgCost: string
}

export const ENGINE_HOLDINGS: EngineHolding[] = [
  {
    asset: {
      symbol: 'ETH',
      name: 'Ethereum',
      coingecko_id: 'ethereum',
      chain: 'ethereum',
      contract: null,
      quote: 'eth',
    },
    quantity: '0.0474',
    avgCost: '2850',
  },
  {
    asset: {
      symbol: 'USDT',
      name: 'Tether USD',
      coingecko_id: 'tether',
      chain: 'ethereum',
      contract: '0xdac17f958d2ee523a2206206994597c13d831ec7',
      quote: 'usdt',
    },
    quantity: '500',
    avgCost: '1',
  },
]
