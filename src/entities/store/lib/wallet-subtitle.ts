/** `Bank · in the budget`: the kind, and whether the budget counts it. */
export function walletSubtitle(store: Store, t: (key: string) => string): string {
  const kind = t(`stores.kind.${store.kind}`)
  return store.purpose === 'everyday' && store.kind !== 'liability' ? `${kind} · ${t('purpose.inBudgetBadge')}` : kind
}
