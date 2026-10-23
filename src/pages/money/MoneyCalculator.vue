<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import { ScenarioBar } from '@/features/calc-scenario-manage'
import { GoalDialog, type GoalFromCalc } from '@/features/goal-edit'
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
    />
    <div class="grid items-start gap-4 @4xl:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
      <InvestCalculatorForm :calc="calc" />
      <!-- The answer is what the page is for: on a phone it comes before the fields. -->
      <InvestResult :calc="calc" :scenario-id="scenarioId" class="@max-4xl:order-first" @goal="wantGoal" />
    </div>
    <GoalDialog v-model:open="goalOpen" :draft="goalDraft" :from-calc="goalFrom" />
  </div>
</template>
