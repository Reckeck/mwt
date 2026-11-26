import { type QuickFormState, QUICK_KINDS, type QuickKind, emptyQuickForm } from '@/features/transaction-create'
import type { QuickAddRequest } from '@/shared/lib/quick-add'

const LAST_STORE_KEY = 'quick-add.store'

/** The wallet used last time; private mode remembers nothing. */
export function rememberedStore(): string {
  try {
    return localStorage.getItem(LAST_STORE_KEY) ?? ''
  } catch {
    return ''
  }
}

export function rememberStore(storeId: string) {
  try {
    localStorage.setItem(LAST_STORE_KEY, storeId)
  } catch {
    // Private mode: the choice is simply not remembered.
  }
}

/** The form an opening starts on: what the caller asked for, else the remembered wallet, else the first one. */
export function startForm(request: QuickAddRequest | null, walletIds: string[], now: string): QuickFormState {
  const kind = QUICK_KINDS.find((candidate): candidate is QuickKind => candidate === request?.kind) ?? 'payment'
  const pick = (id: string | undefined) => (id && walletIds.includes(id) ? id : '')
  const storeId = pick(request?.storeId) || pick(rememberedStore()) || walletIds[0] || ''
  return {
    ...emptyQuickForm(kind, storeId),
    toStoreId: pick(request?.toStoreId),
    category: request?.category ?? '',
    amount: request?.amount ?? '',
    timestamp: now,
  }
}
