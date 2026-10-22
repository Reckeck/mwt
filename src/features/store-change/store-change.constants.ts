import { isCryptoKind } from '@/shared/lib/store-tree'

/** Kinds a wallet may be changed between: they need no data of their own. */
export const CONVERTIBLE_KINDS: StoreKind[] = ['bank', 'crypto', 'exchange']

/** Kinds `current` can become; crypto ones only while the vault keeps crypto. */
export function convertibleKinds(current: StoreKind, crypto: boolean): StoreKind[] {
  return CONVERTIBLE_KINDS.filter((entry) => entry !== current && (crypto || !isCryptoKind(entry)))
}
