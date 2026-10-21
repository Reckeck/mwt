<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { ScenarioBar } from '@/features/calc-scenario-manage'
import { GoalDialog } from '@/features/goal-edit'
import { InvestCalculatorForm, useInvestCalc } from '@/features/invest-calculator'
import { InvestResult } from '@/widgets/invest-result'

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

const goalOpen = ref(false)
const goalDraft = ref<GoalInput | null>(null)

function load(scenario: CalcScenario) {
  calc.load(scenario.input, scenario.source, scenario.prefilled_at)
  calc.calculate()
}

function openGoal(draft: GoalInput) {
  goalDraft.value = draft
  goalOpen.value = true
}
</script>

<template>
  <div class="grid gap-4">
    <ScenarioBar v-model:scenario-id="scenarioId" :snapshot="calc.snapshot" @load="load" @goal="openGoal" />
    <div class="grid items-start gap-4 @4xl:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
      <InvestCalculatorForm :calc="calc" />
      <InvestResult :calc="calc" :scenario-id="scenarioId" />
    </div>
    <GoalDialog v-model:open="goalOpen" :draft="goalDraft" />
  </div>
</template>
