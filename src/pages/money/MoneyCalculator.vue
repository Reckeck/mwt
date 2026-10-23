<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'

import { ScenarioBar } from '@/features/calc-scenario-manage'
import { GoalDialog, type GoalFromCalc } from '@/features/goal-edit'
import { COMPARE_FACTOR, InvestCalculatorForm, useInvestCalc } from '@/features/invest-calculator'
import { formatPct } from '@/shared/lib/format'
import { type CompareLine, CompareMenu, InvestResult, useCompareLines } from '@/widgets/invest-result'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const calc = useInvestCalc()

/** The open scenario is in the address (`?scenario=`), so a goal card can link straight to it. */
const scenarioId = computed<string | null>({
  get: () => (typeof route.query.scenario === 'string' ? route.query.scenario : null),
  set: (id) => {
    void router.replace({ query: { ...route.query, scenario: id ?? undefined } })
  },
})

/** «Сравнить»: the variants follow the form, the saved scenarios run with their own parameters. */
const compare = useCompareLines(
  () => scenarioId.value,
  () => calc.outcome.data.value?.input ?? null,
  (variant, input) =>
    variant === 'more_rate'
      ? t('invest.compareWith.rate', { pct: formatPct(Number(input.rate_pct), { decimals: Number.isInteger(Number(input.rate_pct)) ? 0 : 2 }) })
      : t('invest.compareWith.contribution', { pct: formatPct((COMPARE_FACTOR - 1) * 100, { decimals: 0 }) }),
)

/** An opened line leaves the comparison: it is the run on the form now. */
function openCompared(target: CompareLine['open']) {
  if ('variant' in target) {
    calc.applyCompare(target.variant)
    compare.picked.value = compare.picked.value.filter((key) => key !== `variant:${target.variant}`)
  } else {
    scenarioId.value = target.scenario
    compare.picked.value = compare.picked.value.filter((key) => key !== `scenario:${target.scenario}`)
  }
}

const goalOpen = ref(false)
const goalDraft = ref<GoalInput | null>(null)
const goalFrom = ref<GoalFromCalc | null>(null)

function load(scenario: CalcScenario) {
  calc.load(scenario.input, scenario.source, scenario.prefilled_at)
  calc.calculate()
}

const bar = ref<InstanceType<typeof ScenarioBar> | null>(null)
/** A found contribution goes into the form first: the goal then saves with the amount the answer named. */
function wantGoal() {
  const solved = calc.outcome.data.value?.solved
  if (solved?.reachable && solved.unknown === 'contribution') calc.acceptSolved()
  bar.value?.createGoal()
}

function openGoal(draft: GoalInput) {
  goalDraft.value = draft
  goalFrom.value = { scenario: draft.name, initial: calc.outcome.data.value?.input.initial ?? '0' }
  goalOpen.value = true
}
</script>

<template>
  <div class="grid gap-4">
    <ScenarioBar
      ref="bar"
      v-model:scenario-id="scenarioId"
      :snapshot="calc.snapshot"
      @load="load"
      @goal="openGoal"
      @want-goal="wantGoal"
    >
      <template #actions><CompareMenu :compare="compare" /></template>
    </ScenarioBar>
    <div class="grid items-start gap-4 @4xl:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
      <InvestCalculatorForm :calc="calc" />
      <!-- The answer is what the page is for: on a phone it comes before the fields. -->
      <InvestResult :calc="calc" :compare="compare" class="@max-4xl:order-first" @goal="wantGoal" @open="openCompared" />
    </div>
    <GoalDialog v-model:open="goalOpen" :draft="goalDraft" :from-calc="goalFrom" />
  </div>
</template>
