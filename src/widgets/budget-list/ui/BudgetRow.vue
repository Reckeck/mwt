<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink } from 'vue-router'

import { Money } from '@/entities/currency'
import { drillTo, localDay } from '@/shared/lib/drill'
import { Badge } from '@/shared/ui/badge'
import { ProgressBar } from '@/shared/ui/progress-bar'

import { budgetBar, daysLeft } from '../lib/budget-view'

/**
 * A Budget row: the name, what is spent of the limit, a bar with the limit's tick and what is left.
 * Actions of the owner (edit, delete) come in the `actions` slot, so the month screen shows the same row read-only.
 */
const props = defineProps<{ budget: Budget; status: BudgetStatus | null; name: string }>()
const { t } = useI18n()

const bar = computed(() => budgetBar(Number(props.status?.used ?? 0), props.budget.warn))
const remaining = computed(() => Number(props.status?.remaining ?? props.budget.limit))
const perDay = computed(() => (props.status && remaining.value > 0 ? remaining.value / daysLeft(props.status.period_end) : null))

/** The operations the spent figure is made of: the budget's period, category or wallets. */
const spentLink = computed(() => {
  if (!props.status) return null
  const target = props.budget.target
  return drillTo({
    from: localDay(props.status.period_start),
    to: localDay(props.status.period_end, true),
    kinds: ['payment'],
    category: typeof target === 'object' && 'category' in target ? target.category : undefined,
    storeIds: typeof target === 'object' && 'store' in target ? target.store.store_ids : undefined,
  })
})
</script>

<template>
  <div class="grid gap-2 py-3" :data-focus-row="props.budget.id" data-budget-row>
    <div class="flex items-start justify-between gap-2">
      <div class="grid min-w-0 gap-0.5">
        <div class="flex flex-wrap items-center gap-2">
          <p class="min-w-0 truncate font-medium">{{ props.name }}</p>
          <Badge variant="outline">{{ t(`budget.periodKind.${props.budget.period}`) }}</Badge>
          <Badge v-if="!props.budget.active" variant="outline">{{ t('budget.paused') }}</Badge>
        </div>
        <p class="text-sm tabular-nums text-muted-foreground">
          <RouterLink v-if="spentLink" :to="spentLink" class="hover:underline" data-tour="budget-spent">
            <Money bare :amount="props.status?.spent ?? '0'" :currency="props.budget.currency" :decimals="0" />
          </RouterLink>
          <Money v-else bare amount="0" :currency="props.budget.currency" :decimals="0" />
          /
          <Money :amount="props.budget.limit" :currency="props.budget.currency" :decimals="0" />
        </p>
      </div>
      <div v-if="$slots.actions" class="flex shrink-0 items-center gap-1"><slot name="actions" /></div>
    </div>
    <template v-if="props.budget.active">
      <ProgressBar :value="bar.value" :tone="bar.tone" :marker="bar.marker" :label="props.name" />
      <p class="text-xs tabular-nums" :class="remaining < 0 ? 'text-destructive' : 'text-muted-foreground'">
        <i18n-t v-if="remaining < 0" keypath="budget.over">
          <template #amount><Money :amount="String(-remaining)" :currency="props.budget.currency" :decimals="0" /></template>
        </i18n-t>
        <template v-else>
          <i18n-t keypath="budget.left">
            <template #amount><Money :amount="String(remaining)" :currency="props.budget.currency" :decimals="0" /></template>
          </i18n-t>
          <template v-if="perDay !== null">
            · <Money :amount="String(Math.round(perDay))" :currency="props.budget.currency" :decimals="0" />{{ t('budget.perDayShort') }}
          </template>
        </template>
      </p>
    </template>
  </div>
</template>
