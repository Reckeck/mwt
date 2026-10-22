<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useMutation, useQuery } from '@tanstack/vue-query'
import { ChevronDown, ChevronUp } from 'lucide-vue-next'

import { HEALTH_DOT } from '@/widgets/capital-health'
import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { comparisonDelta } from '@/shared/lib/comparison'
import { formatPct, signClass } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { useSettingsStore } from '@/shared/store/settings'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'

import TodayTodos from './ui/TodayTodos.vue'

/**
 * «Сегодня» above the grid (plan H5): the capital and its change over the period,
 * the checkup's light, what can be spent today and up to three things to do.
 * Collapsed, it is one line; the choice is the device's (appconfig).
 */
const props = defineProps<{ period: DashPeriod }>()
const { t } = useI18n()
const settings = useSettingsStore()

const today = useQuery({
  queryKey: computed(() => ['dashboard-today', props.period]),
  queryFn: () => call<DashboardToday>('get_dashboard_today', { period: props.period }),
  placeholderData: keepPreviousData,
})
const change = computed(() => comparisonDelta(today.data.value?.net_worth))

const collapsed = computed(() => settings.app?.dashboard_today_collapsed ?? false)
const toggle = useMutation({
  mutationFn: (value: boolean) => settings.updateApp({ dashboard_today_collapsed: value }),
  meta: { silent: 'the strip folds or unfolds in place', error: 'dashboard.today.toggleFailed' },
})
</script>

<template>
  <Card size="sm" data-tour="dash-today">
    <SectionState
      :loading="today.isPending.value"
      :error="today.error.value"
      skeleton="figure"
      compact
      @retry="today.refetch()"
    >
      <div v-if="today.data.value" class="grid gap-2">
        <div class="flex min-w-0 flex-wrap items-center gap-x-5 gap-y-1">
          <p class="text-sm font-medium">{{ t('dashboard.today.title') }}</p>
          <p class="flex min-w-0 items-baseline gap-1.5 text-sm">
            <span class="text-muted-foreground">{{ t('dashboard.today.netWorth') }}</span>
            <Money
              class="font-semibold"
              :amount="today.data.value.net_worth.value"
              :currency="today.data.value.currency"
              :decimals="0"
              :convert="false"
            />
            <span v-if="change.pct !== null" class="text-xs tabular-nums" :class="signClass(change.pct)">
              {{ formatPct(change.pct, { signed: true, decimals: 1 }) }}
            </span>
          </p>
          <p v-if="today.data.value.health" class="flex items-center gap-1.5 text-sm">
            <span :class="cn('size-2.5 rounded-full', HEALTH_DOT[today.data.value.health])" aria-hidden="true" />
            <span class="text-muted-foreground">{{ t(`health.color.${today.data.value.health}`) }}</span>
          </p>
          <p class="flex items-baseline gap-1.5 text-sm">
            <span class="text-muted-foreground">{{ t('dashboard.today.leftToday') }}</span>
            <Money
              class="font-semibold"
              :amount="today.data.value.left_today"
              :currency="today.data.value.budget_currency"
              :decimals="0"
              :convert="false"
            />
          </p>
          <Button
            variant="ghost"
            size="icon-sm"
            class="-my-1 ml-auto"
            :aria-label="collapsed ? t('dashboard.today.expand') : t('dashboard.today.collapse')"
            :title="collapsed ? t('dashboard.today.expand') : t('dashboard.today.collapse')"
            :aria-expanded="!collapsed"
            :disabled="toggle.isPending.value"
            @click="toggle.mutate(!collapsed)"
          >
            <ChevronDown v-if="collapsed" class="size-4" />
            <ChevronUp v-else class="size-4" />
          </Button>
        </div>
        <template v-if="!collapsed">
          <TodayTodos
            v-if="today.data.value.todos.length"
            :todos="today.data.value.todos"
            :currency="today.data.value.budget_currency"
          />
          <p v-else class="text-xs text-muted-foreground">{{ t('dashboard.today.nothing') }}</p>
        </template>
      </div>
    </SectionState>
  </Card>
</template>
