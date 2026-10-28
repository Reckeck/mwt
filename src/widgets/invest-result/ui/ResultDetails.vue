<script setup lang="ts">
import { computed, ref, toRef } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronRight } from 'lucide-vue-next'

import { useMoneyChartTooltip } from '@/entities/currency'
import type { CalcOutcome, InvestCalc } from '@/features/invest-calculator'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartColors } from '@/shared/lib/chart-theme'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { useTheme } from '@/shared/lib/theme'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'

import { resultChart } from '../model/result-chart'
import type { ContributionAlternative } from '../model/use-answer-extras'
import type { CompareLine, CompareLines } from '../model/use-compare-lines'
import CompareList from './CompareList.vue'
import InvestCompare from './InvestCompare.vue'
import InvestYearTable from './InvestYearTable.vue'
import ResultHint from './ResultHint.vue'
import ResultStats from './ResultStats.vue'

/** How the answer adds up: the figures, the chart, «что выгоднее» and the years. */
const props = defineProps<{
  calc: InvestCalc
  compare: CompareLines
  outcome: CalcOutcome
  currency: string
  alternatives: ContributionAlternative[]
}>()
const emit = defineEmits<{ open: [target: CompareLine['open']] }>()

const { t } = useI18n()
const axisColor = chartAxisColor(useTheme().resolved)
const tooltip = useMoneyChartTooltip(toRef(props, 'currency'))
const yearsOpen = ref(false)

const option = computed<ChartOption>(() =>
  resultChart(
    props.outcome.result,
    props.compare.lines.value,
    {
      year: (year) => t('invest.yearN', { year }),
      invested: t('invest.chart.invested'),
      interest: t('invest.chart.interest'),
      real: t('invest.chart.real'),
      crossover: t('invest.chart.crossover'),
      depleted: t('invest.chart.depleted'),
      current: t('invest.compareWith.current'),
    },
    {
      axisColor: axisColor.value,
      primary: chartColors().primary,
      palette: chartColors().palette,
      currency: props.currency,
      tooltip: tooltip.value,
    },
  ),
)

/** A click on a compared line opens that run: its own parameters go into the form. */
function onChartClick(params: { seriesName?: string }) {
  const line = props.compare.lines.value.find((entry) => entry.name === params.seriesName)
  if (line) emit('open', line.open)
}

/** «Что выгоднее» answers «сколько будет»: it has nothing to add to a found value or to running out. */
const comparing = computed(() => !props.outcome.solved && Number(props.outcome.result.withdrawn ?? 0) === 0)
</script>

<template>
  <div class="@container grid gap-4">
    <ResultStats :outcome="props.outcome" :currency="props.currency" />
    <ResultHint v-if="props.outcome.solved" :solved="props.outcome.solved" :currency="props.currency" :alternatives="props.alternatives" />

    <div class="h-64 w-full min-w-0"><Chart :option="option" autoresize @click="onChartClick" /></div>
    <p v-if="props.outcome.result.crossover_year !== null && comparing" class="flex gap-2 text-sm text-muted-foreground">
      <span class="text-primary">●</span>{{ t('invest.insight.crossover', { year: props.outcome.result.crossover_year }) }}
    </p>
    <template v-if="props.compare.lines.value.length">
      <p class="text-xs text-muted-foreground" data-calc-compare-note>{{ t('invest.compareWith.note') }}</p>
      <CompareList
        :current="props.outcome.result.final_nominal"
        :lines="props.compare.lines.value"
        :currency="props.currency"
        @open="emit('open', $event)"
      />
    </template>

    <InvestCompare v-if="comparing" :input="props.outcome.input" @apply="props.calc.applyCompare" />

    <Collapsible v-model:open="yearsOpen" class="grid gap-2 border-t pt-3">
      <CollapsibleTrigger
        class="touch-hit flex w-full items-center justify-between gap-3 rounded-md text-left focus-visible:ring-3 focus-visible:ring-ring/50 focus-visible:outline-none"
      >
        <span class="grid min-w-0 gap-0.5">
          <span class="text-sm font-medium">{{ t('invest.byYears') }}</span>
          <span class="truncate text-xs text-muted-foreground">{{ t('invest.byYearsHint') }}</span>
        </span>
        <ChevronRight class="size-4 shrink-0 text-muted-foreground transition-transform" :class="yearsOpen && 'rotate-90'" />
      </CollapsibleTrigger>
      <CollapsibleContent>
        <InvestYearTable :years="props.outcome.result.years" :currency="props.currency" />
      </CollapsibleContent>
    </Collapsible>
  </div>
</template>
