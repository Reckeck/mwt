<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { wealthChange } from '@/entities/report'
import { chartColors } from '@/shared/lib/chart-theme'
import { Chart } from '@/shared/lib/echarts'
import { formatPct } from '@/shared/lib/format'
import { Card } from '@/shared/ui/card'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { netWorthOption } from './lib/net-worth-option'
import type { NetWorthProps, NetWorthSpan } from './net-worth.types'

/**
 * The portfolio hero: capital total, its change over the last month, and the line over the chosen span
 * (`--chart-*` colours, so both themes follow). The span is the owner's (`v-model:span`), the report is theirs too.
 */
const props = withDefaults(defineProps<NetWorthProps>(), { loading: false })
const span = defineModel<NetWorthSpan>('span', { required: true })

const SPANS: NetWorthSpan[] = ['last3', 'last6', 'last12', 'all']
const { t } = useI18n()

const total = computed(() => props.points.at(-1)?.net ?? '0')
const change = computed(() => wealthChange(props.points))
const tone = computed(() => ((change.value?.month ?? 0) < 0 ? 'text-destructive' : 'text-success'))
const pct = computed(() =>
  change.value?.monthPct == null ? '' : ` (${formatPct(change.value.monthPct, { signed: true, decimals: 1 })})`,
)
const option = computed(() =>
  netWorthOption(chartColors(), { points: props.points, currency: props.currency, name: t('money.wealth.title') }),
)

function pick(value: unknown) {
  if (typeof value === 'string' && value) span.value = value as NetWorthSpan
}
</script>

<template>
  <Card class="grid gap-4 p-4 md:p-6" data-tour="wealth-head" data-net-worth>
    <div class="flex flex-wrap items-start justify-between gap-3">
      <div class="flex min-w-0 flex-col gap-0.5">
        <span class="text-sm text-muted-foreground">{{ t('money.wealth.title') }}</span>
        <span class="text-3xl font-semibold tabular-nums" data-net-worth-total>
          <Money :amount="total" :currency="props.currency" />
        </span>
        <span v-if="change" class="text-sm tabular-nums" :class="tone">
          <Money :amount="change.month" :currency="props.currency" :decimals="0" signed />{{ pct }}
          {{ t('money.wealth.perMonth') }}
        </span>
        <span v-else class="text-sm">&nbsp;</span>
      </div>
      <ToggleGroup
        type="single"
        variant="range"
        :spacing="1"
        :model-value="span"
        :aria-label="t('money.wealth.span')"
        data-range-switch
        @update:model-value="pick"
      >
        <ToggleGroupItem v-for="item in SPANS" :key="item" :value="item" :data-range="item">
          {{ t(`money.wealth.spans.${item}`) }}
        </ToggleGroupItem>
      </ToggleGroup>
    </div>
    <Chart :option="option" class="h-56 w-full md:h-72" role="img" :aria-label="t('money.wealth.title')" data-net-worth-chart />
  </Card>
</template>
