<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'
import { Info } from 'lucide-vue-next'

import { call } from '@/shared/api/ipc'
import { decimalCmp } from '@/shared/lib/decimal'
import { formatMoney, formatPct } from '@/shared/lib/format'
import { Alert, AlertDescription } from '@/shared/ui/alert'

import { exchangeQuote } from '../lib/exchange-math'

/**
 * The deal rate of an exchange under its two sides (boards 462813, 460801): one got unit in the
 * given currency, the market rate, and what the deal lost (a warning) or saved over the market.
 * `from` is the given currency, `to` the got one; the amounts are the form's.
 */
const props = defineProps<{ gave: string; got: string; from: string; to: string }>()
const { t } = useI18n()

const enabled = computed(() => Boolean(props.from && props.to && props.from !== props.to && exchangeQuote(props.gave, props.got, null)))
const market = useQuery({
  queryKey: computed(() => ['convert', props.from, props.to, props.gave, '']),
  placeholderData: keepPreviousData,
  queryFn: () => call<ConvertedAmount>('convert_amount', { input: { from: props.from, to: props.to, amount: props.gave, at: null } }),
  enabled,
})
const quote = computed(() => (enabled.value ? exchangeQuote(props.gave, props.got, market.data.value?.amount ?? null) : null))
const lost = computed(() => (quote.value?.loss ? decimalCmp(quote.value.loss, '0') === 1 : false))

/** The one line of the plate; money is preformatted here, the template only shows it. */
const line = computed(() => {
  const value = quote.value
  if (!value) return ''
  // money: plain — the rate, the market and the loss sit inside one sentence of the plate
  const money = (amount: string, decimals: number) => formatMoney(amount, { currency: props.from, decimals })
  const rate = t('tx.quick.rateLine', { to: props.to.toUpperCase(), rate: money(value.rate, 2) })
  if (!value.market || !value.loss || !value.percent || decimalCmp(value.loss, '0') === 0) return rate
  const market = t('tx.quick.marketLine', { market: money(value.market, 2) })
  const amount = money(value.loss.replace('-', ''), 0)
  const percent = formatPct(value.percent.replace('-', ''), { decimals: 1 })
  return `${rate} · ${market}. ${t(lost.value ? 'tx.quick.lossLine' : 'tx.quick.gainLine', { amount, percent })}`
})
</script>

<template>
  <Alert v-if="line" :variant="lost ? 'warning' : 'default'" data-exchange-summary :data-exchange-loss="lost || undefined">
    <Info />
    <!-- money: plain — the whole sentence is preformatted in `line` -->
    <AlertDescription>{{ line }}</AlertDescription>
  </Alert>
</template>
