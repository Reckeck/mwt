/** Which way money went, for the colour rule (D7): spent, earned, or only moved between own wallets. */
export type MoneyFlow = 'expense' | 'income' | 'move'

/** Text colour per flow: expense is ink, income is the positive green, a move is muted. */
export const MONEY_FLOW_CLASS: Record<MoneyFlow, string> = {
  expense: 'text-foreground',
  income: 'text-success',
  move: 'text-muted-foreground',
}

/** The flow of a signed figure: out is an expense, in is income; zero and non-numbers are a move. */
export function flowOfAmount(amount: string | number | null | undefined): MoneyFlow {
  const numeric = typeof amount === 'string' ? Number(amount) : amount
  if (numeric === null || numeric === undefined || !Number.isFinite(numeric) || numeric === 0) return 'move'
  return numeric > 0 ? 'income' : 'expense'
}

/** The figure to format: an expense always carries a minus, income reads as positive, a move has no sign. */
export function flowAmount(amount: string | number | null | undefined, flow: MoneyFlow): number | null {
  const numeric = typeof amount === 'string' ? Number(amount) : amount
  if (numeric === null || numeric === undefined || !Number.isFinite(numeric)) return null
  const magnitude = Math.abs(numeric)
  return flow === 'expense' ? -magnitude : magnitude
}
