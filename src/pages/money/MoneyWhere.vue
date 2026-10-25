<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ChevronDown, Target } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { GoalDialog, type GoalRemainder } from '@/features/goal-edit'
import { AnalyticsReportView, useReport } from '@/widgets/analytics-reports'
import { drillTo } from '@/shared/lib/drill'
import { Button } from '@/shared/ui/button'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'
import { SectionState } from '@/shared/ui/section-state'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { FLOW_VIEWS, FLOW_VIEW_KIND, type FlowView, flowViewOf } from './model/money-groups'
import { WhereAnswer, WhereTable } from './ui/where'

/**
 * «Where it went» (Money 1 · 6): spent, lent, moved between one's own wallets,
 * invested, lost — and what is left of the income, ready to become a goal.
 * The month-by-month reports wait below, folded.
 */
const props = defineProps<{ filter: ReportFilter }>()

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const report = useReport('where', () => props.filter)
const where = computed(() => (report.data.value && 'where' in report.data.value ? report.data.value.where : null))
const left = computed(() => (where.value && Number(where.value.left) > 0 ? where.value.left : null))
const journal = computed(() => drillTo({ from: props.filter.from ?? undefined, to: props.filter.to ?? undefined, storeIds: props.filter.scope.store_ids }))

const goal = ref(false)
/** The goal wizard offers to put the remainder in at once (Money 2 · 8). */
const remainder = computed<GoalRemainder | null>(() =>
  left.value && where.value
    ? { amount: left.value, currency: where.value.meta.currency, month: (props.filter.from ?? new Date().toISOString()).slice(0, 7) }
    : null,
)

/** The view lives in the address: Back and a reload keep it. */
const details = ref(Boolean(route.query.view))
const view = computed({
  get: () => flowViewOf(route.query.view),
  set: (next: FlowView | undefined) => {
    if (next) void router.replace({ query: { ...route.query, view: next === 'months' ? undefined : next } })
  },
})
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <SectionState :loading="report.isPending.value" :error="report.error.value" skeleton="report" @retry="report.refetch()">
      <template v-if="where">
        <WhereAnswer :report="where" />
        <WhereTable v-if="where.rows.length" :report="where" :filter="props.filter" />
        <div class="flex flex-wrap justify-end gap-2 max-md:grid max-md:justify-stretch">
          <Button variant="outline" class="max-md:hidden" as-child>
            <RouterLink :to="journal">{{ t('money.where.toJournal') }}</RouterLink>
          </Button>
          <Button v-if="left" class="max-md:h-11" data-where-goal @click="goal = true">
            <Target />
            <i18n-t keypath="money.where.toGoal" tag="span" class="max-md:hidden">
              <template #left><Money :amount="left" :currency="where.meta.currency" :decimals="0" /></template>
            </i18n-t>
            <span class="md:hidden">{{ t('money.where.toGoalShort') }}</span>
          </Button>
        </div>
      </template>
    </SectionState>
    <Collapsible v-model:open="details" class="grid gap-3">
      <CollapsibleTrigger as-child>
        <Button variant="ghost" size="sm" class="w-fit text-muted-foreground">
          {{ t('money.where.details') }} <ChevronDown :class="['transition-transform', details && 'rotate-180']" />
        </Button>
      </CollapsibleTrigger>
      <CollapsibleContent class="grid gap-4">
        <ToggleGroup v-model="view" type="single" variant="outline" size="sm" class="w-fit" :aria-label="t('analytics.view.label')" data-tour="report-view">
          <ToggleGroupItem v-for="entry in FLOW_VIEWS" :key="entry" :value="entry" class="px-3">
            {{ t(`analytics.view.${entry}`) }}
          </ToggleGroupItem>
        </ToggleGroup>
        <AnalyticsReportView v-if="details" :kind="FLOW_VIEW_KIND[view]" :filter="props.filter" />
      </CollapsibleContent>
    </Collapsible>
    <GoalDialog v-model:open="goal" :remainder="remainder" />
  </div>
</template>
