<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { Search, Star } from 'lucide-vue-next'

import { CurrencyPicker } from '@/entities/currency'
import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { Checkbox } from '@/shared/ui/checkbox'
import { Dialog } from '@/shared/ui/dialog'
import { AmountInput } from '@/shared/ui/amount-input'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { nonNegativeAmount, positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS, dateInputToIso, isoToDateInput, localDate } from '@/shared/lib/planning'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'
import { DateTimePicker } from '@/shared/ui/date-time-picker'

import type { GoalDialogProps } from './goal-edit.types'
import { useStoreTree } from '@/shared/api/queries'
import { SwitchRow } from '@/shared/ui/switch-row'

const props = withDefaults(defineProps<GoalDialogProps>(), { goal: null, storeId: '' })
const open = defineModel<boolean>('open', { required: true })
const emit = defineEmits<{ saved: [goal: Goal] }>()

const { t } = useI18n()
const queryClient = useQueryClient()
const settings = useSettingsStore()

const tree = useStoreTree()
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

/** The wallet list is long: a search narrows it, the chosen ones stay on top. */
const storeSearch = ref('')
const shownNodes = computed(() => {
  const needle = storeSearch.value.trim().toLowerCase()
  return liveNodes.value
    .filter((node) => !needle || node.path.join(' ').toLowerCase().includes(needle))
    .sort((left, right) => Number(storeIds.value.includes(right.store.id)) - Number(storeIds.value.includes(left.store.id)))
})

function toggleStore(id: string) {
  storeIds.value = storeIds.value.includes(id)
    ? storeIds.value.filter((entry) => entry !== id)
    : [...storeIds.value, id]
}

const form = useFormValidation(
  () => ({
    name: required(name.value),
    target: positiveAmount(target.value),
    currency: required(currency.value, 'pick'),
    startsAt: required(startsAt.value),
    deadline: deadline.value && startsAt.value && deadline.value < startsAt.value ? t('validation.dateOrder') : null,
    commitment: nonNegativeAmount(commitment.value),
    stores: storeIds.value.length ? null : t('validation.stores'),
  }),
  { resetOn: open },
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
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
    open.value = false
    emit('saved', goal)
  },
  meta: {
    error: 'goal.saveFailed',
    success: (data) => ({ text: props.goal ? t('goal.saved') : t('goal.created'), description: (data as Goal).name }),
  },
})
</script>

<template>
  <Dialog v-model:open="open" :title="props.goal ? t('goal.edit') : t('goal.new')">
    <form id="goal-form" class="@container grid gap-5" @submit.prevent="form.submit(save.mutate, $event)">
      <FormField for="goal-name" :label="t('goal.name')" required :error="form.error('name')">
        <Input id="goal-name" v-model="name" :placeholder="t('goal.namePlaceholder')" />
      </FormField>

      <div class="grid gap-3 @xl:grid-cols-[minmax(0,1fr)_11rem]">
        <FormField for="goal-target" :label="t('goal.target')" required :error="form.error('target')">
          <AmountInput id="goal-target" v-model="target" />
        </FormField>
        <CurrencyPicker
          id="goal-currency"
          v-model="currency"
          :label="t('goal.currency')"
          required
          :error="form.error('currency')"
        />
      </div>

      <div class="grid items-start gap-3 @xl:grid-cols-3">
        <FormField for="goal-start" :label="t('goal.startsAt')" required :error="form.error('startsAt')">
          <DateTimePicker id="goal-start" v-model="startsAt" />
        </FormField>
        <FormField for="goal-deadline" :label="t('goal.deadline')" :error="form.error('deadline')">
          <DateTimePicker id="goal-deadline" v-model="deadline" />
          <p class="text-xs text-muted-foreground">{{ t('goal.optional') }}</p>
        </FormField>
        <FormField for="goal-commitment" :label="t('goal.commitment')" :error="form.error('commitment')">
          <AmountInput id="goal-commitment" v-model="commitment" :placeholder="t('goal.optional')" />
        </FormField>
      </div>

      <div class="grid gap-1.5">
        <Label>{{ t('goal.priority') }}</Label>
        <div class="flex items-center gap-1">
          <Button
            variant="ghost"
            size="icon-sm"
            v-for="level in 5"
            :key="level"
            :aria-pressed="priority === level"
            :aria-label="String(level)"
            class="motion-reduce:transform-none rounded p-0.5 transition-transform hover:scale-110"
            @click="priority = level"
          >
            <Star class="size-5" :class="level <= priority ? 'fill-warning text-warning' : 'text-muted-foreground'" />
          </Button>
        </div>
      </div>

      <div class="grid gap-2" data-tour="goal-stores" :data-invalid="form.error('stores') ? '' : undefined">
        <div class="flex items-baseline justify-between gap-2">
          <Label required>{{ t('goal.stores') }}</Label>
          <span class="text-xs text-muted-foreground">{{ t('goal.storesChosen', { count: storeIds.length }) }}</span>
        </div>
        <p v-if="form.error('stores')" class="text-xs text-destructive" role="alert">{{ form.error('stores') }}</p>
        <p v-else class="text-xs text-muted-foreground">{{ t('goal.storesHint') }}</p>
        <div class="overflow-hidden rounded-md border border-border">
          <div class="relative border-b border-border focus-within:bg-accent/40 focus-within:ring-1 focus-within:ring-inset focus-within:ring-ring">
            <Search class="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
            <input
              v-model="storeSearch"
              type="search"
              class="h-9 w-full bg-transparent pl-8 pr-3 text-sm placeholder:text-muted-foreground focus-visible:outline-none"
              :placeholder="t('goal.storesSearch')"
              :aria-label="t('goal.storesSearch')"
            />
          </div>
          <div class="grid max-h-52 gap-0.5 overflow-y-auto p-1">
            <div
              v-for="node in shownNodes"
              :key="node.store.id"
              class="flex items-center gap-2.5 rounded px-2 py-1.5 text-sm hover:bg-accent"
            >
              <Checkbox
                :id="`goal-store-${node.store.id}`"
                :model-value="storeIds.includes(node.store.id)"
                @update:model-value="toggleStore(node.store.id)"
              />
              <label :for="`goal-store-${node.store.id}`" class="min-w-0 flex-1 cursor-pointer truncate">
                {{ node.path.join(' › ') }}
              </label>
            </div>
            <p v-if="!shownNodes.length" class="px-2 py-3 text-center text-xs text-muted-foreground">{{ t('tx.side.noMatches') }}</p>
          </div>
        </div>
      </div>

      <SwitchRow id="goal-active" :label="t('goal.active')" v-model="active" />
    </form>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="goal-form" type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
