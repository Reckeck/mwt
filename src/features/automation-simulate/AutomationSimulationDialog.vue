<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { FlaskConical } from 'lucide-vue-next'
import { Money } from '@/entities/currency'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { call } from '@/shared/api/ipc'
import { formatDate, formatMoney } from '@/shared/lib/format'
import { rejectText } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { amountClass, txLegs } from '@/shared/lib/transactions'

import { SIMULATION_HORIZONS } from './automation-simulate.constants'
import type { AutomationSimulationDialogProps } from './automation-simulate.types'

/**
 * «Проверить»: what the automations would do over the next days, played out
 * on a copy of the vault. Nothing is written, so it is safe to try anything.
 */
const props = withDefaults(defineProps<AutomationSimulationDialogProps>(), { automationId: null, name: '' })
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const days = ref<number>(90)

const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const storeName = (id: string) =>
  flattenStoreTree(tree.data.value ?? []).find((node) => node.store.id === id)?.path.join(' › ') ?? id

const simulation = useQuery({
  queryKey: computed(() => ['automation-simulation', props.automationId, days.value]),
  queryFn: () =>
    call<AutomationSimulation>('simulate_automations', {
      input: { automation_id: props.automationId, days: days.value } satisfies SimulateAutomationsInput,
    }),
  enabled: open,
  staleTime: 0,
})

function pickDays(value: unknown) {
  if (typeof value === 'string' && value) days.value = Number(value)
}

/** «+1 000 USD · Card» from the operation the run would create. */
function runLine(run: SimulatedRun): string {
  if (run.outcome === 'skipped') return t('automation.run.outcome.skipped')
  if ('rejected' in run.outcome) return rejectText(run.outcome.rejected.reason, t, storeName)
  const leg = run.transaction ? txLegs(run.transaction)[0] : null
  return leg
    ? `${formatMoney(leg.amount, { currency: leg.asset.symbol, signed: true })} · ${storeName(leg.store_id)}`
    : t('automation.run.outcome.created')
}
const rejected = (run: SimulatedRun) => typeof run.outcome === 'object' && 'rejected' in run.outcome
</script>

<template>
  <Dialog
    v-model:open="open"
    :title="props.name ? t('simulation.titleFor', { name: props.name }) : t('simulation.title')"
    :description="t('simulation.hint')"
    size="lg"
  >
    <div class="grid gap-4">
      <ToggleGroup
        type="single"
        variant="outline"
        size="sm"
        class="justify-start"
        :spacing="1"
        :model-value="String(days)"
        @update:model-value="pickDays"
      >
        <ToggleGroupItem v-for="entry in SIMULATION_HORIZONS" :key="entry" :value="String(entry)">
          {{ t('simulation.days', { count: entry }) }}
        </ToggleGroupItem>
      </ToggleGroup>

      <p v-if="simulation.isFetching.value && !simulation.data.value" class="text-sm text-muted-foreground">
        {{ t('simulation.running') }}
      </p>
      <template v-else-if="simulation.data.value">
        <div class="flex flex-wrap items-center gap-2 text-sm">
          <FlaskConical class="size-4 text-muted-foreground" />
          <Badge variant="secondary">{{ t('simulation.created', { count: simulation.data.value.created }) }}</Badge>
          <Badge :variant="simulation.data.value.rejected ? 'destructive' : 'outline'">
            {{ t('simulation.rejected', { count: simulation.data.value.rejected }) }}
          </Badge>
        </div>

        <p v-if="!simulation.data.value.runs.length" class="text-sm text-muted-foreground">{{ t('simulation.nothing') }}</p>
        <ul v-else class="grid max-h-80 gap-1 overflow-y-auto">
          <li
            v-for="run in simulation.data.value.runs"
            :key="`${run.automation_id}:${run.planned_at}`"
            class="grid grid-cols-[6.5rem_minmax(0,1fr)] items-baseline gap-2 rounded-md bg-muted/40 px-3 py-1.5 text-sm"
          >
            <span class="tabular-nums text-muted-foreground">{{ formatDate(run.planned_at) }}</span>
            <span class="min-w-0">
              <span v-if="!props.automationId" class="font-medium">{{ run.automation_name }} · </span>
              <span :class="rejected(run) && 'text-destructive'">{{ runLine(run) }}</span>
            </span>
          </li>
        </ul>

        <div v-if="simulation.data.value.balances.length" class="grid gap-1">
          <p class="text-xs font-medium text-muted-foreground">{{ t('simulation.balances') }}</p>
          <p
            v-for="row in simulation.data.value.balances"
            :key="`${row.store_id}:${row.asset.symbol}`"
            class="flex flex-wrap items-baseline justify-between gap-2 text-sm"
          >
            <span class="min-w-0 truncate">{{ storeName(row.store_id) }}</span>
            <span class="tabular-nums">
              <span class="text-muted-foreground"><Money :amount="row.before" :currency="row.asset.symbol" /></span>
              →
              <span :class="amountClass(row.after)"><Money :amount="row.after" :currency="row.asset.symbol" /></span>
            </span>
          </p>
        </div>
      </template>
    </div>
    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.close') }}</Button>
    </template>
  </Dialog>
</template>
