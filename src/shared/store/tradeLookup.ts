import { defineStore } from 'pinia'
import { computed, ref } from 'vue'

import { call } from '@/shared/api/ipc'

// IPC payload types are ambient globals generated into `src/lib/ipc.gen.d.ts`.

/** Why a lookup produced nothing. Drives a quiet inline hint, never a toast. */
export type LookupNotice = 'empty' | 'unavailable' | 'choose_network' | 'no_fields'
export type KeyNotice = 'no_key' | 'unreachable' | 'rate_limited' | 'forbidden_region' | null

/**
 * Best-effort trade enrichment. Nothing here ever blocks saving a
 * transaction: every failure resolves to "no suggestions" so the form keeps
 * whatever the user typed.
 */
export const useTradeLookupStore = defineStore('tradeLookup', () => {
  const hints = ref<TxFieldHint[]>([])
  const notice = ref<LookupNotice | null>(null)
  const loading = ref(false)
  const explorerUrl = ref<string | null>(null)

  const hasSuggestions = computed(() => hints.value.length > 0)

  function reset() {
    hints.value = []
    notice.value = null
    explorerUrl.value = null
  }

  function apply(result: InspectTxResult) {
    // Never overwrite form state with nothing: an empty result is a no-op.
    if (!result.fetched || result.hints.length === 0) {
      notice.value = (result.notice as LookupNotice) ?? 'unavailable'
      return false
    }
    hints.value = result.hints
    notice.value = null
    explorerUrl.value = result.info?.explorer_url ?? null
    return true
  }

  async function inspectTx(txHash: string, network?: string) {
    const hash = txHash.trim()
    if (!hash) {
      reset()
      return
    }
    loading.value = true
    try {
      const res = await call<InspectTxResult>('inspect_tx', {
        input: { tx_hash: hash, network: network ?? null },
      })
      apply(res)
    } catch {
      // Offline, locked vault, or provider down: stay silent and non-blocking.
      reset()
    } finally {
      loading.value = false
    }
  }

  // --- Bybit -----------------------------------------------------------------

  const executions = ref<BybitExecution[]>([])
  const keyNotice = ref<KeyNotice>(null)
  const key = ref<ExchangeKeyStatus>({ configured: false, hint: null, spot_store_id: null, futures_store_id: null })
  const executionsLoading = ref(false)

  const hasKey = computed(() => key.value.configured)
  /** Wallet whose key the fills come from; the backend also looks up its ancestors. */
  const storeId = ref<string | null>(null)

  async function loadKeyStatus(forStore: string) {
    storeId.value = forStore
    try {
      key.value = await call<ExchangeKeyStatus>('exchange_key_status', { input: { store_id: forStore } })
    } catch {
      key.value = { configured: false, hint: null, spot_store_id: null, futures_store_id: null }
    }
  }

  async function saveKey(input: SetExchangeKeyInput) {
    key.value = await call<ExchangeKeyStatus>('set_exchange_key', { input })
  }

  async function clearKey() {
    if (!storeId.value) return
    key.value = await call<ExchangeKeyStatus>('clear_exchange_key', { input: { store_id: storeId.value } })
    executions.value = []
    keyNotice.value = null
  }

  async function loadExecutions(category: BybitCategory = 'spot', symbol?: string) {
    executionsLoading.value = true
    try {
      const res = await call<BybitExecutionsResult>('bybit_executions', {
        input: { category, symbol: symbol ?? null, limit: 20, store_id: storeId.value },
      })
      executions.value = res.executions
      keyNotice.value = res.notice as KeyNotice
    } catch {
      executions.value = []
      keyNotice.value = 'unreachable'
    } finally {
      executionsLoading.value = false
    }
  }

  return {
    hints,
    notice,
    loading,
    explorerUrl,
    hasSuggestions,
    inspectTx,
    reset,
    executions,
    executionsLoading,
    keyNotice,
    key,
    hasKey,
    loadKeyStatus,
    saveKey,
    clearKey,
    loadExecutions,
  }
})
