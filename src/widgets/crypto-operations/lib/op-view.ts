import type { Component } from 'vue'
import {
  ArrowDownLeft,
  ArrowLeftRight,
  ArrowUpRight,
  CircleCheck,
  CircleHelp,
  Gift,
  Layers,
  Lock,
  LockOpen,
  Split,
} from 'lucide-vue-next'

import { CHAIN_LABELS } from '@/shared/lib/chains'

/** The square mark of an operation's kind; the direction decides it for a plain transfer. */
export function cryptoOpIcon(row: Pick<CryptoOpRow, 'kind' | 'legs'>): Component {
  switch (row.kind) {
    case 'swap':
      return ArrowLeftRight
    case 'approve':
      return CircleCheck
    case 'stake':
      return Lock
    case 'unstake':
      return LockOpen
    case 'claim':
      return Gift
    case 'bridge':
      return Split
    case 'contract':
      return Layers
    case 'unknown':
      return CircleHelp
    default:
      return row.legs[0]?.direction === 'in' ? ArrowDownLeft : ArrowUpRight
  }
}

/** The signed amount of a leg for `<Money>`: money in is positive, money out is negative. */
export const legAmount = (leg: CryptoOpLeg): string => (leg.direction === 'out' ? `-${leg.amount}` : leg.amount)

/** `0x9f3c…1a71`: an address or a hash short enough for a line, whole in the card. */
export function shortAddress(address: string): string {
  return address.length > 12 ? `${address.slice(0, 6)}…${address.slice(-4)}` : address
}

/** What the network label of a row says: the exchange's name for a fill, the network's for a chain operation. */
export function opPlace(row: Pick<CryptoOpRow, 'network' | 'exchange'>): string {
  if (row.exchange) return row.exchange.charAt(0).toUpperCase() + row.exchange.slice(1)
  return row.network ? CHAIN_LABELS[row.network] : ''
}

/** The title of a row: «Received USDT», «Sent ETH», «Swap ETH → USDT», «Contract approval». */
export function opTitle(
  row: Pick<CryptoOpRow, 'kind' | 'legs'>,
  t: (key: string, params?: Record<string, unknown>) => string,
): string {
  const moved = row.legs.find((leg) => leg.direction === 'in') ?? row.legs[0]
  const out = row.legs.find((leg) => leg.direction === 'out')
  const inn = row.legs.find((leg) => leg.direction === 'in')
  const symbol = moved?.symbol ?? ''
  if (row.kind === 'swap' && out && inn)
    return t('cryptoTrack.operations.rowTitle.swapPair', { from: out.symbol, to: inn.symbol })
  if (row.kind === 'transfer' || row.kind === 'swap') {
    return t(
      moved?.direction === 'in' ? 'cryptoTrack.operations.rowTitle.received' : 'cryptoTrack.operations.rowTitle.sent',
      { symbol },
    )
  }
  return t(`cryptoTrack.operations.rowTitle.${row.kind}`, { symbol })
}
