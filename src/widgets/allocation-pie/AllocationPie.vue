<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { wealthRows } from '@/entities/report'
import { chartColors } from '@/shared/lib/chart-theme'
import { Chart } from '@/shared/lib/echarts'
import { cn } from '@/shared/lib/utils'
import { Card } from '@/shared/ui/card'

import { GROUP_STYLE } from './allocation-pie.constants'
import type { AllocationPieProps } from './allocation-pie.types'
import { allocationOption } from './lib/allocation-option'

/** What capital is made of: a donut by group and the groups under it with their sum and share, debts last. */
const props = defineProps<AllocationPieProps>()
const { t } = useI18n()

const rows = computed(() => wealthRows(props.points.at(-1)))
const option = computed(() =>
  allocationOption(chartColors(), {
    rows: rows.value,
    currency: props.currency,
    label: (row) => t(`money.wealth.group.${row.group}`),
  }),
)
</script>

<template>
  <Card v-if="rows.length" class="grid gap-4 p-4 md:p-6" data-tour="wealth-table" data-allocation>
    <h3 class="heading-card">{{ t('money.wealth.of') }}</h3>
    <div class="grid items-center gap-4 sm:grid-cols-[minmax(0,12rem)_minmax(0,1fr)]">
      <Chart :option="option" class="mx-auto h-40 w-full max-w-48" role="img" :aria-label="t('money.wealth.of')" />
      <ul class="grid gap-2">
        <li v-for="row in rows" :key="row.group" class="flex items-center gap-2 text-sm" data-allocation-row>
          <span aria-hidden="true" :class="cn('size-2 shrink-0 rounded-full', GROUP_STYLE[row.group].swatch)" />
          <span class="min-w-0 flex-1 truncate">{{ t(`money.wealth.group.${row.group}`) }}</span>
          <span class="tabular-nums" :class="row.value < 0 && 'text-destructive'">
            <Money :amount="row.value" :currency="props.currency" />
          </span>
          <span class="w-10 text-right text-muted-foreground tabular-nums">{{ row.share === null ? '' : `${row.share} %` }}</span>
        </li>
      </ul>
    </div>
  </Card>
</template>
