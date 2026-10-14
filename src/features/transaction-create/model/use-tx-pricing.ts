import { computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { formatDateTime, formatMoney } from '@/shared/lib/format'
import { useSettingsStore } from '@/shared/store/settings'

import { pricedAsset, toIsoTimestamp } from '../transaction-form.model'
import type { TxFormState } from './use-tx-form-state'

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
    if (!priced.value || priced.value.symbol.toLowerCase() === hintCode.value || form.price.trim() || !data) return ''
    if (!data.ts) return t('tx.rate.none')
    const converted = hintCode.value === 'usd' ? data.price_usd : quoteInCurrency.data.value?.amount
    if (!converted) return ''
    const price = Number(converted)
    return t('tx.rate.at', {
      date: formatDateTime(data.ts),
      price: `1 ${priced.value.symbol.toUpperCase()} = ${formatMoney(price, { decimals: price < 1 ? 6 : 2, currency: hintCode.value })}`,
    })
  })

  // Moving an operation to another date re-prices it there, unless the price
  // was changed by hand in this edit.
  const loadedPrice = form.price
  watch(
    () => form.timestamp,
    () => {
      if (editing.value && form.price === loadedPrice) form.price = ''
    },
  )

  return { rateHint }
}
