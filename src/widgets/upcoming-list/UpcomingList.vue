<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Bell, CalendarClock, CircleAlert, Home, Landmark } from 'lucide-vue-next'

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
} as const
</script>

<template>
  <Card>
    <template #header>
      <p class="text-sm text-muted-foreground">{{ t('upcoming.title') }}</p>
    </template>
    <p v-if="upcoming.isSuccess.value && !items.length" class="text-sm text-muted-foreground">{{ t('upcoming.empty') }}</p>
    <ul class="grid gap-2">
      <li v-for="item in items" :key="`${item.kind}-${item.source_id}-${item.at}`" class="grid gap-1 text-sm">
        <div class="flex items-center gap-2">
          <component
            :is="ICONS[item.kind]"
            class="size-4 shrink-0"
            :class="item.kind === 'rejected_run' ? 'text-destructive' : 'text-muted-foreground'"
          />
          <span class="w-20 shrink-0 text-xs tabular-nums text-muted-foreground">{{ formatDate(item.date) }}</span>
          <span class="min-w-0 flex-1 truncate">
            {{ item.kind === 'rejected_run' ? t('upcoming.notCreated', { name: item.title }) : item.title }}
          </span>
          <span
            v-if="item.amount"
            class="shrink-0 tabular-nums"
            :class="Number(item.amount) > 0 ? 'text-emerald-600 dark:text-emerald-400' : ''"
          >
            {{ formatMoney(item.amount, { decimals: 0, currency: item.asset?.symbol }) }}
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
