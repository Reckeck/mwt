<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { CAPITAL_HEALTH_KEY } from '@/shared/api/queries'
import { formatQuantity } from '@/shared/lib/format'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'

/** `Emergency fund` (Home 4): how many months of spending the emergency fund covers; the figure is the checkup's. */
const { t } = useI18n()
const report = useQuery({
  queryKey: CAPITAL_HEALTH_KEY,
  queryFn: () => call<HealthReport>('capital_health'),
  placeholderData: keepPreviousData,
})
const months = computed(() => report.data.value?.emergency.months_covered ?? null)
const target = computed(() => report.data.value?.emergency.fund.target_months ?? 0)
</script>

<template>
  <Card size="sm">
    <SectionState :loading="report.isPending.value" :error="report.error.value" skeleton="figure" compact @retry="report.refetch()">
      <div class="grid gap-1" data-tour="dash-fund">
        <p class="truncate text-xs text-muted-foreground">{{ t('dashboard.widget.fund') }}</p>
        <RouterLink :to="{ name: 'money-health' }" class="rounded-sm text-2xl font-semibold tabular-nums hover:underline focus-visible:outline-2 focus-visible:outline-ring">
          {{ months === null ? '—' : t('dashboard.health.months', { months: formatQuantity(months, 1), target }) }}
        </RouterLink>
        <p class="text-xs text-muted-foreground">{{ t('dashboard.fund.hint') }}</p>
      </div>
    </SectionState>
  </Card>
</template>
