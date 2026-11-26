import { fiatRef } from '@/shared/lib/asset-ref'

/** Wallet kinds that hold no basket to earmark: a home, points and loans. */
const NO_ENVELOPE_KINDS: readonly StoreKind[] = ['property', 'loyalty', 'liability']

export const hasEnvelopes = (store: Pick<Store, 'kind'>): boolean => !NO_ENVELOPE_KINDS.includes(store.kind)

/** Assets an envelope may earmark: the binding, else what the store holds, else the base currency. */
export function envelopeAssets(store: Pick<Store, 'binding'>, held: AssetRef[], baseCurrency: string): AssetRef[] {
  const binding = store.binding
  if (binding !== 'any' && 'asset' in binding) return [binding.asset]
  if (binding !== 'any' && 'pair' in binding) return [binding.pair.base, binding.pair.quote]
  return held.length ? held : [fiatRef(baseCurrency)]
}
