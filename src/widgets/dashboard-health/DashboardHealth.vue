<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'
import { ArrowRight } from 'lucide-vue-next'

import { HEALTH_DOT, percent, reasonText } from '@/widgets/capital-health'
import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { CAPITAL_HEALTH_KEY } from '@/shared/api/queries'
import { formatQuantity } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'
import { useWidgetSize } from '@/shared/ui/widget-frame'

/**
 * «Чекап» on the dashboard (plan H4): S — the light; M — the first reason and its
 * step; L — the fund's months, the debt load and the freedom index too.
 */
const { t } = useI18n()
const size = useWidgetSize()

const report = useQuery({
  queryKey: CAPITAL_HEALTH_KEY,
  queryFn: () => call<HealthReport>('capital_health'),
  placeholderData: keepPreviousData,
})

const first = computed(() => {
  const data = report.data.value
  const reason = data?.reasons[0]
  return data && reason ? reasonText(reason, data.currency, t) : null
})

/** The three figures of «L»; an unknown one shows a dash, so the row keeps its place. */
const figures = computed(() => {
  const data = report.data.value
  if (!data) return []
  const dash = '—'
  return [
    {
      key: 'fund',
      label: t('dashboard.health.fund'),
      value:
        data.emergency.months_covered === null
          ? dash
          : t('dashboard.health.months', {
              months: formatQuantity(data.emergency.months_covered, 1),
              target: data.emergency.fund.target_months,
            }),
    },
    { key: 'dti', label: t('dashboard.health.dti'), value: data.debts.dti_pct === null ? dash : percent(data.debts.dti_pct) },
    {
      key: 'freedom',
      label: t('dashboard.health.freedom'),
      value: data.passive.freedom_fact_pct === null ? dash : percent(data.passive.freedom_fact_pct),
    },
  ]
})
</script>

<template>
  <Card size="sm">
    <SectionState
      :loading="report.isPending.value"
      :error="report.error.value"
      skeleton="figure"
      compact
      @retry="report.refetch()"
    >
      <div v-if="report.data.value" class="grid gap-2" data-tour="dash-health">
        <p class="truncate text-xs text-muted-foreground">{{ t('dashboard.widget.health') }}</p>
        <RouterLink
          :to="{ name: 'money-health' }"
          class="flex min-w-0 items-center gap-2 rounded-sm hover:underline focus-visible:outline-2 focus-visible:outline-ring"
        >
          <span :class="cn('size-3 shrink-0 rounded-full', HEALTH_DOT[report.data.value.color])" aria-hidden="true" />
          <span class="truncate font-semibold">{{ t(`health.color.${report.data.value.color}`) }}</span>
        </RouterLink>
        <template v-if="size !== 's'">
          <p v-if="first" class="line-clamp-2 text-xs">
            <template v-for="(part, at) in first.what" :key="at">
              <Money v-if="'amount' in part" v-bind="part" />
              <template v-else>{{ part.text }}</template>
            </template>
          </p>
          <p v-if="first" class="line-clamp-2 text-xs text-muted-foreground">
            <template v-for="(part, at) in first.step" :key="at">
              <Money v-if="'amount' in part" v-bind="part" />
              <template v-else>{{ part.text }}</template>
            </template>
          </p>
          <p v-else class="line-clamp-2 text-xs text-muted-foreground">
            {{ t(`health.colorHint.${report.data.value.color}`) }}
          </p>
        </template>
        <dl v-if="size === 'l'" class="grid grid-cols-3 gap-2 text-xs">
          <div v-for="figure in figures" :key="figure.key" class="min-w-0">
            <dt class="truncate text-muted-foreground">{{ figure.label }}</dt>
            <dd class="truncate font-medium tabular-nums">{{ figure.value }}</dd>
          </div>
        </dl>
        <Button v-if="size !== 's'" variant="link" size="sm" class="h-auto self-start p-0" as-child>
          <RouterLink :to="{ name: 'money-health' }">
            {{ t('dashboard.health.open') }}
            <ArrowRight class="size-3.5" />
          </RouterLink>
        </Button>
      </div>
    </SectionState>
  </Card>
</template>
