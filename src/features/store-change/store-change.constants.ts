/** Kinds a wallet may be changed between: they need no data of their own. */
export const CONVERTIBLE_KINDS: StoreKind[] = ['bank', 'crypto', 'exchange']

/** Kinds `current` can become. */
export function convertibleKinds(current: StoreKind): StoreKind[] {
  return CONVERTIBLE_KINDS.filter((entry) => entry !== current)
}
