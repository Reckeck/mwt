<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { BudgetList } from '@/widgets/budget-list'
import { GoalList } from '@/widgets/goal-list'

/** The plan: spending limits, then what is saved for. Stage 2 turns both into two-step wizards. */
const { t } = useI18n()
</script>

<template>
  <div class="grid grid-cols-1 gap-6">
    <section class="grid gap-3" data-tour="plan-limits">
      <h2 class="heading-section">{{ t('money.plan.limits') }}</h2>
      <BudgetList />
    </section>
    <section class="grid gap-3" data-tour="plan-goals">
      <h2 class="heading-section">{{ t('money.plan.goals') }}</h2>
      <GoalList />
    </section>
  </div>
</template>
