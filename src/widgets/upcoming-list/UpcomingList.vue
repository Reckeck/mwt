<script setup lang="ts">
import { computed } from 'vue'
import { type RouteLocationRaw, RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'
import { Bell, CalendarClock, CircleAlert, HandCoins, Home, Landmark } from 'lucide-vue-next'
import { Money } from '@/entities/currency'

import { AutomationRunActions } from '@/features/automation-edit'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { formatDate } from '@/shared/lib/format'

import type { UpcomingListProps } from './upcoming-list.types'
import { SectionState } from '@/shared/ui/section-state'

const props = withDefaults(defineProps<UpcomingListProps>(), { days: 30, limit: 8 })

const { t } = useI18n()

const upcoming = useQuery({
  queryKey: computed(() => ['upcoming', props.days]),
  placeholderData: keepPreviousData,
  queryFn: () => call<UpcomingItem[]>('get_upcoming', { days: props.days }),
})
const runs = useQuery({
  queryKey: ['automation-runs'],
  queryFn: () => call<AutomationRun[]>('list_automation_runs', { automationId: null }),
})

const items = computed(() => (upcoming.data.value ?? []).slice(0, props.limit))
/** The list by day: one date header over its lines instead of a date on every line. */
const byDay = computed(() => {
  const groups: { date: string; items: UpcomingItem[] }[] = []
  for (const item of items.value) {
    const last = groups.at(-1)
    if (last?.date === item.date) last.items.push(item)
    else groups.push({ date: item.date, items: [item] })
  }
  return groups
})
/** «Will the money last»: the first payment the accounts can't cover, or none. */
const firstShort = computed(() => (upcoming.data.value ?? []).find((item) => item.shortfall) ?? null)
/** The run behind an item: a refused run by its id, an approximate one by the operation it made. */
const runOf = (item: UpcomingItem) =>
  runs.data.value?.find((run) =>
    item.kind === 'unconfirmed_run'
      ? typeof run.outcome === 'object' && 'created' in run.outcome && run.outcome.created.tx_id === item.source_id
      : run.id === item.source_id,
  ) ?? null

/** What the line says: a refusal and an approximate amount say what they wait for. */
function titleOf(item: UpcomingItem): string {
  if (item.kind === 'rejected_run') return t('upcoming.notCreated', { name: item.title })
  if (item.kind === 'unconfirmed_run') return t('upcoming.unconfirmed', { name: item.title })
  return item.title
}

const ICON_COLOR: Partial<Record<UpcomingKind, string>> = {
  rejected_run: 'text-destructive',
  unconfirmed_run: 'text-warning',
}

const ICONS = {
  automation: CalendarClock,
  housing: Home,
  loan_payment: Landmark,
  reminder: Bell,
  rejected_run: CircleAlert,
  unconfirmed_run: CircleAlert,
  debt: HandCoins,
} as const

/** Where an item is managed: its list, its loan wallet or its contact. A refused run keeps its own buttons. */
function targetOf(item: UpcomingItem): RouteLocationRaw | null {
  switch (item.kind) {
    case 'automation':
      return { name: 'automations' }
    case 'reminder':
      return { name: 'automations-reminders' }
    case 'loan_payment':
      return { name: 'wallet', params: { id: item.source_id } }
    case 'debt':
      return { name: 'transactions', query: { contact: item.source_id } }
    default:
      return null
  }
}
</script>

<template>
  <!-- min-w-0 / minmax(0,1fr): a truncated title must not widen the card past its column. -->
  <Card size="sm" class="min-w-0">
    <template #header>
      <p data-widget-title class="text-sm text-muted-foreground">{{ t('upcoming.title') }}</p>
    </template>
    <SectionState
      v-if="upcoming.isPending.value || upcoming.error.value"
      :loading="upcoming.isPending.value"
      :error="upcoming.error.value"
      :rows="3"
      compact
      @retry="upcoming.refetch()"
    />
    <p v-else-if="!items.length" class="text-sm text-muted-foreground">{{ t('upcoming.empty') }}</p>
    <p
      v-else
      class="mb-2 text-xs"
      :class="firstShort ? 'text-destructive' : 'text-success'"
      data-tour="dash-upcoming-coverage"
    >
      <template v-if="firstShort">
        {{ t('upcoming.coverShort', { date: formatDate(firstShort.date) }) }}
        <Money :amount="firstShort.shortfall" :decimals="0" :currency="firstShort.asset?.symbol" />
      </template>
      <template v-else>{{ t('upcoming.coverOk') }}</template>
    </p>
    <ul class="grid grid-cols-[minmax(0,1fr)] gap-3">
      <li v-for="day in byDay" :key="day.date" class="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-1">
        <p class="text-xs tabular-nums text-muted-foreground">{{ formatDate(day.date) }}</p>
        <ul class="grid grid-cols-[minmax(0,1fr)] gap-2">
      <li
        v-for="item in day.items"
        :key="`${item.kind}-${item.source_id}-${item.at}`"
        class="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-1 text-sm"
      >
        <div class="flex items-center gap-2">
          <component
            :is="ICONS[item.kind]"
            class="size-4 shrink-0"
            :class="ICON_COLOR[item.kind] ?? 'text-muted-foreground'"
          />
          <RouterLink
            v-if="targetOf(item)"
            :to="targetOf(item) as RouteLocationRaw"
            class="min-w-0 flex-1 truncate hover:underline"
            :title="item.title"
          >
            {{ item.title }}
          </RouterLink>
          <span v-else class="min-w-0 flex-1 truncate" :title="item.title">
            {{ titleOf(item) }}
          </span>
          <span
            v-if="item.amount"
            class="shrink-0 tabular-nums"
            :class="Number(item.amount) > 0 ? 'text-success' : ''"
          >
            <template v-if="item.estimated">~</template><Money :amount="item.amount" :decimals="0" :currency="item.asset?.symbol" />
          </span>
        </div>
        <p v-if="item.shortfall" class="pl-6 text-xs text-destructive">
          {{ t('upcoming.shortLabel') }} <Money :amount="item.shortfall" :decimals="0" :currency="item.asset?.symbol" />
        </p>
        <div v-if="(item.kind === 'rejected_run' || item.kind === 'unconfirmed_run') && runOf(item)" class="pl-6">
          <AutomationRunActions :run="runOf(item) as AutomationRun" :name="item.title" />
        </div>
      </li>
        </ul>
      </li>
    </ul>
  </Card>
</template>
