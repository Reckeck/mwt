<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money, useMoneyChartTooltip } from '@/entities/currency'
import type { InvestCalc } from '@/features/invest-calculator'
import { Chart, type ChartOption } from '@/shared/lib/echarts'
import { chartAxisColor } from '@/shared/lib/echarts-theme'
import { useTheme } from '@/shared/lib/theme'
import { Button } from '@/shared/ui/button'
import { SectionState } from '@/shared/ui/section-state'
import { StatCard } from '@/shared/ui/stat-card'
import { Toggle } from '@/shared/ui/toggle'

import { resultChart } from './model/result-chart'
import { MAX_COMPARED, useScenarioLines } from './model/use-scenario-lines'
import InvestCompare from './ui/InvestCompare.vue'
import InvestYearTable from './ui/InvestYearTable.vue'

/** What the plan grows into: the figures, the chart, «что выгоднее» and the table. */
const props = defineProps<{ calc: InvestCalc; scenarioId: string | null }>()

const { t } = useI18n()
const axisColor = chartAxisColor(useTheme().resolved)
const outcome = computed(() => props.calc.outcome.data.value)
const currency = computed(() => outcome.value?.input.currency ?? props.calc.form.currency)
const tooltip = useMoneyChartTooltip(currency)
const overlay = useScenarioLines(() => props.scenarioId)

const option = computed<ChartOption | null>(() => {
  const data = outcome.value
  if (!data) return null
  return resultChart(
    data.result,
    overlay.lines.value,
    {
      year: (year) => t('invest.yearN', { year }),
      invested: t('invest.chart.invested'),
      interest: t('invest.chart.interest'),
      real: t('invest.chart.real'),
      crossover: t('invest.chart.crossover'),
      depleted: t('invest.chart.depleted'),
    },
    { axisColor: axisColor.value, currency: currency.value, tooltip: tooltip.value },
  )
})

const solved = computed(() => outcome.value?.solved ?? null)
const hasTax = computed(() => Number(outcome.value?.result.tax ?? 0) > 0)
const hasWithdrawal = computed(() => Number(outcome.value?.result.withdrawn ?? 0) > 0)
</script>

<template>
  <SectionState
    :loading="props.calc.outcome.isPending.value && props.calc.outcome.fetchStatus.value !== 'idle'"
    :error="props.calc.outcome.error.value"
    :empty="!outcome"
    :empty-title="t('invest.resultEmpty')"
    skeleton="report"
  >
    <div v-if="outcome" class="@container grid gap-4" data-tour="calc-result">
      <div v-if="solved" class="grid gap-2 rounded-md border p-3" :class="!solved.reachable && 'border-warning'">
        <p class="text-sm">
          <template v-if="solved.unknown === 'contribution'">
            {{ t(solved.reachable ? 'invest.solved.contribution' : 'invest.unreachable.contribution') }}
            <Money :amount="solved.value" :currency="currency" class="font-semibold" />
          </template>
          <template v-else-if="solved.unknown === 'term'">
            {{ t(solved.reachable ? 'invest.solved.term' : 'invest.unreachable.term') }}
            <b>{{ t('invest.termValue', { years: Math.floor(Number(solved.value) / 12), months: Number(solved.value) % 12 }) }}</b>
          </template>
          <template v-else>
            {{ t(solved.reachable ? 'invest.solved.rate' : 'invest.unreachable.rate') }}
            <b>{{ t('invest.pct', { value: solved.value }) }}</b>
          </template>
        </p>
        <Button v-if="solved.reachable" variant="outline" size="sm" class="w-fit" @click="props.calc.acceptSolved">
          {{ t('invest.solved.apply') }}
        </Button>
      </div>

      <div class="grid gap-3 @md:grid-cols-2 @3xl:grid-cols-4">
        <StatCard :label="t('invest.stat.final')" :hint="t('invest.stat.effective', { value: outcome.result.effective_rate_pct })">
          <Money :amount="outcome.result.final_nominal" :currency="currency" :decimals="0" />
        </StatCard>
        <StatCard :label="t('invest.stat.real')" :hint="t('invest.stat.realHint', { value: outcome.input.inflation_pct })">
          <Money :amount="outcome.result.final_real" :currency="currency" :decimals="0" />
        </StatCard>
        <StatCard :label="t('invest.stat.invested')">
          <Money :amount="outcome.result.invested" :currency="currency" :decimals="0" />
        </StatCard>
        <StatCard :label="t('invest.stat.interest')">
          <Money :amount="outcome.result.interest" :currency="currency" :decimals="0" />
        </StatCard>
        <StatCard v-if="hasTax" :label="t('invest.stat.tax')">
          <Money :amount="outcome.result.tax" :currency="currency" :decimals="0" />
        </StatCard>
        <StatCard v-if="hasWithdrawal" :label="t('invest.stat.withdrawn')">
          <Money :amount="outcome.result.withdrawn" :currency="currency" :decimals="0" />
        </StatCard>
      </div>
      <p v-if="outcome.result.depleted_at !== null" class="text-sm text-destructive">
        {{ t('invest.depleted', { months: outcome.result.depleted_at }) }}
      </p>
      <p v-if="outcome.result.crossover_year !== null" class="text-sm text-muted-foreground">
        {{ t('invest.crossover', { year: outcome.result.crossover_year }) }}
      </p>

      <Chart v-if="option" :option="option" class="h-72" />
      <div v-if="overlay.others.value.length" class="flex flex-wrap items-center gap-2" data-tour="calc-overlay">
        <span class="text-sm text-muted-foreground">{{ t('invest.overlay', { max: MAX_COMPARED }) }}</span>
        <Toggle
          v-for="scenario in overlay.others.value"
          :key="scenario.id"
          variant="outline"
          size="sm"
          :model-value="overlay.picked.value.includes(scenario.id)"
          @update:model-value="overlay.toggle(scenario.id)"
        >
          {{ scenario.name }}
        </Toggle>
      </div>

      <InvestCompare :input="outcome.input" />
      <InvestYearTable :years="outcome.result.years" :currency="currency" />
    </div>
  </SectionState>
</template>
