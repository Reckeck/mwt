/** The debt effect of a quick operation: only a move out can lend; a plain one carries none. */
export function quickDebtEffect(
  kind: 'payment' | 'topup' | 'transfer' | 'convert',
  asDebt: boolean,
): DebtEffect | null {
  return kind === 'transfer' && asDebt ? 'lend' : null
}
