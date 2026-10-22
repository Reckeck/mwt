<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink, useRoute } from 'vue-router'
import { Plus } from 'lucide-vue-next'

import { QuickAddDialog } from '@/features/quick-add'
import { Money, PnlText } from '@/entities/currency'
import { monthName } from '@/entities/report'
import { Button, buttonVariants } from '@/shared/ui/button'
import { formatPct } from '@/shared/lib/format'
import { activeLocale } from '@/shared/lib/locale'
import { useTxContext } from '@/shared/lib/tx-context'
import { SectionState } from '@/shared/ui/section-state'

import type { AnalyticsOverviewProps } from './analytics-overview.types'
import { useOverview } from './model/use-overview'
import { OverviewCard } from './ui/overview-card'
import { OverviewFigure } from './ui/overview-figure'

/** The analytics start screen: four questions, each with its answer in words. */
const props = defineProps<AnalyticsOverviewProps>()

const { t } = useI18n()
const route = useRoute()
const overview = useOverview(() => props.filter)
const data = computed(() => overview.data.value ?? null)
const adding = ref(false)

/** The same filter rides along to the report a card opens; the overview's month becomes its period. */
function linkTo(name: string, view?: string) {
  const { month, ...query } = route.query
  const period = typeof month === 'string' ? { from: month, to: month } : {}
  return { name, query: { ...query, ...period, ...(view ? { view } : {}) } }
}
const context = useTxContext()
/** This month's spending against the last one; positive means more was spent. */
const spentDelta = computed(() => (data.value ? Number(data.value.flow.spent) - Number(data.value.flow.previous_spent) : 0))
const month = computed(() => (data.value ? monthName(data.value.month, activeLocale.value) : ''))
</script>

<template>
  <SectionState
    :loading="overview.isPending.value"
    :error="overview.error.value"
    skeleton="chart"
    @retry="overview.refetch()"
  >
    <div v-if="data" class="grid grid-cols-1 gap-4 @3xl:grid-cols-2" data-tour="overview-cards">
      <OverviewCard
        :title="t('analytics.group.flow')"
        :period="t('analytics.overview.periodMonth', { month })"
        :to="linkTo('money-where')"
        :has-data="data.flow.has_data"
        :insights="data.flow.insights"
        :currency="data.currency"
        :empty-title="t('analytics.overview.flowEmpty')"
        :empty-description="t('analytics.overview.flowEmptyHint')"
      >
        <OverviewFigure :label="t('report.cashflow.income')" :hint="t('report.term.income')">
          <Money :amount="data.flow.income" :currency="data.currency" :decimals="0" />
        </OverviewFigure>
        <OverviewFigure :label="t('report.cashflow.spent')" :hint="t('report.term.spent')">
          <Money :amount="data.flow.spent" :currency="data.currency" :decimals="0" />
        </OverviewFigure>
        <OverviewFigure :label="t('report.cashflow.netTotal')" :hint="t('report.term.net')">
          <Money :amount="data.flow.net" :currency="data.currency" :decimals="0" colored />
        </OverviewFigure>
        <!-- Spending more than last month is the bad direction: red, not green. -->
        <OverviewFigure :label="t('analytics.overview.vsPrevMonth')">
          <span :class="spentDelta > 0 ? 'text-destructive' : spentDelta < 0 ? 'text-success' : ''">
            <Money :amount="spentDelta" :currency="data.currency" :decimals="0" signed />
          </span>
        </OverviewFigure>
        <OverviewFigure v-if="data.flow.top_category" :label="t('analytics.overview.topCategory')">
          {{ context.categoryName(data.flow.top_category) }}
        </OverviewFigure>
        <template #action>
          <Button size="sm" @click="adding = true"><Plus class="size-4" />{{ t('analytics.overview.addTx') }}</Button>
        </template>
      </OverviewCard>

      <OverviewCard
        :title="t('analytics.group.wealth')"
        :period="t('analytics.overview.periodMonth', { month })"
        :to="linkTo('money-wealth')"
        :has-data="data.wealth.has_data"
        :insights="data.wealth.insights"
        :currency="data.currency"
        :empty-title="t('analytics.overview.wealthEmpty')"
        :empty-description="t('analytics.overview.wealthEmptyHint')"
      >
        <OverviewFigure :label="t('report.networth.net')" :hint="t('report.term.networth')">
          <Money :amount="data.wealth.net" :currency="data.currency" :decimals="0" />
        </OverviewFigure>
        <OverviewFigure :label="t('analytics.overview.monthChange')">
          <PnlText :amount="data.wealth.change" :currency="data.currency" :decimals="0" />
        </OverviewFigure>
        <template #action>
          <RouterLink :to="{ name: 'stores' }" :class="buttonVariants({ size: 'sm' })">{{ t('analytics.overview.toWallets') }}</RouterLink>
        </template>
      </OverviewCard>

      <OverviewCard
        :title="t('analytics.group.investments')"
        :period="t('analytics.overview.periodYear', { year: data.year })"
        :to="linkTo('money-investments')"
        :has-data="data.investments.has_data"
        :insights="data.investments.insights"
        :currency="data.currency"
        :empty-title="t('analytics.overview.investEmpty')"
        :empty-description="t('analytics.overview.investEmptyHint')"
      >
        <OverviewFigure :label="t('report.pnl.realized')" :hint="t('report.term.realized')">
          <PnlText :amount="data.investments.realized" :currency="data.currency" :decimals="0" />
        </OverviewFigure>
        <OverviewFigure :label="t('report.pnl.fees')">
          <Money :amount="data.investments.fees" :currency="data.currency" :decimals="0" />
        </OverviewFigure>
        <OverviewFigure :label="t('analytics.overview.losses')" :hint="t('report.term.losses')">
          <Money :amount="data.investments.losses" :currency="data.currency" :decimals="0" />
        </OverviewFigure>
        <template #action>
          <Button size="sm" variant="outline" @click="adding = true"><Plus class="size-4" />{{ t('analytics.overview.addTrade') }}</Button>
        </template>
      </OverviewCard>

      <OverviewCard
        :title="t('analytics.view.year')"
        :period="t('analytics.overview.periodYear', { year: data.year })"
        :to="linkTo('money-where', 'year')"
        :has-data="data.savings.has_data"
        :insights="data.savings.insights"
        :currency="data.currency"
        :empty-title="t('analytics.overview.yearEmpty')"
        :empty-description="t('analytics.overview.yearEmptyHint')"
      >
        <OverviewFigure :label="t('report.year.saved', { year: data.year })" :hint="t('report.term.savings_fact')">
          <Money :amount="data.savings.saved" :currency="data.currency" :decimals="0" colored />
        </OverviewFigure>
        <OverviewFigure :label="t('analytics.overview.savedShare')" :hint="t('report.term.saved_pct')">
          {{ formatPct(data.savings.saved_pct, { decimals: 0 }) }}
        </OverviewFigure>
        <template #action>
          <Button size="sm" @click="adding = true"><Plus class="size-4" />{{ t('analytics.overview.addTx') }}</Button>
        </template>
      </OverviewCard>
    </div>
  </SectionState>
  <QuickAddDialog v-model:open="adding" />
</template>
