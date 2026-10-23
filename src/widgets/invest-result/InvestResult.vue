<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronRight } from 'lucide-vue-next'

import { useMoneyChartTooltip } from '@/entities/currency'
import type { InvestCalc } from '@/features/invest-calculator'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartColors } from '@/shared/lib/chart-theme'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { useTheme } from '@/shared/lib/theme'
import { Card } from '@/shared/ui/card'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'
import { SectionState } from '@/shared/ui/section-state'

import { resultChart } from './model/result-chart'
import { useAnswerExtras } from './model/use-answer-extras'
import type { CompareLine, CompareLines } from './model/use-compare-lines'
import CompareList from './ui/CompareList.vue'
import InvestCompare from './ui/InvestCompare.vue'
import InvestYearTable from './ui/InvestYearTable.vue'
import ResultAnswer from './ui/ResultAnswer.vue'
import ResultHint from './ui/ResultHint.vue'
import ResultStats from './ui/ResultStats.vue'

/** The answer first, then how it adds up: the figures, the chart, «что выгоднее» and the years. */
const props = defineProps<{ calc: InvestCalc; compare: CompareLines }>()
const emit = defineEmits<{ goal: []; open: [target: CompareLine['open']] }>()

const { t } = useI18n()
const axisColor = chartAxisColor(useTheme().resolved)
const outcome = computed(() => props.calc.outcome.data.value)
const currency = computed(() => outcome.value?.input.currency ?? props.calc.form.currency)
const tooltip = useMoneyChartTooltip(currency)
const extras = useAnswerExtras(outcome, () => props.calc.form.target)
const yearsOpen = ref(false)

const option = computed<ChartOption | null>(() => {
  const data = outcome.value
  if (!data) return null
  return resultChart(
    data.result,
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
      currency: currency.value,
      tooltip: tooltip.value,
    },
  )
})

/** A click on a compared line opens that run: its own parameters go into the form. */
function onChartClick(params: { seriesName?: string }) {
  const line = props.compare.lines.value.find((entry) => entry.name === params.seriesName)
  if (line) emit('open', line.open)
}

/** «Что выгоднее» answers «сколько будет»: it has nothing to add to a found value or to running out. */
const comparing = computed(() => !outcome.value?.solved && Number(outcome.value?.result.withdrawn ?? 0) === 0)
</script>

<template>
  <Card>
    <SectionState
      :loading="props.calc.outcome.isPending.value && props.calc.outcome.fetchStatus.value !== 'idle'"
      :error="props.calc.outcome.error.value"
      :empty="!outcome"
      :empty-title="t('invest.resultEmpty')"
      skeleton="report"
    >
      <div v-if="outcome" class="@container grid gap-4" data-tour="calc-result" data-calc-result>
        <ResultAnswer
          :outcome="outcome"
          :currency="currency"
          :target="props.calc.form.target"
          :prefilled="props.calc.prefilled.value"
          :safe-withdrawal="extras.safeWithdrawal.value"
          @accept="props.calc.acceptSolved"
          @goal="emit('goal')"
          @withdraw="props.calc.applyWithdrawal"
        />
        <ResultStats :outcome="outcome" :currency="currency" />
        <ResultHint v-if="outcome.solved" :solved="outcome.solved" :currency="currency" :alternatives="extras.alternatives.value" />

        <div v-if="option" class="h-64 w-full min-w-0"><Chart :option="option" autoresize @click="onChartClick" /></div>
        <p v-if="outcome.result.crossover_year !== null && comparing" class="flex gap-2 text-sm text-muted-foreground">
          <span class="text-primary">●</span>{{ t('invest.insight.crossover', { year: outcome.result.crossover_year }) }}
        </p>
        <template v-if="props.compare.lines.value.length">
          <p class="text-xs text-muted-foreground" data-calc-compare-note>{{ t('invest.compareWith.note') }}</p>
          <CompareList :current="outcome.result.final_nominal" :lines="props.compare.lines.value" :currency="currency" @open="emit('open', $event)" />
        </template>

        <InvestCompare v-if="comparing" :input="outcome.input" @apply="props.calc.applyCompare" />

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
            <InvestYearTable :years="outcome.result.years" :currency="currency" />
          </CollapsibleContent>
        </Collapsible>
      </div>
    </SectionState>
  </Card>
</template>
