<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RefreshCw, TriangleAlert, WifiOff } from 'lucide-vue-next'

import { usePriceRefresh } from '@/features/price-refresh'
import { useRatesAge } from '@/entities/currency'
import { TIME_CYCLE, dateFormat, pluralForm } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'

/**
 * «Нет сети / курсы устарели» over the figures (Currency-2 · 8): when the rates
 * the sums use are from an earlier day, or the device is offline — what they
 * are as of, that operations are still recorded, and «Обновить». Goes with
 * the next rates.
 */
const { t, locale } = useI18n()
const rates = useRatesAge()
const refresh = usePriceRefresh()

const shown = computed(() => rates.offline.value || rates.stale.value)
const moment = computed(() =>
  rates.at.value
    ? dateFormat({ day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit', hourCycle: TIME_CYCLE }).format(new Date(rates.at.value))
    : '',
)
const day = computed(() => (rates.at.value ? dateFormat({ day: 'numeric', month: 'short' }).format(new Date(rates.at.value)) : ''))
const ago = computed(() =>
  rates.days.value > 0
    ? t(`ratesBar.daysAgo.${pluralForm(rates.days.value, locale.value)}`, { count: rates.days.value })
    : t('ratesBar.today'),
)
const title = computed(() => t(rates.offline.value ? 'ratesBar.offline' : 'ratesBar.stale'))
/** After the bold title: how old the rates are, and offline that nothing is lost. */
const detail = computed(() =>
  [rates.at.value ? t('ratesBar.detail', { at: moment.value, ago: ago.value }) : '', rates.offline.value ? t('ratesBar.recorded') : '']
    .filter(Boolean)
    .join(' '),
)
</script>

<template>
  <section
    v-if="shown"
    class="flex items-center gap-2.5 rounded-xl border border-warning bg-card px-3.5 py-2.5 text-sm shadow-sm max-md:gap-2 max-md:py-0.5 max-md:pr-1 max-md:pl-3"
    data-rates-bar
  >
    <component :is="rates.offline.value ? WifiOff : TriangleAlert" class="size-4 shrink-0 text-warning" aria-hidden="true" />
    <p class="min-w-0 flex-1 max-md:hidden">
      <b>{{ title }}</b>&#32;{{ detail }}
    </p>
    <p class="min-w-0 flex-1 md:hidden">
      {{ rates.at.value ? t(rates.offline.value ? 'ratesBar.offlineShort' : 'ratesBar.staleShort', { day }) : title }}
    </p>
    <Button
      variant="outline"
      size="sm"
      class="max-md:h-10 max-md:border-0 max-md:px-2 max-md:text-primary max-md:shadow-none"
      :disabled="refresh.isPending.value"
      data-rates-refresh
      @click="refresh.mutate()"
    >
      <RefreshCw class="max-md:hidden" :class="refresh.isPending.value && 'animate-spin'" aria-hidden="true" />
      {{ t('ratesBar.refresh') }}
    </Button>
  </section>
</template>
