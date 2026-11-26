<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown } from 'lucide-vue-next'

import { BudgetList } from '@/widgets/budget-list'
import { BudgetSettings } from '@/widgets/budget-settings'
import { FixedCosts } from '@/widgets/fixed-costs'
import { GoalDetailsSheet } from '@/widgets/goal-details'
import { GoalList } from '@/widgets/goal-list'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'
import { Button } from '@/shared/ui/button'

import PlanCarryOver from './ui/plan/PlanCarryOver.vue'
import PlanEmergencyFund from './ui/plan/PlanEmergencyFund.vue'
import PlanEnvelopes from './ui/plan/PlanEnvelopes.vue'
import PlanGoals from './ui/plan/PlanGoals.vue'

/**
 * The plan (Budget): limits as Budget rows, goals, envelopes, the carry-over prompt and the emergency fund.
 * The full goal list with its calendar waits folded below.
 * The regular payments sit under the envelopes.
 * The plan's own settings (savings rate, a usual month, the daily budget) wait folded below.
 */
const { t } = useI18n()
const settingsOpen = ref(false)
const goalsOpen = ref(false)
/** One goal page for the plan's goal rows and the full list. */
const goalId = ref<string | null>(null)
const goalCardOpen = computed({
  get: () => goalId.value !== null,
  set: (value) => {
    if (!value) goalId.value = null
  },
})
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <div class="grid items-start gap-4 @3xl:grid-cols-2">
      <section class="grid content-start gap-2 rounded-lg border border-border bg-card p-4" data-tour="plan-limits">
        <h2 class="heading-section">{{ t('money.plan.limits') }}</h2>
        <BudgetList editable />
      </section>
      <div class="grid gap-4">
        <PlanGoals @open="goalId = $event" />
        <PlanEmergencyFund />
        <PlanCarryOver />
      </div>
    </div>
    <PlanEnvelopes />
    <FixedCosts />
    <Collapsible v-model:open="goalsOpen" class="grid gap-3">
      <CollapsibleTrigger as-child>
        <Button variant="ghost" size="sm" class="w-fit text-muted-foreground" data-plan-details>
          {{ t('money.plan.details') }} <ChevronDown :class="['transition-transform', goalsOpen && 'rotate-180']" />
        </Button>
      </CollapsibleTrigger>
      <CollapsibleContent><GoalList v-if="goalsOpen" @open="goalId = $event" /></CollapsibleContent>
    </Collapsible>
    <Collapsible v-model:open="settingsOpen" class="grid gap-3">
      <CollapsibleTrigger as-child>
        <Button variant="ghost" size="sm" class="w-fit text-muted-foreground" data-plan-settings>
          {{ t('money.plan.settings') }} <ChevronDown :class="['transition-transform', settingsOpen && 'rotate-180']" />
        </Button>
      </CollapsibleTrigger>
      <CollapsibleContent><BudgetSettings v-if="settingsOpen" /></CollapsibleContent>
    </Collapsible>
    <GoalDetailsSheet v-model:open="goalCardOpen" :goal-id="goalId" />
  </div>
</template>
