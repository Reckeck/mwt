<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { ChevronDown, FlaskConical, Pencil, Play, Plus, Trash2 } from 'lucide-vue-next'

import { AutomationDialog, AutomationRunActions } from '@/features/automation-edit'
import { AutomationSimulationDialog } from '@/features/automation-simulate'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { ListHeader } from '@/shared/ui/list-header'
import { Card } from '@/shared/ui/card'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { EmptyState } from '@/shared/ui/empty-state'
import { call } from '@/shared/api/ipc'
import { formatDate, formatMoney } from '@/shared/lib/format'
import { LEDGER_KEYS, PLANNING_KEYS, rejectText, scheduleText } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { txKind, txLegs } from '@/shared/lib/transactions'

import type { AutomationListProps } from './automation-list.types'
import { useNewFromRoute } from './model/use-new-from-route'

const props = withDefaults(defineProps<AutomationListProps>(), { storeId: '', presets: () => [] })

const { t } = useI18n()
const queryClient = useQueryClient()

const automations = useQuery({ queryKey: ['automations'], queryFn: () => call<Automation[]>('list_automations') })
const runs = useQuery({
  queryKey: ['automation-runs'],
  queryFn: () => call<AutomationRun[]>('list_automation_runs', { automationId: null }),
})
const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const storeName = (id: string) =>
  flattenStoreTree(tree.data.value ?? []).find((node) => node.store.id === id)?.path.join(' › ') ?? id

/** «−25 000 RUB · Тинькофф» from the template's first leg. */
function templateLine(automation: Automation): string {
  const tx = automation.template as Transaction
  const leg = txLegs(tx)[0]
  if (!leg) return t(`tx.kind.${txKind(tx)}`)
  return `${t(`tx.kind.${txKind(tx)}`)} · ${formatMoney(leg.amount, { currency: leg.asset.symbol })} · ${storeName(leg.store_id)}`
}

/** On a wallet: only the automations that move money in or out of it (or its sub-accounts). */
const scopeIds = computed(() => {
  if (!props.storeId) return null
  const node = flattenStoreTree(tree.data.value ?? []).find((entry) => entry.store.id === props.storeId)
  return new Set([props.storeId, ...(node?.children.map((child) => child.store.id) ?? [])])
})
const shown = computed(() =>
  (automations.data.value ?? []).filter((automation) => {
    const ids = scopeIds.value
    return !ids || txLegs(automation.template as Transaction).some((leg) => ids.has(leg.store_id))
  }),
)

const runsOf = (id: string) => (runs.data.value ?? []).filter((run) => run.automation_id === id)
const pending = (id: string) => runsOf(id).filter((run) => typeof run.outcome === 'object' && 'rejected' in run.outcome)

const expanded = ref<string | null>(null)
const dialogOpen = ref(false)
const editing = ref<Automation | null>(null)
const preset = ref<PresetInfo | null>(null)

function openNew(from: PresetInfo | null = null) {
  editing.value = null
  preset.value = from
  dialogOpen.value = true
}
useNewFromRoute(
  () => !props.readonly,
  () => props.presets,
  (from) => openNew(from),
)

function openEdit(automation: Automation) {
  editing.value = automation
  preset.value = null
  dialogOpen.value = true
}

/* --- «Проверить»: emulate ahead without writing anything --- */
const simulateOpen = ref(false)
const simulated = ref<Automation | null>(null)
function openSimulation(automation: Automation | null) {
  simulated.value = automation
  simulateOpen.value = true
}

function refresh() {
  for (const queryKey of [...PLANNING_KEYS, ...LEDGER_KEYS]) void queryClient.invalidateQueries({ queryKey })
}

const runNow = useMutation({
  mutationFn: (id: string) => call<Transaction>('run_automation', { id }),
  onSuccess: () => {
    refresh()
  },
  meta: { error: 'automation.runFailed', success: 'automation.ranNow' },
})

const remove = useMutation({
  mutationFn: (id: string) => call('delete_automation', { id }),
  onSuccess: () => {
    refresh()
  },
  meta: { error: 'automation.deleteFailed', success: 'automation.deleted' },
})

function outcomeText(run: AutomationRun): string {
  if (run.outcome === 'skipped') return t('automation.run.outcome.skipped')
  if ('created' in run.outcome) return t('automation.run.outcome.created')
  return rejectText(run.outcome.rejected.reason, t, storeName)
}

const empty = computed(() => automations.isSuccess.value && !shown.value.length)
</script>

<template>
  <div class="grid gap-4">
    <ListHeader
      :intro="props.storeId || props.readonly ? t('automation.intro') : t('automation.introWallet')"
      :action-label="props.readonly ? undefined : t('automation.new')"
      :hide-action="empty"
      @action="openNew()"
    >
      <template v-if="shown.length" #actions>
        <Button variant="outline" data-tour="automation-simulate" @click="openSimulation(null)">
          <FlaskConical class="size-4" />
          {{ t('simulation.open') }}
        </Button>
      </template>
    </ListHeader>

    <!-- Ready-made automations of this kind of wallet: one click, a few fields. -->
    <div v-if="props.presets.length && !props.readonly" class="flex flex-wrap gap-2" data-tour="automation-presets">
      <Button v-for="entry in props.presets" :key="entry.preset" size="sm" variant="secondary" @click="openNew(entry)">
        <Plus class="size-4" />
        {{ t(`preset.${entry.preset}`) }}
      </Button>
    </div>

    <EmptyState v-if="empty" compact :title="t('automation.empty')" :description="t('automation.intro')">
      <Button v-if="!props.readonly" @click="openNew()"><Plus class="size-4" />{{ t('automation.new') }}</Button>
    </EmptyState>

    <Card v-for="automation in shown" :key="automation.id">
      <div class="grid gap-3">
        <div class="flex flex-wrap items-start justify-between gap-2">
          <div class="grid min-w-0 gap-1">
            <div class="flex flex-wrap items-center gap-2">
              <p class="truncate font-medium">{{ automation.name }}</p>
              <Badge v-if="automation.preset" variant="secondary">{{ t(`preset.${automation.preset}`) }}</Badge>
              <Badge v-if="!automation.active" variant="outline">{{ t('automation.paused') }}</Badge>
              <Badge v-if="pending(automation.id).length" variant="destructive">
                {{ t('automation.pendingRuns', { count: pending(automation.id).length }) }}
              </Badge>
            </div>
            <p class="text-sm text-muted-foreground">{{ scheduleText(automation.schedule, t) }}</p>
            <p class="text-sm tabular-nums">{{ templateLine(automation) }}</p>
          </div>
          <div class="flex shrink-0 items-center gap-1">
            <Button
              variant="ghost"
              size="icon"
              class="size-8 text-muted-foreground"
              :title="t('simulation.openOne')"
              :aria-label="t('simulation.openOne')"
              @click="openSimulation(automation)"
            >
              <FlaskConical class="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              class="size-8 text-muted-foreground"
              :title="t('automation.runNow')"
              :aria-label="t('automation.runNow')"
              :disabled="runNow.isPending.value"
              @click="runNow.mutate(automation.id)"
            >
              <Play class="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              class="size-8 text-muted-foreground"
              :title="t('automation.edit')"
              :aria-label="t('automation.edit')"
              @click="openEdit(automation)"
            >
              <Pencil class="size-4" />
            </Button>
            <ConfirmButton
              :label="t('automation.delete')"
              :confirm-label="t('actions.confirmDelete')"
              @confirm="remove.mutate(automation.id)"
            >
              <Trash2 class="size-4" />
            </ConfirmButton>
          </div>
        </div>

        <Button
          variant="ghost"
          size="block"
          class="w-fit px-1 py-0.5 text-xs flex w-fit items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
          :aria-expanded="expanded === automation.id"
          @click="expanded = expanded === automation.id ? null : automation.id"
        >
          <ChevronDown class="size-3.5 transition-transform" :class="expanded === automation.id && 'rotate-180'" />
          {{ t('automation.history', { count: runsOf(automation.id).length }) }}
        </Button>

        <ul v-if="expanded === automation.id" class="grid gap-1">
          <li v-if="!runsOf(automation.id).length" class="text-xs text-muted-foreground">{{ t('automation.noRuns') }}</li>
          <li
            v-for="run in runsOf(automation.id)"
            :key="run.id"
            class="flex flex-wrap items-center justify-between gap-2 rounded-md bg-muted/40 px-3 py-2 text-sm"
          >
            <span class="flex min-w-0 items-center gap-2">
              <span class="shrink-0 tabular-nums text-muted-foreground">{{ formatDate(run.planned_at) }}</span>
              <span
                class="min-w-0"
                :class="typeof run.outcome === 'object' && 'rejected' in run.outcome && 'text-destructive'"
              >
                {{ outcomeText(run) }}
              </span>
            </span>
            <AutomationRunActions v-if="typeof run.outcome !== 'object' || !('created' in run.outcome)" :run="run" />
          </li>
        </ul>
      </div>
    </Card>

    <AutomationDialog v-model:open="dialogOpen" :automation="editing" :store-id="props.storeId" :preset="preset" />
    <AutomationSimulationDialog
      v-model:open="simulateOpen"
      :automation-id="simulated?.id ?? null"
      :name="simulated?.name ?? ''"
    />
  </div>
</template>
