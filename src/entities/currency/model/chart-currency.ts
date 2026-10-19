import { type Ref, computed, ref, watch } from 'vue'

import { useSettingsStore } from '@/shared/store/settings'

import { useConversions } from './conversions'
import { useFavoriteCurrencies } from './favorites'

/** Remembered per viewer: the last currency picked on any money chart. */
const STORAGE_KEY = 'chart.currency'

function readStored(): string {
  try {
    return localStorage.getItem(STORAGE_KEY) ?? ''
  } catch {
    return ''
  }
}

/** One choice for every money chart on screen, so they never disagree. */
const picked = ref(readStored())
watch(picked, (code) => {
  try {
    localStorage.setItem(STORAGE_KEY, code)
  } catch {
    /* private window: the choice is not remembered */
  }
})

/**
 * A money chart in a currency of the user's choice: the base or a favourite.
 * `source` is the currency the series comes in; `convert` turns its values
 * into the shown one at the cached rate. While the rate is not known yet the
 * chart stays in `source` rather than drawing wrong numbers.
 */
export function useChartCurrency(source: Ref<string>) {
  const settings = useSettingsStore()
  const favorites = useFavoriteCurrencies()
  const conversions = useConversions(true)

  // A favourite without a rate (a coin never priced yet) would be a switch that does nothing.
  const options = computed(() => [
    ...new Set([
      source.value.toLowerCase(),
      settings.baseCurrency.toLowerCase(),
      ...favorites.codes.value.filter((code) => conversions.convertTo(1, source.value, code) !== null),
    ]),
  ])

  const factor = computed(() => {
    const wanted = picked.value && options.value.includes(picked.value) ? picked.value : source.value.toLowerCase()
    const rate = conversions.convertTo(1, source.value, wanted)
    return rate === null ? { currency: source.value.toLowerCase(), rate: 1 } : { currency: wanted, rate }
  })

  /** The currency the chart shows now. */
  const currency = computed(() => factor.value.currency)
  /** The model of the switch: the shown currency, and the user's pick when set. */
  const selected = computed({
    get: () => currency.value,
    set: (code: string) => {
      if (code) picked.value = code
    },
  })
  const convert = (value: number) => value * factor.value.rate

  return { options, selected, currency, convert }
}
