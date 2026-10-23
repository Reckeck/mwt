<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown } from 'lucide-vue-next'

import { AnalyticsReportView, useReport } from '@/widgets/analytics-reports'
import { defaultRange } from '@/shared/lib/period'
import { Button } from '@/shared/ui/button'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'
import { EmptyState } from '@/shared/ui/empty-state'
import { SectionState } from '@/shared/ui/section-state'

import { WealthChart, WealthHead, WealthTable } from './ui/wealth'

/**
 * Capital (Money 2 · 12): the total and how it changed, a stack by group and
 * what it is made of. The wallet-by-wallet table and the full net worth chart
 * wait below, folded.
 */
const props = defineProps<{ filter: ReportFilter }>()

const { t } = useI18n()
const report = useReport('networth', () => props.filter)
const worth = computed(() => (report.data.value && 'networth' in report.data.value ? report.data.value.networth : null))
const range = computed(() => {
  const fallback = defaultRange()
  return { from: props.filter.from ?? fallback.from, to: props.filter.to ?? fallback.to }
})
const details = ref(false)
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <SectionState :loading="report.isPending.value" :error="report.error.value" skeleton="report" @retry="report.refetch()">
      <template v-if="worth?.has_data">
        <WealthHead :points="worth.points" :currency="worth.meta.currency" :range="range">
          <WealthChart :points="worth.points" :currency="worth.meta.currency" />
        </WealthHead>
        <WealthTable :point="worth.points.at(-1)" :currency="worth.meta.currency" />
      </template>
      <EmptyState v-else-if="worth" :title="t('money.wealth.empty')" />
    </SectionState>
    <Collapsible v-model:open="details" class="grid gap-3">
      <CollapsibleTrigger as-child>
        <Button variant="ghost" size="sm" class="w-fit text-muted-foreground" data-wealth-details>
          {{ t('money.wealth.details') }} <ChevronDown :class="['transition-transform', details && 'rotate-180']" />
        </Button>
      </CollapsibleTrigger>
      <CollapsibleContent class="grid gap-4">
        <template v-if="details">
          <AnalyticsReportView kind="networth" :filter="props.filter" />
          <h2 class="heading-section">{{ t('report.byWallet') }}</h2>
          <AnalyticsReportView kind="stores" :filter="props.filter" />
        </template>
      </CollapsibleContent>
    </Collapsible>
  </div>
</template>
