<script setup lang="ts">
// check-ui-ignore row-repeat-call: runsOf/pending read maps built once per list (computed)
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { FlaskConical, Pencil, Play, Plus, Trash2, Workflow } from 'lucide-vue-next'

import { AutomationDialog } from '@/features/automation-edit'
import { AutomationSimulationDialog } from '@/features/automation-simulate'
import { Button } from '@/shared/ui/button'
import { ListHeader } from '@/shared/ui/list-header'
import { Card } from '@/shared/ui/card'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { EmptyState } from '@/shared/ui/empty-state'
import { Skeleton } from '@/shared/ui/skeleton'
import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import type { SumPart } from '@/shared/lib/money-parts'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { txKind, txLegs } from '@/shared/lib/transactions'

import type { AutomationListProps } from './automation-list.types'
import { useEditFromRoute, useNewFromRoute } from './model/use-new-from-route'

import { NO_RUNS, groupRuns } from './lib/group-runs'
import { AutomationRuns } from './ui/automation-runs'
import { AutomationSummary } from './ui/automation-summary'

const props = withDefaults(defineProps<AutomationListProps>(), { storeId: '', presets: () => [] })
/** The cards are drawn: what the page puts below can appear without being pushed. */
const emit = defineEmits<{ ready: [] }>()

const { t } = useI18n()
const queryClient = useQueryClient()
const router = useRouter()

const automations = useQuery({ queryKey: ['automations'], queryFn: () => call<Automation[]>('list_automations') })
const runs = useQuery({
  queryKey: ['automation-runs'],
  queryFn: () => call<AutomationRun[]>('list_automation_runs', { automationId: null }),
})
const tree = useStoreTree()
/** Wallet paths by id, flattened once per tree (each card asks for one or two). */
const paths = computed(() => new Map(flattenStoreTree(tree.data.value ?? []).map((node) => [node.store.id, node.path.join(' › ')])))
const storeName = (id: string) => paths.value.get(id) ?? id

/**
 * `−25,000 RUB · Tinkoff` from the template's first leg. An amount that
 * differs shows as «~» and the current estimate (the average of the latest).
 */
function templateLine(automation: Automation): SumPart[] {
  const tx = automation.template as Transaction
  const leg = txLegs(tx)[0]
  if (!leg) return [{ text: t(`tx.kind.${txKind(tx)}`) }]
  const varies = automation.amount_mode === 'varies'
  const value = varies && automation.estimate ? automation.estimate : leg.amount
  return [
    { text: `${t(`tx.kind.${txKind(tx)}`)} · ${varies ? '~' : ''}` },
    { amount: value, currency: leg.asset.symbol },
    { text: ` · ${storeName(leg.store_id)}` },
  ]
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

const grouped = computed(() => groupRuns(runs.data.value ?? []))
const runsOf = (id: string) => grouped.value.all.get(id) ?? NO_RUNS
const pending = (id: string) => grouped.value.rejected.get(id) ?? NO_RUNS
const unconfirmed = (id: string) => grouped.value.unconfirmed.get(id) ?? NO_RUNS

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
// `Change in the form` from an automation's workflow: `?edit=<id>`.
useEditFromRoute(
  () => automations.data.value,
  (automation) => openEdit(automation),
)

/* --- `Test`: emulate ahead without writing anything --- */
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
  onSuccess: refresh,
  meta: { error: 'automation.runFailed', success: 'automation.ranNow' },
})

const remove = useMutation({
  mutationFn: (id: string) => call('delete_automation', { id }),
  onSuccess: refresh,
  meta: { error: 'automation.deleteFailed', success: 'automation.deleted' },
})

const empty = computed(() => automations.isSuccess.value && !shown.value.length)
// The runs arrive after the list: a card drawn before them grows and pushes the gallery below.
const loading = computed(() => automations.isPending.value || runs.isPending.value || tree.isPending.value)
const skeletons = computed(() => Math.max(1, shown.value.length))
watch(loading, (now) => !now && emit('ready'), { immediate: true })
</script>

<template>
  <div class="grid gap-4">
    <ListHeader
      :intro="props.storeId || props.readonly ? t('automation.intro') : t('automation.introWallet')"
      :action-label="props.readonly ? undefined : t('automation.new')"
      :hide-action="empty"
      @action="openNew()"
    >
      <!-- Held while loading: the button arriving with the list would push everything below. -->
      <template v-if="loading || shown.length" #actions>
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

    <!-- While the list loads: about the height of the empty block or a couple of cards. -->
    <!-- A card per automation once the list is known (the runs come later), one before that. -->
    <template v-if="loading"><Skeleton v-for="n in skeletons" :key="n" class="h-34 rounded-xl" /></template>
    <EmptyState v-if="empty" compact :title="t('automation.empty')" :description="t('automation.intro')">
      <Button v-if="!props.readonly" @click="openNew()"><Plus class="size-4" />{{ t('automation.new') }}</Button>
    </EmptyState>

    <Card v-for="automation in loading ? [] : shown" :key="automation.id">
      <div class="grid gap-3">
        <div class="flex flex-wrap items-start justify-between gap-2">
          <AutomationSummary
            :automation="automation"
            :line="templateLine(automation)"
            :rejected="pending(automation.id).length"
            :unconfirmed="unconfirmed(automation.id).length"
          />
          <div class="flex shrink-0 items-center gap-1 pointer-coarse:gap-2">
            <Button
              variant="ghost"
              size="icon-sm"
              class="text-muted-foreground"
              :title="t('simulation.openOne')"
              :aria-label="t('simulation.openOne')"
              @click="openSimulation(automation)"
            >
              <FlaskConical class="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              class="text-muted-foreground"
              :title="t('automation.runNow')"
              :aria-label="t('automation.runNow')"
              :disabled="runNow.isPending.value"
              @click="runNow.mutate(automation.id)"
            >
              <Play class="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              class="text-muted-foreground"
              :title="t('automation.edit')"
              :aria-label="t('automation.edit')"
              @click="openEdit(automation)"
            >
              <Pencil class="size-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              class="text-muted-foreground"
              :title="t('automation.openWorkflow')"
              :aria-label="t('automation.openWorkflow')"
              data-tour="automation-open-workflow"
              @click="router.push({ name: 'workflow-edit', params: { id: automation.id } })"
            >
              <Workflow class="size-4" />
            </Button>
            <ConfirmButton
              :label="t('automation.delete')"
              :confirm-label="t('automation.deleteConfirm')"
              :description="t('confirm.irreversible')"
              @confirm="remove.mutate(automation.id)"
            >
              <Trash2 class="size-4" />
            </ConfirmButton>
          </div>
        </div>

        <AutomationRuns :runs="runsOf(automation.id)" :name="automation.name" :store-name="storeName" />
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
