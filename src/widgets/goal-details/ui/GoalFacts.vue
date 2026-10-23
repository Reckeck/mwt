<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink, useRoute } from 'vue-router'

import { EntityLink } from '@/shared/ui/entity-link'
import { backQuery } from '@/shared/lib/drill'
import { formatMonthKey, pluralForm } from '@/shared/lib/format'
import { storeTxFilter, txFilterToQuery } from '@/shared/lib/tx-filter'

/**
 * «Где копим · К дате · Пополнение · Прогноз · История» (New-Goal 3): the
 * wallet opens its glance (the `wallet` slot), the automation its card, the
 * history the wallet's operations with a chip back here.
 */
const props = defineProps<{ entry: GoalProgress; store: string | null; storeName: string; filler: RelationItem | null }>()

defineSlots<{ wallet?(props: { storeId: string; label: string }): unknown }>()

const { t, locale } = useI18n()
const route = useRoute()

const month = (iso: string) => formatMonthKey(iso, { month: 'long', year: 'numeric' }).replace(/\s*г\.$/, '')
const deadline = computed(() => (props.entry.goal.deadline ? month(props.entry.goal.deadline) : ''))
const forecast = computed(() => {
  const { forecast: at, realistic, goal } = props.entry
  if (!at) return props.entry.accumulating ? '' : t('goal.notAccumulating')
  const late = !realistic || (goal.deadline !== null && at.slice(0, 7) > goal.deadline.slice(0, 7))
  return t(late ? 'goalCard.forecastLate' : 'goalCard.forecastOnTime', { date: month(at) })
})
const deposits = computed(() =>
  t(`goalCard.deposits.${pluralForm(props.entry.deposits, locale.value)}`, { count: props.entry.deposits }),
)
const historyTo = computed(() => {
  if (!props.store) return null
  const query: Record<string, string> = {}
  for (const [key, value] of Object.entries(txFilterToQuery(storeTxFilter(props.store)))) if (value) query[key] = value
  return { name: 'transactions', query: { ...query, ...backQuery({ to: route.fullPath, label: props.entry.goal.name }) } }
})

const ROW = 'flex items-baseline justify-between gap-4 py-3'
</script>

<template>
  <dl class="grid divide-y divide-border border-y text-sm" data-goal-facts>
    <div v-if="props.store" :class="ROW">
      <dt class="text-muted-foreground">{{ t('goal.where') }}</dt>
      <dd class="min-w-0 break-words pb-1 text-right">
        <slot name="wallet" :store-id="props.store" :label="props.storeName">
          <EntityLink :target="{ kind: 'store', id: props.store }" :label="props.storeName" />
        </slot>
      </dd>
    </div>
    <div v-if="deadline" :class="ROW">
      <dt class="text-muted-foreground">{{ t('goal.deadline') }}</dt>
      <dd class="text-right">{{ deadline }}</dd>
    </div>
    <div v-if="props.filler" :class="ROW">
      <dt class="text-muted-foreground">{{ t('goalCard.filling') }}</dt>
      <dd class="min-w-0 break-words pb-1 text-right" data-goal-filler>
        <EntityLink :target="props.filler.target" :label="props.filler.label" />
        <span class="text-muted-foreground"> · {{ t('goalCard.auto') }}</span>
      </dd>
    </div>
    <div v-if="forecast" :class="ROW">
      <dt class="text-muted-foreground">{{ t('goalCard.forecast') }}</dt>
      <dd class="text-right">{{ forecast }}</dd>
    </div>
    <div v-if="historyTo && props.entry.deposits" :class="ROW">
      <dt class="text-muted-foreground">{{ t('goalCard.history') }}</dt>
      <dd class="text-right">
        <RouterLink :to="historyTo" class="touch-hit-lg text-primary hover:underline" data-goal-history>{{ deposits }} →</RouterLink>
      </dd>
    </div>
  </dl>
</template>
