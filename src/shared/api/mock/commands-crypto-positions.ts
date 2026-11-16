/**
 * Commands: the DeFi positions of an address (spec D0).
 *
 * The numbers are the reference examples of the Rust side
 * (`crates/core/src/cryptotrack/protocols/fees.rs`), so the screen is tried on the same arithmetic the
 * application computes: a pool whose impermanent loss is already inside its value change, a supply with a
 * debt, a staking that accrues on its own balance, a position behind a hook whose fees nobody could read
 * (absent, not zero) and a closed one, frozen at a real zero.
 */

import { type MockCommands, resolve } from './core'
import { ACCOUNT } from './crypto-operations-seed'

const ETH = 'ethereum|native'
const USDC = 'ethereum|0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48'
const PEPE = 'ethereum|0x6982508145454ce325ddbe47a25d4ec3d2311933'
const STETH = 'ethereum|0xae7ab96520de3a18e5e111b5eaab095312d7fe84'

/** The parts every row shares, so each position below says only what is its own. */
const base = {
  account_id: ACCOUNT,
  chain: 'ethereum' as Chain,
  approximate: false,
  updated_at: '2026-11-16T09:40:00Z',
}

const positions: CryptoPositionView[] = [
  {
    ...base,
    id: 'uniswap-v3|ethereum|123456',
    protocol: 'uniswap-v3',
    name: 'ETH / USDC 0.05%',
    kind: 'pool',
    status: 'open',
    assets: [
      { asset_key: ETH, symbol: 'ETH', direction: 'in', quantity: '0.7071067811865475', value_usd: '2828.4271' },
      { asset_key: USDC, symbol: 'USDC', direction: 'in', quantity: '2828.4271247461902', value_usd: '2828.4271' },
    ],
    invested_usd: '4000',
    put_in_usd: '4000',
    value_usd: '5656.8542',
    fees_collected_usd: '120',
    fees_uncollected_usd: '30',
    il_usd: '-343.1458',
    il_ratio: '-0.057191',
    gas_usd: '18.5',
    net_usd: '1788.3542',
    apr: '2.235443',
    opened_at: '2026-03-01T10:00:00Z',
    tracking: { position: { protocol: 'uniswap-v3', chain: 'ethereum', id: '123456' } },
  },
  {
    ...base,
    id: 'aave-v3|ethereum|usdc-eth',
    protocol: 'aave-v3',
    name: 'USDC supply, ETH debt',
    kind: 'lending',
    status: 'open',
    assets: [
      { asset_key: USDC, symbol: 'USDC', direction: 'in', quantity: '10000', value_usd: '10000' },
      { asset_key: ETH, symbol: 'ETH', direction: 'out', quantity: '1', value_usd: '4000' },
    ],
    invested_usd: '5000',
    put_in_usd: '5000',
    // The collateral minus the debt, and nothing to compare against holding.
    value_usd: '6000',
    accrued_usd: '240',
    fees_collected_usd: '0',
    fees_uncollected_usd: '0',
    gas_usd: '12.4',
    net_usd: '987.6',
    apr: '0.987600',
    opened_at: '2026-03-01T10:00:00Z',
    tracking: { position: { protocol: 'aave-v3', chain: 'ethereum', id: 'usdc-eth' } },
  },
  {
    ...base,
    id: 'lido|ethereum|steth',
    protocol: 'lido',
    name: 'stETH',
    kind: 'staking',
    status: 'open',
    assets: [{ asset_key: STETH, symbol: 'STETH', direction: 'in', quantity: '2.0412', value_usd: '8164.8' }],
    invested_usd: '8000',
    put_in_usd: '8000',
    value_usd: '8164.8',
    accrued_usd: '164.8',
    fees_collected_usd: '0',
    fees_uncollected_usd: '0',
    gas_usd: '6.2',
    net_usd: '158.6',
    apr: '0.099125',
    opened_at: '2026-03-01T10:00:00Z',
    tracking: { position: { protocol: 'lido', chain: 'ethereum', id: 'steth' } },
  },
  {
    ...base,
    id: 'uniswap-v4|ethereum|7788',
    protocol: 'uniswap-v4',
    name: 'ETH / USDC, hook',
    kind: 'pool',
    status: 'open',
    // A hook decides what this position owes, so the adapter says the numbers
    // are an approximation and reports no fees at all: the screen must print a
    // dash for every figure that depends on them, never a zero.
    approximate: true,
    assets: [
      { asset_key: ETH, symbol: 'ETH', direction: 'in', quantity: '0.5', value_usd: '2000' },
      { asset_key: USDC, symbol: 'USDC', direction: 'in', quantity: '2000', value_usd: '2000' },
    ],
    invested_usd: '3800',
    put_in_usd: '3800',
    value_usd: '4000',
    fees_collected_usd: '0',
    il_usd: '-56.2',
    il_ratio: '-0.013855',
    gas_usd: '9.1',
    opened_at: '2026-06-01T10:00:00Z',
    tracking: { position: { protocol: 'uniswap-v4', chain: 'ethereum', id: '7788' } },
  },
  {
    ...base,
    id: 'uniswap-v3|ethereum|99001',
    protocol: 'uniswap-v3',
    name: 'ETH / USDT 0.3%',
    kind: 'pool',
    status: 'closed',
    assets: [],
    invested_usd: '-310.5',
    // Ever put in 1 000, 1 310.50 taken back out: what is still inside is negative, and put in stays 1 000.
    put_in_usd: '1000',
    // Nothing is left in it: a real zero, and the row adds nothing to the capital.
    value_usd: '0',
    fees_collected_usd: '86.4',
    fees_uncollected_usd: '0',
    gas_usd: '21.9',
    net_usd: '375',
    apr: '0.456204',
    opened_at: '2026-01-12T10:00:00Z',
    closed_at: '2026-04-30T18:20:00Z',
    updated_at: '2026-04-30T18:20:00Z',
    tracking: { position: { protocol: 'uniswap-v3', chain: 'ethereum', id: '99001' } },
  },
  {
    ...base,
    id: 'uniswap-v3|ethereum|51077',
    protocol: 'uniswap-v3',
    name: 'PEPE / WETH 1%',
    kind: 'pool',
    status: 'open',
    // No price source for PEPE: everything that needs a price is absent (a dash), while what was put in and the
    // gas were fixed on the day of the deal and stay exact.
    assets: [
      { asset_key: PEPE, symbol: 'PEPE', direction: 'in', quantity: '1240000' },
      { asset_key: ETH, symbol: 'WETH', direction: 'in', quantity: '0.21', value_usd: '840' },
    ],
    put_in_usd: '1900',
    invested_usd: '1900',
    gas_usd: '21.4',
    opened_at: '2026-09-07T10:00:00Z',
    tracking: { position: { protocol: 'uniswap-v3', chain: 'ethereum', id: '51077' } },
  },
]

export const cryptoPositionsCommands: MockCommands = {
  crypto_list_positions: () => resolve(positions),
}
