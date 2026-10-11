<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { formatMoney } from '@/shared/lib/format'

import type { UtilityListProps } from './utility-list.types'

const props = defineProps<UtilityListProps>()
const { t } = useI18n()

const summary = useQuery({
  queryKey: computed(() => ['property-summary', props.propertyId]),
  queryFn: () => call<PropertySummary>('get_property_summary', { storeId: props.propertyId }),
})

const money = (value: string | undefined) =>
  formatMoney(value ?? '0', { currency: summary.data.value?.base_currency, decimals: 0 })

const rows = computed(() => {
  const data = summary.data.value
  if (!data) return []
  return [
    { key: 'value', label: t('propertySummary.value'), value: money(data.value_base) },
    { key: 'debt', label: t('propertySummary.debt'), value: money(data.debt_base) },
    { key: 'equity', label: t('propertySummary.equity'), value: money(data.equity_base) },
    { key: 'monthly', label: t('propertySummary.monthly'), value: money(data.monthly_cost_base) },
  ]
})
</script>

<template>
  <Card>
    <div class="grid gap-4 @container">
      <div class="grid grid-cols-2 gap-4 @md:grid-cols-4">
        <div v-for="row in rows" :key="row.key" class="grid gap-0.5">
          <p class="truncate text-xs text-muted-foreground">{{ row.label }}</p>
          <p class="truncate text-lg font-semibold tabular-nums">{{ row.value }}</p>
        </div>
      </div>
      <p v-if="summary.data.value" class="text-xs text-muted-foreground">
        {{
          t('propertySummary.breakdown', {
            utilities: money(summary.data.value.utilities_month_base),
            loan: money(summary.data.value.loan_month_base),
            count: summary.data.value.utility_count,
          })
        }}
      </p>
    </div>
  </Card>
</template>
