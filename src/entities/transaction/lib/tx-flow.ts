import { type MoneyFlow, flowOfAmount } from '@/shared/lib/money-flow'

/**
 * D7: a move between own wallets or an exchange is muted without a sign; everything else is ink out, green in.
 * In one wallet's history (`scoped`) the row shows only that wallet's side, so a move keeps its direction sign
 * (the trade-off against D7: there it is the only way to tell «left this wallet» from «came into it»).
 */
export function txLegFlow(kind: TxKind, ownMove: boolean, amount: string, scoped = false): MoneyFlow {
  if (scoped) return flowOfAmount(amount)
  return ownMove || kind === 'convert' ? 'move' : flowOfAmount(amount)
}
