<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { formatMoney } from '@/shared/lib/format'

import type { UtilityListProps } from './utility-list.types'
import { Money } from '@/entities/currency'

const props = defineProps<UtilityListProps>()
const { t } = useI18n()

const summary = useQuery({
  // no placeholder: another property's summary
  queryKey: computed(() => ['property-summary', props.propertyId]),
  queryFn: () => call<PropertySummary>('get_property_summary', { storeId: props.propertyId }),
})

const money = (value: string | undefined) =>
  formatMoney(value ?? '0', { currency: summary.data.value?.base_currency, decimals: 0 })

const rows = computed(() => {
  const data = summary.data.value
  if (!data) return []
  return [
    { key: 'value', label: t('propertySummary.value'), value: data.value_base },
    { key: 'debt', label: t('propertySummary.debt'), value: data.debt_base },
    { key: 'equity', label: t('propertySummary.equity'), value: data.equity_base },
    { key: 'monthly', label: t('propertySummary.monthly'), value: data.monthly_cost_base },
  ]
})

/** The last 12 months: rent and what the property cost; a car adds its wear and the cost of owning it. */
const year = computed(() => {
  const data = summary.data.value
  if (!data) return []
  const car = data.ownership_month_base !== null
  return [
    { key: 'rent', label: t('propertySummary.rent12m'), value: data.rent_12m_base, shown: !car },
    { key: 'expenses', label: t('propertySummary.expenses12m'), value: data.expenses_12m_base, shown: true },
    { key: 'wear', label: t('propertySummary.wear12m'), value: data.wear_12m_base, shown: car },
    { key: 'ownership', label: t('propertySummary.ownership'), value: data.ownership_month_base ?? '0', shown: car },
  ].filter((row) => row.shown)
})
</script>

<template>
  <Card>
    <div class="grid gap-4 @container">
      <div class="grid grid-cols-2 gap-4 @mid:grid-cols-4">
        <div v-for="row in rows" :key="row.key" class="grid gap-0.5">
          <p class="truncate text-xs text-muted-foreground">{{ row.label }}</p>
          <p class="truncate text-lg font-semibold tabular-nums">
            <Money :amount="row.value" :currency="summary.data.value?.base_currency" :decimals="0" />
          </p>
        </div>
      </div>
      <div v-if="year.length" class="grid grid-cols-2 gap-4 @mid:grid-cols-4" data-tour="property-year">
        <div v-for="row in year" :key="row.key" class="grid gap-0.5">
          <p class="truncate text-xs text-muted-foreground">{{ row.label }}</p>
          <p class="truncate font-semibold tabular-nums">
            <Money :amount="row.value" :currency="summary.data.value?.base_currency" :decimals="0" />
          </p>
        </div>
        <div v-if="summary.data.value?.rent_yield_pct" class="grid gap-0.5">
          <p class="truncate text-xs text-muted-foreground">{{ t('propertySummary.rentYield') }}</p>
          <p class="truncate font-semibold tabular-nums text-success">
            {{ t('propertySummary.pct', { pct: summary.data.value.rent_yield_pct }) }}
          </p>
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
