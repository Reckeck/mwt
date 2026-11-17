import { computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { useSettingsStore } from '@/shared/store/settings'

import { pricedAsset, pricedAssetKey, toIsoTimestamp } from '../transaction-form.model'
import type { TxFormState } from './use-tx-form-state'
import { keepPreviousFor } from '@/shared/lib/query'

/**
 * The rate a new operation records (plan §6.2): the cached quote of its moment
 * shown as a hint, the day's FX fetched ahead so the backend can stamp it, and
 * a re-price when an edit moves the operation to another date.
 */
export function useTxPricing(state: TxFormState) {
  const { t } = useI18n()
  const settings = useSettingsStore()
  const { props, form, editing } = state

  const priced = computed(() => pricedAsset(form))
  const quote = useQuery({
    queryKey: computed(() => [
      'price-at',
      priced.value?.symbol,
      priced.value?.coingecko_id,
      priced.value?.chain,
      priced.value?.contract,
      form.timestamp,
    ]),
    placeholderData: keepPreviousFor(() => priced.value?.symbol),
    queryFn: () => {
      const asset = priced.value as AssetRef
      return call<CachedPrice>('get_cached_price', {
        input: {
          symbol: asset.symbol,
          coingecko_id: asset.coingecko_id,
          chain: asset.chain,
          contract: asset.contract,
          at: toIsoTimestamp(form.timestamp),
        },
      })
    },
    enabled: computed(() => !props.template && Boolean(priced.value) && !form.price.trim()),
  })

  /** The currency the price field is in: the user's pick, else the base currency. */
  const priceCode = computed(() => (form.priceCurrency || settings.baseCurrency).toLowerCase())

  /**
   * The currency the rate hint speaks: the price field's, unless that is the
   * asset itself (1 RUB = 1 RUB says nothing) — then the base currency, or USD
   * when the asset is the base currency.
   */
  const hintCode = computed(() => {
    const asset = priced.value?.symbol.toLowerCase() ?? ''
    if (asset !== priceCode.value) return priceCode.value
    const base = settings.baseCurrency.toLowerCase()
    return asset === base ? 'usd' : base
  })

  /** The cached USD quote, shown in the hint's currency. */
  const quoteInCurrency = useQuery({
    // no placeholder: a hint of another price would read as this one
    queryKey: computed(() => ['convert', quote.data.value?.price_usd, hintCode.value, form.timestamp]),
    queryFn: () =>
      call<ConvertedAmount>('convert_amount', {
        input: {
          from: 'usd',
          to: hintCode.value,
          amount: quote.data.value?.price_usd ?? '0',
          at: toIsoTimestamp(form.timestamp),
        } satisfies ConvertAmountInput,
      }),
    enabled: computed(() => Boolean(quote.data.value?.ts) && hintCode.value !== 'usd'),
  })

  /**
   * The price is stamped by the backend from the rates of the operation's day;
   * fetch that day's rates (or the last published ones) of the asset and the
   * price currency ahead of time, so the stamp has something to use.
   */
  const ensuredFx = new Set<string>()
  watch(
    [priceCode, () => form.timestamp, () => form.price, () => priced.value?.symbol],
    ([code, stamp, price, asset]) => {
      if (props.template || (!price.trim() && !asset)) return
      const day = (toIsoTimestamp(stamp) ?? new Date().toISOString()).slice(0, 10)
      const currencies = [...new Set([code, (asset ?? '').toLowerCase()])].filter(
        (entry) => entry && entry !== 'usd' && !ensuredFx.has(`${entry}@${day}`),
      )
      if (!currencies.length) return
      for (const entry of currencies) ensuredFx.add(`${entry}@${day}`)
      void call<EnsureFxOutput>('ensure_fx', { input: { currencies, date: day } satisfies EnsureFxInput }).catch(() => {
        /* offline: the backend answers FX_MISSING on save, with a clear message */
      })
    },
  )

  /** Which day's rate an empty price field stands for — or that there is none yet. */
  const rateHint = computed(() => {
    const data = quote.data.value
    // A dollar is always worth a dollar; saying so is noise.
    if (!priced.value || priced.value.symbol.toLowerCase() === hintCode.value || form.price.trim() || !data) return null
    if (!data.ts) return { text: t('tx.rate.none') }
    const converted = hintCode.value === 'usd' ? data.price_usd : quoteInCurrency.data.value?.amount
    if (!converted) return null
    const price = Number(converted)
    return {
      date: formatDateTime(data.ts),
      symbol: priced.value.symbol.toUpperCase(),
      amount: price,
      currency: hintCode.value,
      decimals: price < 1 ? 6 : 2,
    }
  })

  // The price belongs to the asset it was set for: another asset clears it, unless the price was
  // changed after the previous snapshot (a fill sets asset and price in one tick).
  // `previousPrice` is the price as of the last flush, so a price typed after an asset change
  // counts as changed in the next one; `priceSnapshot` is the price as of the last asset change.
  let priceSnapshot = form.price
  let previousPrice = form.price
  let previousKey = pricedAssetKey(form)
  watch([() => pricedAssetKey(form), () => form.price], ([key, price]) => {
    if (key !== previousKey && price === previousPrice) {
      form.price = ''
      form.priceCurrency = ''
    }
    if (key !== previousKey) priceSnapshot = form.price
    previousKey = key
    previousPrice = form.price
  })

  // Moving an operation to another date re-prices it there, unless the price
  // was changed by hand in this edit.
  watch(
    () => form.timestamp,
    () => {
      if (editing.value && form.price === priceSnapshot) form.price = ''
    },
  )

  return { rateHint }
}
