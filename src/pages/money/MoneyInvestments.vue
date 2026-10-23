<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown } from 'lucide-vue-next'

import { AnalyticsReportView, useReport } from '@/widgets/analytics-reports'
import { FlowList } from '@/widgets/flow-list'
import { TopupAdvisor } from '@/widgets/topup-advisor'
import { TradeRecordDialog } from '@/features/trade-record'
import { defaultRange } from '@/shared/lib/period'
import { Button } from '@/shared/ui/button'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'
import { EmptyState } from '@/shared/ui/empty-state'
import { SectionState } from '@/shared/ui/section-state'

import { useInvestments } from './model/use-investments'
import { InvestFigures, InvestTable } from './ui/investments'

/**
 * Investments (Money 2 · 13): profit, fees and what was put in, then a row per
 * coin. The allocation, the top-up advice and the loss chains wait below, folded.
 */
const props = defineProps<{ filter: ReportFilter }>()

const { t } = useI18n()
const investments = useInvestments(() => props.filter.scope.store_ids)
const pnl = useReport('pnl', () => props.filter)
const fees = computed(() => (pnl.data.value && 'pnl' in pnl.data.value ? pnl.data.value.pnl.total_fees : '0'))
/** The chains of the same period the reports show: an unset end is the report's default. */
const period = computed(() => {
  const fallback = defaultRange()
  return { from: props.filter.from ?? fallback.from, to: props.filter.to ?? fallback.to }
})
const details = ref(false)
const recording = ref(false)
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <SectionState :loading="investments.isPending.value" :error="investments.error.value" skeleton="report" @retry="investments.refetch()">
      <template v-if="investments.data.value?.rows.length">
        <InvestFigures
          :profit="investments.data.value.profit"
          :fees="fees"
          :invested="investments.data.value.invested"
          :currency="investments.data.value.base_currency"
          @record="recording = true"
        />
        <InvestTable :rows="investments.data.value.rows" :currency="investments.data.value.base_currency" />
      </template>
      <EmptyState v-else-if="investments.data.value" :title="t('money.invest.empty')" :description="t('money.invest.emptyHint')">
        <Button data-invest-record @click="recording = true">{{ t('money.invest.record') }}</Button>
      </EmptyState>
    </SectionState>
    <Collapsible v-model:open="details" class="grid gap-3">
      <CollapsibleTrigger as-child>
        <Button variant="ghost" size="sm" class="w-fit text-muted-foreground" data-invest-details>
          {{ t('money.invest.details') }} <ChevronDown :class="['transition-transform', details && 'rotate-180']" />
        </Button>
      </CollapsibleTrigger>
      <CollapsibleContent class="grid gap-6">
        <template v-if="details">
          <section class="grid gap-3">
            <h2 class="heading-section">{{ t('analytics.section.allocation') }}</h2>
            <AnalyticsReportView kind="portfolio" :filter="props.filter" />
          </section>
          <section class="grid gap-3">
            <h2 class="heading-section">{{ t('analytics.section.topup') }}</h2>
            <TopupAdvisor />
          </section>
          <section class="grid gap-3">
            <h2 class="heading-section">{{ t('analytics.section.result') }}</h2>
            <AnalyticsReportView kind="pnl" :filter="props.filter" />
          </section>
          <section class="grid gap-3">
            <h2 class="heading-section">{{ t('analytics.section.losses') }}</h2>
            <AnalyticsReportView kind="losses" :filter="props.filter" />
            <!-- How each chain of conversions lost money, under the totals. -->
            <FlowList :from="period.from" :to="period.to" :store-ids="props.filter.scope.store_ids" />
          </section>
        </template>
      </CollapsibleContent>
    </Collapsible>
    <TradeRecordDialog v-model:open="recording" />
  </div>
</template>
