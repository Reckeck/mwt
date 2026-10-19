<script setup lang="ts">
import { computed } from 'vue'
import { type RouteLocationRaw, RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Bell, CalendarClock, CircleAlert, HandCoins, Home, Landmark } from 'lucide-vue-next'
import { Money } from '@/entities/currency'

import { AutomationRunActions } from '@/features/automation-edit'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { formatDate, formatMoney } from '@/shared/lib/format'

import type { UpcomingListProps } from './upcoming-list.types'

const props = withDefaults(defineProps<UpcomingListProps>(), { days: 30, limit: 8 })

const { t } = useI18n()

const upcoming = useQuery({
  queryKey: computed(() => ['upcoming', props.days]),
  queryFn: () => call<UpcomingItem[]>('get_upcoming', { days: props.days }),
})
const runs = useQuery({
  queryKey: ['automation-runs'],
  queryFn: () => call<AutomationRun[]>('list_automation_runs', { automationId: null }),
})

const items = computed(() => (upcoming.data.value ?? []).slice(0, props.limit))
const runOf = (id: string) => runs.data.value?.find((run) => run.id === id) ?? null

const ICONS = {
  automation: CalendarClock,
  housing: Home,
  loan_payment: Landmark,
  reminder: Bell,
  rejected_run: CircleAlert,
  debt: HandCoins,
} as const

/** Where an item is managed: its list, its loan wallet or its contact. A refused run keeps its own buttons. */
function targetOf(item: UpcomingItem): RouteLocationRaw | null {
  switch (item.kind) {
    case 'automation':
      return { name: 'planning-automations' }
    case 'reminder':
      return { name: 'planning-reminders' }
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
    <p v-if="upcoming.isSuccess.value && !items.length" class="text-sm text-muted-foreground">{{ t('upcoming.empty') }}</p>
    <ul class="grid grid-cols-[minmax(0,1fr)] gap-2">
      <li
        v-for="item in items"
        :key="`${item.kind}-${item.source_id}-${item.at}`"
        class="grid min-w-0 grid-cols-[minmax(0,1fr)] gap-1 text-sm"
      >
        <div class="flex items-center gap-2">
          <component
            :is="ICONS[item.kind]"
            class="size-4 shrink-0"
            :class="item.kind === 'rejected_run' ? 'text-destructive' : 'text-muted-foreground'"
          />
          <span class="w-20 shrink-0 text-xs tabular-nums text-muted-foreground">{{ formatDate(item.date) }}</span>
          <RouterLink
            v-if="targetOf(item)"
            :to="targetOf(item) as RouteLocationRaw"
            class="min-w-0 flex-1 truncate hover:underline"
            :title="item.title"
          >
            {{ item.title }}
          </RouterLink>
          <span v-else class="min-w-0 flex-1 truncate" :title="item.title">
            {{ item.kind === 'rejected_run' ? t('upcoming.notCreated', { name: item.title }) : item.title }}
          </span>
          <span
            v-if="item.amount"
            class="shrink-0 tabular-nums"
            :class="Number(item.amount) > 0 ? 'text-success' : ''"
          >
            <Money :amount="item.amount" :decimals="0" :currency="item.asset?.symbol" />
          </span>
        </div>
        <p v-if="item.shortfall" class="pl-6 text-xs text-destructive">
          {{ t('upcoming.short', { amount: formatMoney(item.shortfall, { currency: item.asset?.symbol }) }) }}
        </p>
        <div v-if="item.kind === 'rejected_run' && runOf(item.source_id)" class="pl-6">
          <AutomationRunActions :run="runOf(item.source_id) as AutomationRun" />
        </div>
      </li>
    </ul>
  </Card>
</template>
