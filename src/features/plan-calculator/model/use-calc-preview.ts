import { computed, onScopeDispose, ref, watch } from 'vue'

import { call, parseError } from '@/shared/api/ipc'
import { numberFormat } from '@/shared/lib/format'

/**
 * The calculator's fields and its live result: `calc_preview` runs 200 ms
 * after the last change; picking an asset pre-fills its cached price.
 */
export function useCalcPreview() {
  const mode = ref<CalcMode>('converter')
  const from = ref<AssetRef | null>(null)
  const to = ref<AssetRef | null>(null)
  const amount = ref('')
  const priceIn = ref('')
  const priceOut = ref('')
  const targetPrice = ref('')
  const stopPrice = ref('')
  const feePct = ref('0.1')

  const result = ref<CalcResult | null>(null)
  const error = ref<string | null>(null)

  async function run() {
    if (!amount.value.trim() || !priceIn.value.trim()) {
      result.value = null
      return
    }
    try {
      result.value = await call<CalcResult>('calc_preview', {
        input: {
          mode: mode.value,
          from_symbol: from.value?.symbol ?? '',
          to_symbol: to.value?.symbol ?? '',
          amount: amount.value.trim(),
          price_in: priceIn.value.trim(),
          price_out: priceOut.value.trim(),
          target_price: targetPrice.value.trim() || null,
          stop_price: stopPrice.value.trim() || null,
          fee_pct: feePct.value.trim() || '0',
        },
      })
      error.value = null
    } catch (err) {
      result.value = null
      error.value = parseError(err).message
    }
  }

  let timer: ReturnType<typeof setTimeout> | undefined
  watch(
    () =>
      [mode.value, amount.value, priceIn.value, priceOut.value, targetPrice.value, stopPrice.value, feePct.value].join(
        '|',
      ),
    () => {
      clearTimeout(timer)
      timer = setTimeout(() => void run(), 200)
    },
    { immediate: true },
  )
  onScopeDispose(() => clearTimeout(timer))

  /** Pre-fill the entry price from the price cache when an asset is picked. */
  async function autofillPrice(target: AssetRef | null, into: 'in' | 'out') {
    if (!target) return
    try {
      const res = await call<CachedPrice>('get_cached_price', {
        input: {
          symbol: target.symbol,
          coingecko_id: target.coingecko_id,
          chain: target.chain,
          contract: target.contract,
          at: null,
        },
      })
      if (res.ts) {
        if (into === 'in') priceIn.value = res.price_usd
        else priceOut.value = res.price_usd
      }
    } catch {
      // no cached quote — leave the field for manual input
    }
  }
  watch(from, (value) => void autofillPrice(value, 'in'))
  watch(to, (value) => void autofillPrice(value, 'out'))

  /** 1 unit of the source priced in the target, when both prices are known. */
  const impliedRate = computed(() => {
    const priceInUsd = Number(priceIn.value)
    const priceOutUsd = Number(priceOut.value)
    if (!priceInUsd || !priceOutUsd) return null
    return numberFormat({ maximumFractionDigits: 8 }).format(priceInUsd / priceOutUsd)
  })

  return { mode, from, to, amount, priceIn, priceOut, targetPrice, stopPrice, feePct, result, error, impliedRate }
}
