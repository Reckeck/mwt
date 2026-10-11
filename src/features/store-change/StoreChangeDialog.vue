<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { AlertTriangle, ArrowRight } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { StoreNodeSelect } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'
import { call, parseError } from '@/shared/api/ipc'
import { formatDateTime } from '@/shared/lib/format'
import { LEDGER_KEYS } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import { CONVERTIBLE_KINDS } from './store-change.constants'
import type { StoreChangeAction, StoreChangeDialogProps } from './store-change.types'

/**
 * «Сменить тип или перенести»: pick the change and its target, read what it
 * would do (the backend works it out without writing), confirm with the
 * account's name. Conflicting operations block it until they are fixed.
 */
const props = withDefaults(defineProps<StoreChangeDialogProps>(), { initial: null })
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const router = useRouter()
const queryClient = useQueryClient()

const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const nodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))

const isRoot = computed(() => !props.store.parent_id)
const isCashRoot = computed(() => isRoot.value && props.store.kind === 'cash')
const hasChildren = computed(() => nodes.value.some((node) => node.store.parent_id === props.store.id))

const actions = computed<StoreChangeAction[]>(() => {
  const out: StoreChangeAction[] = []
  if (isRoot.value && CONVERTIBLE_KINDS.includes(props.store.kind)) out.push('change_kind')
  if (!isCashRoot.value) out.push('move_to')
  if (!isRoot.value && props.store.kind !== 'cash') out.push('detach')
  if (!isCashRoot.value && !hasChildren.value) out.push('merge_into')
  return out
})

const action = ref<StoreChangeAction>('move_to')
const kind = ref<StoreKind>('crypto')
const parentId = ref('')
const targetId = ref('')
const confirmName = ref('')

watch(open, (isOpen) => {
  if (!isOpen) return
  action.value = props.initial && actions.value.includes(props.initial) ? props.initial : (actions.value[0] ?? 'move_to')
  kind.value = CONVERTIBLE_KINDS.find((entry) => entry !== props.store.kind) ?? 'crypto'
  parentId.value = ''
  targetId.value = ''
  confirmName.value = ''
})

function pickAction(value: unknown) {
  if (typeof value === 'string' && value) action.value = value as StoreChangeAction
}

/** Wallets an account may be moved into: every other live wallet. */
const parents = computed(() =>
  nodes.value
    .filter((node) => !node.store.parent_id && node.store.id !== props.store.id && node.store.id !== props.store.parent_id)
    .map((node) => ({ ...node, children: [] })),
)
/** Accounts it may be merged into: any other live one. */
const mergeTargets = computed(() =>
  nodes.value.filter((node) => node.store.id !== props.store.id).map((node) => ({ ...node, children: [] })),
)

const change = computed<StoreChange | null>(() => {
  switch (action.value) {
    case 'change_kind':
      return { change_kind: { kind: kind.value } }
    case 'move_to':
      return parentId.value ? { move_to: { parent_id: parentId.value } } : null
    case 'detach':
      return 'detach'
    case 'merge_into':
      return targetId.value ? { merge_into: { target_id: targetId.value } } : null
  }
  return null
})

const input = computed<StoreChangeInput | null>(() =>
  change.value ? { store_id: props.store.id, change: change.value } : null,
)

const plan = useQuery({
  queryKey: computed(() => ['store-change-plan', JSON.stringify(input.value)]),
  queryFn: () => call<StoreChangePlan>('plan_store_change', { input: input.value }),
  enabled: computed(() => open.value && input.value !== null),
})

const confirmed = computed(() => confirmName.value.trim().toLowerCase() === props.store.name.trim().toLowerCase())
const blocked = computed(() => !plan.data.value || Boolean(plan.data.value.error) || plan.data.value.conflicts.length > 0)

const apply = useMutation({
  mutationFn: () => call<StoreChangePlan>('apply_store_change', { input: input.value }),
  onSuccess: () => {
    toast.success(t('storeChange.done'), { description: t('storeChange.backupHint') })
    for (const queryKey of [STORE_TREE_QUERY_KEY, ['stores'], ['store'], ['summary'], ['net-worth'], ...LEDGER_KEYS]) {
      void queryClient.invalidateQueries({ queryKey })
    }
    open.value = false
    // A merged account is gone: show where its money went.
    if (action.value === 'merge_into' && targetId.value) {
      void router.push({ name: 'wallet', params: { id: targetId.value } })
    }
  },
  onError: (err) => toast.error(t('storeChange.failed'), { description: codeErrorMessage(parseError(err), t) }),
})

const subTypeLabel = (subType: SubType | null) => (subType ? t(`subType.${subType}`) : t('storeChange.wallet'))
</script>

<template>
  <Dialog
    v-model:open="open"
    :title="t('storeChange.title', { name: props.store.name })"
    :description="t('storeChange.hint')"
    size="lg"
  >
    <form id="store-change-form" class="grid gap-4" @submit.prevent="!blocked && confirmed && apply.mutate()">
      <p v-if="!actions.length" class="text-sm text-muted-foreground">{{ t('storeChange.nothing') }}</p>
      <ToggleGroup
        v-else
        type="single"
        variant="outline"
        size="sm"
        class="flex-wrap justify-start"
        :spacing="1"
        :model-value="action"
        @update:model-value="pickAction"
      >
        <ToggleGroupItem v-for="entry in actions" :key="entry" :value="entry">{{ t(`storeChange.action.${entry}`) }}</ToggleGroupItem>
      </ToggleGroup>

      <p v-if="actions.length" class="text-xs text-muted-foreground">{{ t(`storeChange.actionHint.${action}`) }}</p>

      <div v-if="action === 'change_kind'" class="grid gap-1.5 sm:max-w-64">
        <Label for="store-change-kind">{{ t('storeChange.newKind') }}</Label>
        <Select id="store-change-kind" v-model="kind">
          <option v-for="entry in CONVERTIBLE_KINDS.filter((value) => value !== props.store.kind)" :key="entry" :value="entry">
            {{ t(`stores.kind.${entry}`) }}
          </option>
        </Select>
      </div>
      <div v-else-if="action === 'move_to'" class="grid gap-1.5">
        <Label for="store-change-parent">{{ t('storeChange.into') }}</Label>
        <StoreNodeSelect id="store-change-parent" v-model="parentId" :nodes="parents" :empty-label="t('storeChange.pick')" />
      </div>
      <div v-else-if="action === 'merge_into'" class="grid gap-1.5">
        <Label for="store-change-target">{{ t('storeChange.mergeTarget') }}</Label>
        <StoreNodeSelect id="store-change-target" v-model="targetId" :nodes="mergeTargets" :empty-label="t('storeChange.pick')" />
      </div>

      <!-- What it would do -->
      <section v-if="plan.data.value && input" class="grid gap-3 rounded-lg border border-border p-3" data-tour="store-change-plan">
        <p v-if="plan.data.value.error" class="text-sm text-destructive">{{ codeErrorMessage(parseError(plan.data.value.error), t) }}</p>
        <template v-else>
          <div v-if="plan.data.value.kept.length" class="grid gap-1">
            <p class="text-xs font-medium text-muted-foreground">{{ t('storeChange.kept') }}</p>
            <p v-for="entry in plan.data.value.kept" :key="entry.store_id" class="flex flex-wrap items-center gap-1.5 text-sm">
              <span class="font-medium">{{ entry.name }}</span>
              <span class="text-muted-foreground">{{ subTypeLabel(entry.sub_type_before) }}</span>
              <template v-if="entry.sub_type_before !== entry.sub_type_after">
                <ArrowRight class="size-3.5 text-muted-foreground" />
                <span>{{ subTypeLabel(entry.sub_type_after) }}</span>
              </template>
              <span class="text-xs text-muted-foreground">· {{ t(`stores.kind.${entry.kind_after}`) }}</span>
            </p>
          </div>
          <div v-if="plan.data.value.archived.length" class="grid gap-1">
            <p class="text-xs font-medium text-amber-700 dark:text-amber-300">{{ t('storeChange.archived') }}</p>
            <p v-for="entry in plan.data.value.archived" :key="entry.store_id" class="text-sm">
              {{ entry.name }} <span class="text-muted-foreground">· {{ subTypeLabel(entry.sub_type_before) }}</span>
            </p>
          </div>
          <p v-if="action === 'merge_into'" class="text-sm">
            {{ t('storeChange.movedTransactions', { count: plan.data.value.moved_transactions }) }}
          </p>
          <div v-if="plan.data.value.conflicts.length" class="grid gap-1">
            <p class="flex items-center gap-1.5 text-xs font-medium text-destructive">
              <AlertTriangle class="size-3.5" />
              {{ t('storeChange.conflicts', { count: plan.data.value.conflicts.length }) }}
            </p>
            <ul class="grid max-h-40 gap-1 overflow-y-auto text-xs">
              <li v-for="conflict in plan.data.value.conflicts" :key="conflict.tx_id" class="rounded-md bg-destructive/10 px-2 py-1">
                <span class="text-muted-foreground">{{ formatDateTime(conflict.timestamp) }}</span>
                <span v-if="conflict.comment"> · {{ conflict.comment }}</span>
                — {{ conflict.message }}
              </li>
            </ul>
          </div>
        </template>
      </section>

      <div v-if="input && !blocked" class="grid gap-1.5">
        <Label for="store-change-confirm">{{ t('storeChange.confirm', { name: props.store.name }) }}</Label>
        <Input id="store-change-confirm" v-model="confirmName" autocomplete="off" :placeholder="props.store.name" />
      </div>
    </form>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button
        variant="destructive"
        form="store-change-form"
        type="submit"
        :disabled="!input || blocked || !confirmed || apply.isPending.value"
      >
        {{ t('storeChange.apply') }}
      </Button>
    </template>
  </Dialog>
</template>
