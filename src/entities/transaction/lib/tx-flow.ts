import { type MoneyFlow, flowOfAmount } from '@/shared/lib/money-flow'

/** D7: a move or an exchange is muted without a sign; everything else is ink out, green in. */
export function txLegFlow(kind: TxKind, ownMove: boolean, amount: string): MoneyFlow {
  return ownMove || kind === 'transfer' || kind === 'convert' ? 'move' : flowOfAmount(amount)
}
