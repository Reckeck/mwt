<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Star } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { CurrencyPicker } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { AmountInput } from '@/shared/ui/amount-input'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { call, parseError } from '@/shared/api/ipc'
import { PLANNING_KEYS, dateInputToIso, isoToDateInput, localDate } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { codeErrorMessage } from '@/shared/lib/vault-error'
import { useSettingsStore } from '@/shared/store/settings'

import type { GoalDialogProps } from './goal-edit.types'

const props = withDefaults(defineProps<GoalDialogProps>(), { goal: null, storeId: '' })
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ saved: [goal: Goal] }>()

const { t } = useI18n()
const queryClient = useQueryClient()
const settings = useSettingsStore()

const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const liveNodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))

const name = ref('')
const target = ref('')
const currency = ref('')
const startsAt = ref('')
const deadline = ref('')
const priority = ref(3)
const commitment = ref('')
const storeIds = ref<string[]>([])
const active = ref(true)

watch(open, (isOpen) => {
  if (!isOpen) return
  const goal = props.goal
  name.value = goal?.name ?? ''
  target.value = goal?.target_amount ?? ''
  currency.value = goal?.currency ?? (settings.vault?.budget_currency || settings.baseCurrency)
  startsAt.value = isoToDateInput(goal?.starts_at) || localDate(new Date())
  deadline.value = isoToDateInput(goal?.deadline)
  priority.value = goal?.priority ?? 3
  commitment.value = goal?.monthly_commitment ?? ''
  storeIds.value = goal ? [...goal.scope.store_ids] : props.storeId ? [props.storeId] : []
  active.value = goal?.active ?? true
})

function toggleStore(id: string) {
  storeIds.value = storeIds.value.includes(id)
    ? storeIds.value.filter((entry) => entry !== id)
    : [...storeIds.value, id]
}

const valid = computed(
  () =>
    name.value.trim() !== '' &&
    Number(target.value) > 0 &&
    currency.value !== '' &&
    startsAt.value !== '' &&
    storeIds.value.length > 0 &&
    (!deadline.value || deadline.value >= startsAt.value),
)

const save = useMutation({
  mutationFn: () => {
    const input: GoalInput = {
      name: name.value.trim(),
      active: active.value,
      target_amount: target.value.trim(),
      currency: currency.value,
      starts_at: dateInputToIso(startsAt.value),
      deadline: deadline.value ? dateInputToIso(deadline.value) : null,
      priority: priority.value,
      monthly_commitment: commitment.value.trim() || null,
      scope: { store_ids: storeIds.value, include_children: true },
    }
    return props.goal
      ? call<Goal>('update_goal', { input: { id: props.goal.id, goal: input } })
      : call<Goal>('create_goal', { input })
  },
  onSuccess: (goal) => {
    toast.success(props.goal ? t('goal.saved') : t('goal.created'), { description: goal.name })
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
    open.value = false
    emit('saved', goal)
  },
  onError: (err) => toast.error(t('goal.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <Dialog v-model:open="open" :title="props.goal ? t('goal.edit') : t('goal.new')" size="lg">
    <form id="goal-form" class="@container grid gap-4" @submit.prevent="valid && save.mutate()">
      <div class="grid gap-3 @sm:grid-cols-[1fr_9rem_8rem]">
        <div class="grid gap-1.5">
          <Label for="goal-name">{{ t('goal.name') }}</Label>
          <Input id="goal-name" v-model="name" :placeholder="t('goal.namePlaceholder')" />
        </div>
        <div class="grid gap-1.5">
          <Label for="goal-target">{{ t('goal.target') }}</Label>
          <AmountInput id="goal-target" v-model="target" />
        </div>
        <div class="grid gap-1.5">
          <Label for="goal-currency">{{ t('goal.currency') }}</Label>
          <CurrencyPicker id="goal-currency" v-model="currency" />
        </div>
      </div>

      <div class="grid items-start gap-3 @sm:grid-cols-3">
        <div class="grid gap-1.5">
          <Label for="goal-start">{{ t('goal.startsAt') }}</Label>
          <Input id="goal-start" v-model="startsAt" type="date" />
        </div>
        <div class="grid gap-1.5">
          <Label for="goal-deadline">{{ t('goal.deadline') }}</Label>
          <Input id="goal-deadline" v-model="deadline" type="date" />
        </div>
        <div class="grid gap-1.5">
          <Label for="goal-commitment">{{ t('goal.commitment') }}</Label>
          <AmountInput id="goal-commitment" v-model="commitment" :placeholder="t('goal.optional')" />
        </div>
      </div>

      <div class="grid gap-1.5">
        <Label>{{ t('goal.priority') }}</Label>
        <div class="flex items-center gap-1">
          <button
            v-for="level in 5"
            :key="level"
            type="button"
            :aria-pressed="priority === level"
            :aria-label="String(level)"
            class="rounded p-0.5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            @click="priority = level"
          >
            <Star class="size-5" :class="level <= priority ? 'fill-amber-400 text-amber-400' : 'text-muted-foreground'" />
          </button>
        </div>
      </div>

      <div class="grid gap-1.5">
        <Label>{{ t('goal.stores') }}</Label>
        <p class="text-xs text-muted-foreground">{{ t('goal.storesHint') }}</p>
        <div class="grid max-h-48 gap-1 overflow-y-auto rounded-md border border-border p-2">
          <label v-for="node in liveNodes" :key="node.store.id" class="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              class="size-4 accent-primary"
              :checked="storeIds.includes(node.store.id)"
              @change="toggleStore(node.store.id)"
            />
            {{ node.path.join(' › ') }}
          </label>
        </div>
      </div>

      <label class="flex items-center gap-2 text-sm">
        <input v-model="active" type="checkbox" class="size-4 accent-primary" />
        {{ t('goal.active') }}
      </label>
    </form>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="goal-form" type="submit" :disabled="!valid || save.isPending.value">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
