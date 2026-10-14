import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { call } from '@/shared/api/ipc'
import { useTradeLookupStore } from '@/shared/store/tradeLookup'

import type { TxFormState } from './use-tx-form-state'

/** Text fields an assistant may fill. */
type FillableField =
  | 'feeAmount'
  | 'price'
  | 'amount'
  | 'comment'
  | 'soldQty'
  | 'boughtQty'
  | 'symbol'
  | 'quantity'
  | 'fillPrice'

/**
 * Assistants that fill a form in, never over typed values: the on-chain hash
 * inspection and the recent Bybit fills of an exchange wallet with a key.
 */
export function useTxFills(state: TxFormState) {
  const { t } = useI18n()
  const lookup = useTradeLookupStore()
  const { props, form, isCrypto, storeKind } = state

  function fillIfEmpty(key: FillableField, value: string | null | undefined) {
    if (!value?.trim() || form[key].trim()) return
    form[key] = value.trim()
  }

  let hashTimer: ReturnType<typeof setTimeout> | undefined
  watch(
    () => form.txHash,
    (value) => {
      if (hashTimer) clearTimeout(hashTimer)
      const hash = value.trim()
      if (!hash) {
        lookup.reset()
        return
      }
      hashTimer = setTimeout(() => void lookup.inspectTx(hash, form.network || undefined), 600)
    },
  )

  watch(
    () => lookup.hints,
    (hints) => {
      for (const hint of hints) {
        if (hint.field === 'fee') fillIfEmpty('feeAmount', hint.value)
        if (hint.field === 'price' && !form.price.trim()) {
          form.price = hint.value
          form.priceCurrency = 'usd'
        }
        if (hint.field === 'qtyBought' || hint.field === 'qtySold') fillIfEmpty('amount', hint.value)
      }
    },
    { deep: true },
  )

  const lookupHint = computed(() => {
    if (lookup.loading) return t('tx.lookup.checking')
    if (lookup.notice) return t(`tx.lookup.${lookup.notice}`)
    return null
  })

  /** On-chain fields: crypto wallets for topup/payment, a transfer of a non-fiat asset, DeFi. */
  const showChain = computed(() => {
    if (props.template) return false
    if (form.kind === 'defi') return true
    if (form.kind === 'transfer') return form.asset?.chain !== 'fiat'
    return isCrypto.value && ['topup', 'payment'].includes(form.kind)
  })

  // The fills list needs to know whether this exchange wallet (or a parent) has a key.
  watch(
    storeKind,
    (kind) => {
      if (kind === 'exchange') void lookup.loadKeyStatus(props.storeId)
    },
    { immediate: true },
  )

  const bybitCategory = ref<BybitCategory>('spot')
  const showFills = computed(() => lookup.hasKey && (form.kind === 'trade_spot' || form.kind === 'trade_futures'))
  watch(
    showFills,
    (shown) => {
      if (shown) void lookup.loadExecutions(form.kind === 'trade_futures' ? 'linear' : bybitCategory.value)
    },
    { immediate: true },
  )

  async function resolveAsset(symbol: string | null | undefined): Promise<AssetRef | null> {
    if (!symbol) return null
    const hits = await call<AssetSearchHit[]>('search_assets', { query: symbol })
    const hit = hits.find((item) => item.symbol.toUpperCase() === symbol.toUpperCase()) ?? hits[0]
    if (!hit) return null
    const upper = hit.symbol.toUpperCase()
    return {
      symbol: upper,
      name: hit.name || upper,
      coingecko_id: hit.coingecko_id,
      chain: hit.chain ?? null,
      contract: hit.contract ?? null,
      quote: upper,
    }
  }

  /** Apply a Bybit fill to the form. */
  async function applyFill(exec: BybitExecution) {
    const fill = exec.prefill
    fillIfEmpty('comment', fill.comment)
    if (!form.timestamp && fill.occurred_at) form.timestamp = fill.occurred_at.slice(0, 16)
    fillIfEmpty('feeAmount', fill.fee)
    if (form.kind === 'trade_futures') {
      fillIfEmpty('symbol', exec.symbol)
      fillIfEmpty('quantity', exec.exec_qty)
      fillIfEmpty('fillPrice', exec.exec_price)
      form.direction = exec.side.toLowerCase() === 'buy' ? 'long' : 'short'
      return
    }
    fillIfEmpty('soldQty', fill.qty_sold)
    fillIfEmpty('boughtQty', fill.qty_bought)
    // Exchange fills are quoted in USD(T).
    if (!form.price.trim() && fill.price) {
      form.price = fill.price
      form.priceCurrency = 'usd'
    }
    form.liquidity = exec.is_maker ? 'maker' : 'taker'
    const [sold, bought] = await Promise.all([resolveAsset(fill.asset_sold), resolveAsset(fill.asset_bought)])
    if (!form.soldAsset && sold) form.soldAsset = sold
    if (!form.boughtAsset && bought) form.boughtAsset = bought
  }

  return { lookup, lookupHint, showChain, bybitCategory, showFills, applyFill }
}
