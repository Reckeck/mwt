<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown } from 'lucide-vue-next'

import { BudgetList } from '@/widgets/budget-list'
import { GoalList } from '@/widgets/goal-list'
import { Button } from '@/shared/ui/button'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'

import { PlanGoals, PlanLimits, PlanNext } from './ui/plan'

/**
 * The plan (Money 2 · 9, 10): limits and goals side by side, then what the
 * month still brings. Editing, deleting and the goal calendar wait below,
 * folded.
 */
const props = defineProps<{ filter: ReportFilter }>()

const { t } = useI18n()
const details = ref(false)
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <div class="grid items-start gap-4 @3xl:grid-cols-2">
      <PlanLimits :filter="props.filter" />
      <PlanGoals :filter="props.filter" />
    </div>
    <PlanNext />
    <Collapsible v-model:open="details" class="grid gap-3">
      <CollapsibleTrigger as-child>
        <Button variant="ghost" size="sm" class="w-fit text-muted-foreground" data-plan-details>
          {{ t('money.plan.details') }} <ChevronDown :class="['transition-transform', details && 'rotate-180']" />
        </Button>
      </CollapsibleTrigger>
      <CollapsibleContent class="grid gap-6">
        <BudgetList v-if="details" />
        <GoalList v-if="details" />
      </CollapsibleContent>
    </Collapsible>
  </div>
</template>
