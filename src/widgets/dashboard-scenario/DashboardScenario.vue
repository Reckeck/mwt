<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { CALC_SCENARIOS_KEY, useCalcScenarios } from '@/shared/api/queries'
import { buttonVariants } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { MetricTile } from '@/shared/ui/metric-tile'
import { SectionState } from '@/shared/ui/section-state'
import { useWidgetSize } from '@/shared/ui/widget-frame'

import type { DashboardScenarioProps } from './dashboard-scenario.types'

/**
 * «Сценарий» (plan H4): a calculator scenario's plan by today against what its
 * wallets (or its goal) hold. S — the fact and its gap; M — the plan too; L — the end.
 */
const props = withDefaults(defineProps<DashboardScenarioProps>(), { scenarioId: null })
const { t } = useI18n()
const size = useWidgetSize()

const scenarios = useCalcScenarios()
/** The chosen one while it exists, else the newest. */
const chosenId = computed(() => {
  const list = scenarios.data.value ?? []
  if (list.some((entry) => entry.id === props.scenarioId)) return props.scenarioId
  return [...list].sort((left, right) => right.updated_at.localeCompare(left.updated_at))[0]?.id ?? null
})

const status = useQuery({
  queryKey: computed(() => [...CALC_SCENARIOS_KEY, 'status', chosenId.value]),
  queryFn: () => call<ScenarioStatus>('get_scenario_status', { id: chosenId.value }),
  enabled: computed(() => chosenId.value !== null),
  placeholderData: keepPreviousData,
})

const gapPct = computed(() => {
  const data = status.data.value
  const planned = Number(data?.planned_now ?? 0)
  return data?.fact && planned > 0 ? ((Number(data.fact) - planned) / planned) * 100 : null
})
const link = computed(() => ({ name: 'money-calculator', query: chosenId.value ? { scenario: chosenId.value } : {} }))
</script>

<template>
  <Card size="sm">
    <SectionState
      :loading="scenarios.isPending.value || (chosenId !== null && status.isPending.value)"
      :error="scenarios.error.value ?? status.error.value"
      :empty="!chosenId"
      :empty-title="t('dashboard.scenario.empty')"
      skeleton="figure"
      compact
      @retry="chosenId ? status.refetch() : scenarios.refetch()"
    >
      <template #empty>
        <RouterLink :to="{ name: 'money-calculator' }" :class="buttonVariants({ variant: 'outline', size: 'sm' })">
          {{ t('dashboard.scenario.create') }}
        </RouterLink>
      </template>
      <div v-if="status.data.value" class="grid gap-2" data-tour="dash-scenario">
        <MetricTile
          :label="status.data.value.name"
          :delta-pct="gapPct"
          good-when="up"
          :base-label="t('dashboard.scenario.toPlan')"
          :insight="status.data.value.fact === null ? t('dashboard.scenario.noFact') : undefined"
          :action="{ label: t('dashboard.scenario.open'), to: link }"
        >
          <Money
            v-if="status.data.value.fact !== null"
            :amount="status.data.value.fact"
            :currency="status.data.value.currency"
            :decimals="0"
            :convert="false"
          />
          <template v-else>—</template>
        </MetricTile>
        <dl v-if="size !== 's'" class="grid gap-1 text-xs">
          <div class="flex justify-between gap-2">
            <dt class="text-muted-foreground">{{ t('goal.plannedNow') }}</dt>
            <dd class="tabular-nums">
              <Money :amount="status.data.value.planned_now" :currency="status.data.value.currency" :decimals="0" :convert="false" />
            </dd>
          </div>
          <div v-if="size === 'l'" class="flex justify-between gap-2">
            <dt class="text-muted-foreground">
              {{ t('dashboard.scenario.end', { done: status.data.value.elapsed_months, term: status.data.value.term_months }) }}
            </dt>
            <dd class="tabular-nums">
              <Money :amount="status.data.value.final_nominal" :currency="status.data.value.currency" :decimals="0" :convert="false" />
            </dd>
          </div>
        </dl>
      </div>
    </SectionState>
  </Card>
</template>
