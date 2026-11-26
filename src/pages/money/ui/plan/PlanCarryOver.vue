<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { CarryOverSwitch } from '@/features/month-carry-over'
import { useMonthBalance } from '@/entities/report'
import { currentMonth } from '@/shared/lib/period'
import { SectionState } from '@/shared/ui/section-state'

import { monthBalanceFilter } from '../../lib/month-filter'

/** The carry-over prompt: what a month leaves can join the next one, from the first month shown. */
const { t } = useI18n()
const balance = useMonthBalance(() => monthBalanceFilter(currentMonth()))
</script>

<template>
  <section class="grid content-start gap-3 rounded-lg border border-border bg-card p-4">
    <h2 class="heading-section">{{ t('money.plan.carryTitle') }}</h2>
    <SectionState :loading="balance.isPending.value" :error="balance.error.value" skeleton="figure" compact @retry="balance.refetch()">
      <div v-if="balance.data.value" class="grid gap-2">
        <p class="text-sm text-muted-foreground">{{ t('money.plan.carryPrompt') }}</p>
        <CarryOverSwitch :carry-since="balance.data.value.carry_since" :from="balance.data.value.rows[0]?.month ?? currentMonth()" />
      </div>
    </SectionState>
  </section>
</template>
