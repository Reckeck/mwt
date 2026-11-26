import { txMeta } from '@/shared/lib/transactions'

/** Brought in from an exchange; a correction has no categories, so it gets the mark instead. */
export function txIsImported(tx: Transaction): boolean {
  const { origin, categories } = txMeta(tx)
  return typeof origin === 'object' && 'import' in origin && !categories.includes('exchange_import')
}
