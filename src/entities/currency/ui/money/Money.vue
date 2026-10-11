<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery, useQueryClient } from '@tanstack/vue-query'

import { Tooltip, TooltipContent, TooltipTrigger } from '@/shared/ui/tooltip'
import { call } from '@/shared/api/ipc'
import { formatDate, formatMoney, moneyParts } from '@/shared/lib/format'
import { localDate } from '@/shared/lib/planning'
import { useSettingsStore } from '@/shared/store/settings'

import { fxTopUp } from './fx-top-up'
import { FX_TABLE_STALE_MS, MONEY_TOOLTIP_MAX_ROWS } from './money.constants'
import type { MoneyProps } from './money.types'

/**
 * An amount with grouped digits and a separate, muted currency mark.
 *
 * Hovering an amount in the base currency shows it converted into the
 * favourite and tracked currencies at the latest cached rate; a rate older
 * than today is labelled with its date instead of being passed off as current.
 */
const props = withDefaults(defineProps<MoneyProps>(), {
  currency: '',
  decimals: undefined,
  signed: false,
  compact: false,
  convert: undefined,
})

const { t } = useI18n()
const settings = useSettingsStore()

const code = computed(() => (props.currency || settings.baseCurrency).toLowerCase())
const parts = computed(() =>
  moneyParts(props.amount, { currency: code.value, decimals: props.decimals, signed: props.signed, compact: props.compact }),
)
const numeric = computed(() => Number(props.amount))
const convertible = computed(
  () => (props.convert ?? code.value === settings.baseCurrency) && Number.isFinite(numeric.value) && numeric.value !== 0,
)

/** Loaded on the first hover only; every Money on the screen shares the one query. */
const hovered = ref(false)
const table = useQuery({
  queryKey: ['fx-table'],
  queryFn: () => call<FxTable>('get_fx_table'),
  enabled: computed(() => hovered.value && convertible.value),
  staleTime: FX_TABLE_STALE_MS,
})

const today = localDate(new Date())
const queryClient = useQueryClient()

/**
 * Stale cached rates are topped up from the public sources once per session:
 * hovering is a strong hint the user wants today's numbers, and the backend
 * walks back to the last published day on its own when today has none.
 */
watch(
  () => table.data.value,
  (data) => {
    if (!data || fxTopUp.done) return
    const wanted = [...new Set([...settings.favoriteCurrencies.map((code) => code.toLowerCase()), ...data.rows.filter((row) => row.tracked).map((row) => row.currency), data.base])]
    const stale = wanted.filter((currency) => {
      const row = data.rows.find((entry) => entry.currency === currency)
      return currency !== 'usd' && (!row || row.updated_at.slice(0, 10) < today)
    })
    fxTopUp.done = true
    if (!stale.length) return
    void call<EnsureFxOutput>('ensure_fx', { input: { currencies: stale, date: null } satisfies EnsureFxInput })
      .then((result) => {
        if (result.points.length) void queryClient.invalidateQueries({ queryKey: ['fx-table'] })
      })
      .catch(() => {
        /* offline: the cached rates stay, labelled with their date */
      })
  },
)
const rows = computed(() => {
  const data = table.data.value
  if (!data || data.base !== code.value) return []
  const favorites = settings.favoriteCurrencies.map((entry) => entry.toLowerCase())
  const rank = (currency: string) => {
    const index = favorites.indexOf(currency)
    return index === -1 ? favorites.length : index
  }
  return [...data.rows]
    .filter((row) => favorites.includes(row.currency) || row.tracked)
    .sort((left, right) => rank(left.currency) - rank(right.currency) || left.currency.localeCompare(right.currency))
    .slice(0, MONEY_TOOLTIP_MAX_ROWS)
    .map((row) => ({
      currency: row.currency,
      value: formatMoney(numeric.value * Number(row.per_base), { currency: row.currency }),
      stale: row.updated_at && row.updated_at.slice(0, 10) < today ? formatDate(row.updated_at) : '',
    }))
})
</script>

<template>
  <Tooltip v-if="convertible" @update:open="(isOpen: boolean) => isOpen && (hovered = true)">
    <TooltipTrigger as-child>
      <span class="money inline-flex cursor-default items-baseline gap-[0.25em] whitespace-nowrap tabular-nums">
        <span>{{ parts.number }}</span>
        <span v-if="parts.symbol" class="text-[0.85em] font-normal text-muted-foreground">{{ parts.symbol }}</span>
      </span>
    </TooltipTrigger>
    <TooltipContent side="top" class="min-w-44">
      <p v-if="table.isPending.value" class="opacity-70">{{ t('money.loadingRates') }}</p>
      <p v-else-if="!rows.length" class="opacity-70">{{ t('money.noRates') }}</p>
      <dl v-else class="grid grid-cols-[auto_auto] gap-x-3 gap-y-0.5 tabular-nums">
        <template v-for="row in rows" :key="row.currency">
          <dt class="text-right">{{ row.value }}</dt>
          <dd class="opacity-70">{{ row.stale ? t('money.rateOn', { date: row.stale }) : '' }}</dd>
        </template>
      </dl>
    </TooltipContent>
  </Tooltip>
  <span v-else class="money inline-flex items-baseline gap-[0.25em] whitespace-nowrap tabular-nums">
    <span>{{ parts.number }}</span>
    <span v-if="parts.symbol" class="text-[0.85em] font-normal text-muted-foreground">{{ parts.symbol }}</span>
  </span>
</template>
