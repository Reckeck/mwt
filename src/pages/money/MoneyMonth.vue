<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'

import { useMonthBalance } from '@/entities/report'
import { formatMonthKey } from '@/shared/lib/format'
import { shiftMonth } from '@/shared/lib/period'
import { SectionState } from '@/shared/ui/section-state'

import { MonthBalanceCard, MonthDaysChart, MonthNow } from './ui/month-balance'

/** «Остаток месяца» (Money 3 · 15): the month in a sentence, day by day, then the twelve months with the carry. */
const props = defineProps<{ filter: ReportFilter }>()

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const month = computed(() => props.filter.to ?? '')
const year = computed<ReportFilter>(() => ({ ...props.filter, from: shiftMonth(month.value, -11), to: month.value }))
const balance = useMonthBalance(year)
const data = computed(() => balance.data.value ?? null)
const row = computed(() => data.value?.rows.find((entry) => entry.month === month.value) ?? null)
const daysTitle = computed(() => {
  const name = formatMonthKey(month.value, { month: 'long' })
  return t('balance.days.title', { month: name.charAt(0).toUpperCase() + name.slice(1) })
})

function pick(key: string) {
  void router.replace({ query: { ...route.query, from: key, to: key } })
}
</script>

<template>
  <SectionState :loading="balance.isPending.value" :error="balance.error.value" skeleton="figure" @retry="balance.refetch()">
    <div v-if="data && row" class="grid grid-cols-1 gap-4 md:grid-cols-[330px_minmax(0,1fr)]">
      <MonthNow :balance="data" :row="row" />
      <MonthDaysChart :balance="data" :title="daysTitle" />
      <MonthBalanceCard :balance="data" :month="month" class="md:col-span-2" @pick="pick" />
    </div>
  </SectionState>
</template>
